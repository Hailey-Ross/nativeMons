<#
Nicknamed Pokemon across generations, on copies only. Exit code 0 when all pass. At each move it checks the nickname,
the nicknamed flag, the trainer, which converter ran, and that both saves still pass mon-bridge check.

  - Gen 1: a Pokemon named here (SPARKY, TENLETTERS) goes to every Gen 3 save and the Gen 5 save.
  - Gen 2 (optional): the save's own nicknamed Pokemon, one from its trainer and one traded in, go the same way.
  - Back: the moved SPARKY returns to Gen 1 and Gen 2, and each Gen 3 save's own nicknamed Pokemon goes to Gen 1 and Gen 2.
  - Out and home: a Gen 1 and a Gen 2 Pokemon go to Gen 3 and back, and must come home as their kept original.

  nickname-move.ps1 -Gen1 <save> [-Gen2 <save>] -Gen3 <save>[,<save>...] -Gen5 <save> [-App <built nativeMons>] [-Keep]
#>
param(
  [Parameter(Mandatory)][Alias('Yellow')][string]$Gen1, [Alias('Crystal')][string]$Gen2,
  [Parameter(Mandatory)][Alias('FireRed')][string[]]$Gen3, [Parameter(Mandatory)][Alias('White')][string]$Gen5,
  [string]$App, [string]$Work, [switch]$Keep
)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$Gen3 = @($Gen3 | ForEach-Object { $_ -split ',' } | Where-Object { $_ })
if (-not $App) {
  $choice = Join-Path $repo 'build-output.txt'
  $App = if (Test-Path $choice) { (Get-Content $choice | Where-Object { $_.Trim() } | Select-Object -First 1).Trim() } else { Join-Path $repo 'build\nativeMons' }
}
$mb = Join-Path $App 'tools\mon-bridge\mon-bridge.exe'
$env:NATIVEMONS_NODE = Join-Path $App 'tools\openhome\node.exe'
$env:NATIVEMONS_OPENHOME = Join-Path $App 'tools\openhome\convert.cjs'
$inputs = @($Gen1, $Gen5) + $Gen3 + @($Gen2 | Where-Object { $_ })
foreach ($f in @($mb, $env:NATIVEMONS_NODE, $env:NATIVEMONS_OPENHOME) + $inputs) { if (-not (Test-Path -LiteralPath $f)) { throw "Missing: $f" } }
if (-not $Work) { $Work = Join-Path $env:TEMP 'nativemons-nickname-move' }
if (Test-Path $Work) { Remove-Item -LiteralPath $Work -Recurse -Force }
New-Item -ItemType Directory -Force $Work | Out-Null

# The helper builds beside the app, never into the repo.
$helperDir = Join-Path (Split-Path $App -Parent) 'nickname-helper'
$helper = Join-Path $helperDir 'nickname-helper.exe'
$src = Join-Path $PSScriptRoot 'helper'
$newest = (Get-ChildItem $src -File | Measure-Object LastWriteTime -Maximum).Maximum
if (-not (Test-Path $helper) -or (Get-Item $helper).LastWriteTime -lt $newest) {
  & dotnet publish "$src\NicknameHelper.csproj" -c Release -v q -o $helperDir "-p:BaseIntermediateOutputPath=$helperDir/.build/obj/" "-p:BaseOutputPath=$helperDir/.build/bin/" | Out-Null
  if ($LASTEXITCODE -ne 0) { throw 'The nickname helper did not build.' }
  Remove-Item -LiteralPath "$helperDir\.build" -Recurse -Force
}

$results = New-Object System.Collections.ArrayList
function T([string]$name, [bool]$ok, [string]$detail = '') {
  [void]$results.Add($ok)
  '{0} {1}{2}' -f $(if ($ok) { 'PASS' } else { 'FAIL' }), $name, $(if ($detail) { "  [$detail]" })
}
$script:n = 0
function Fresh([string]$from, [string]$tag) { $script:n++; $f = Join-Path $Work ('{0:D2}-{1}{2}' -f $script:n, $tag, [IO.Path]::GetExtension($from)); Copy-Item -LiteralPath $from $f; $f }
function Leaf([string]$f) { Split-Path $f -Leaf }
function Slots([string]$save) { @(& $mb list $save | ForEach-Object { $c = $_ -split "`t"; "$($c[0])`t$($c[1])" }) }
function Dex([string]$save, [string]$at) { foreach ($r in & $mb list $save) { $c = $r -split "`t"; if ("$($c[0])`t$($c[1])" -eq $at) { return [int]$c[6] } }; 0 }
function Show([string]$save, [string]$at) { $b, $s = $at -split "`t"; (& $helper show $save $b $s) -split "`t" }
function Nicknamed([string]$save) { @(& $helper nicknamed $save | ForEach-Object { $c = $_ -split "`t"; [pscustomobject]@{ At = "$($c[0])`t$($c[1])"; Species = $c[2]; Nick = $c[3]; OT = $c[4] } }) }
function Check([string]$save) { & $mb check $save | Out-Null; $LASTEXITCODE -eq 0 }
function Trainer([string]$save) { ((& $mb check $save) -split "`t")[2] }
# Moves one Pokemon, then returns where it landed and the converter used.
function MoveOne([string]$from, [string]$at, [string]$to, [string]$backups) {
  $b, $s = $at -split "`t"
  $ErrorActionPreference = 'Continue'   # (5.1 turns its stderr line into an error)
  $before = Slots $to
  # Kept originals live beside the backups folder: a fresh one makes every move a real conversion.
  if (-not $backups) { $backups = Join-Path $Work ("m-{0}\backups" -f [guid]::NewGuid().ToString('N').Substring(0, 8)) }
  New-Item -ItemType Directory -Force $backups | Out-Null
  $out = & $mb move $from $b $s $to $backups dropitem 2>&1 | ForEach-Object { "$_" }
  $code = $LASTEXITCODE
  $landed = @(Slots $to | Where-Object { $before -notcontains $_ })
  [pscustomobject]@{ Code = $code; Landed = $(if ($landed.Count -eq 1) { $landed[0] } else { $null }); OpenHome = [bool]($out -match 'converter: OpenHome'); Restored = [bool]($out -match "`trestored`t"); Text = ($out -join ' | ') }
}
function Skip([string]$name, [string]$why) { "SKIP $name  [$why]" }
function Bytes([string]$save, [string]$at) { $b, $s = $at -split "`t"; $f = Join-Path $Work 'dump.bin'; & $helper dump $save $b $s $f | Out-Null; $h = (Get-FileHash -LiteralPath $f).Hash; Remove-Item -LiteralPath $f; $h }
# Into Gen 1/2, mon-bridge's own copy is used: OpenHome's output there has no names (notes/mon-bridge.md).
function Expect([string]$step, [string]$save, $move, [string]$nick, [string]$ot, [bool]$openHome) {
  T "$step`: move made" ($move.Code -eq 0 -and $move.Landed) $move.Text
  if (-not $move.Landed) { return }
  if ($openHome) { T "$step`: converted by OpenHome" $move.OpenHome } else { T "$step`: converted by mon-bridge's own copy" (-not $move.OpenHome) }
  # Game Boy text has only the curly apostrophe.
  $curly = [string][char]0x2019
  if (-not $openHome) { $nick = $nick.Replace("'", $curly); $ot = $ot.Replace("'", $curly) }
  $m = Show $save $move.Landed
  T "$step`: nickname kept" ($m[1] -ceq $nick) "got '$($m[1])', want '$nick'"
  T "$step`: marked as nicknamed" ($m[2] -eq 'nicknamed') $m[2]
  T "$step`: trainer kept" ($m[3] -ceq $ot) "got '$($m[3])', want '$ot'"
  T "$step`: destination passes checks" (Check $save)
}
# Tries candidates until one may go (a move or species the older game lacks is refused, not a failure).
function MoveFirstAllowed([string]$step, [string]$from, $candidates, [string]$toTemplate, [bool]$openHome, [int]$maxDex) {
  foreach ($c in $candidates) {
    if ($maxDex -and (Dex $from $c.At) -gt $maxDex) { continue }
    $to = Fresh $toTemplate 'to'
    $move = MoveOne $from $c.At $to
    if ($move.Code -ne 0 -and $move.Text -match "doesn't exist|don't exist|needs at least one move") { continue }
    Expect "$step ($($c.Species) '$($c.Nick)', OT $($c.OT))" $to $move $c.Nick $c.OT $openHome
    T "$step`: source passes checks" (Check $from)
    return
  }
  Skip $step 'none of its nicknamed Pokemon can go there'
}
# Out and back with the same originals folder: the kept original comes home byte for byte.
function RoundTrip([string]$step, [string]$homeSave, [string]$at, [string]$away, [switch]$OldKeep) {
  $h = Fresh $homeSave 'home'; $a = Fresh $away 'away'
  $want = Show $h $at; $wantBytes = Bytes $h $at
  $backups = Join-Path $Work ("rt-{0}\backups" -f $script:n)
  $out = MoveOne $h $at $a $backups
  if (-not $out.Landed) { T "$step`: out" $false $out.Text; return }
  # Builds before 2026-11-23 kept a Game Boy record without its names: cut the kept file back to that.
  if ($OldKeep) { foreach ($k in Get-ChildItem (Join-Path (Split-Path $backups) 'pokemon-originals') -File) { $b = [IO.File]::ReadAllBytes($k.FullName); $len = if ($k.Extension -eq '.pk1') { 44 } else { 48 }; [IO.File]::WriteAllBytes($k.FullName, $b[0..($len - 1)]) } }
  $in = MoveOne $a $out.Landed $h $backups
  T "$step`: home again, from the kept original" ($in.Code -eq 0 -and $in.Landed -and $in.Restored) $in.Text
  if (-not $in.Landed) { return }
  $got = Show $h $in.Landed
  T "$step`: names as they were" ($got[1] -ceq $want[1] -and $got[2] -eq $want[2] -and $got[3] -ceq $want[3]) "got '$($got[1])' $($got[2]) OT '$($got[3])'"
  T "$step`: record identical" ((Bytes $h $in.Landed) -eq $wantBytes)
  T "$step`: both saves pass checks" ((Check $h) -and (Check $a))
}

$originals = @{}
foreach ($f in $inputs) { $originals[$f] = (Get-FileHash -LiteralPath $f).Hash }
$targets = @($Gen3 | ForEach-Object { [pscustomobject]@{ Save = $_; Gen = 3 } }) + [pscustomobject]@{ Save = $Gen5; Gen = 5 }
$back = @([pscustomobject]@{ Save = $Gen1; Gen = 1; MaxDex = 151 }) + @($Gen2 | Where-Object { $_ } | ForEach-Object { [pscustomobject]@{ Save = $_; Gen = 2; MaxDex = 251 } })

"== Gen 1, named here"
$sparky = $null
foreach ($t in $targets) {
  $g1 = Fresh $Gen1 'gen1'
  $nick = if ($t.Gen -eq 5) { 'TENLETTERS' } else { 'SPARKY' }
  & $helper rename $g1 0 0 $nick
  $ot = (Show $g1 "0`t0")[3]
  T "Gen 1 copy: named $nick, passes checks" ((Show $g1 "0`t0")[1] -ceq $nick -and (Check $g1))
  $to = Fresh $t.Save 'to'
  $move = MoveOne $g1 "0`t0" $to
  Expect "Gen 1 to Gen $($t.Gen) $(Leaf $t.Save)" $to $move $nick $ot $true
  T "Gen 1 to Gen $($t.Gen): source passes checks" (Check $g1)
  if (-not $sparky -and $t.Gen -eq 3 -and $move.Landed) { $sparky = [pscustomobject]@{ Save = $to; At = $move.Landed; OT = $ot } }
}

if ($Gen2) {
  "== Gen 2, its own nicknames"
  $named = Nicknamed $Gen2
  $own = Trainer $Gen2
  $mine = @($named | Where-Object { $_.OT -ceq $own })
  $traded = @($named | Where-Object { $_.OT -cne $own })
  T "Gen 2 save has nicknamed Pokemon of its own and traded ones" ($mine.Count -and $traded.Count) "$($mine.Count) own, $($traded.Count) traded"
  foreach ($t in $targets) {
    foreach ($set in @(@{ Name = 'own'; List = $mine }, @{ Name = 'traded'; List = $traded })) {
      if (-not $set.List.Count) { continue }
      $g2 = Fresh $Gen2 'gen2'
      MoveFirstAllowed "Gen 2 $($set.Name) to Gen $($t.Gen) $(Leaf $t.Save)" $g2 $set.List $t.Save $true 0
    }
  }
}

"== Back to the Game Boy games"
foreach ($b in $back) {
  if ($sparky) {
    $from = Fresh $sparky.Save 'gen3'
    $to = Fresh $b.Save 'to'
    Expect "SPARKY Gen 3 to Gen $($b.Gen) $(Leaf $b.Save)" $to (MoveOne $from $sparky.At $to) 'SPARKY' $sparky.OT $false
    T "SPARKY Gen 3 to Gen $($b.Gen): source passes checks" (Check $from)
  }
  foreach ($g3 in $Gen3) {
    $list = Nicknamed $g3
    if (-not $list.Count) { Skip "Gen 3 $(Leaf $g3) to Gen $($b.Gen)" 'it has no nicknamed Pokemon'; continue }
    $from = Fresh $g3 'gen3'
    MoveFirstAllowed "Gen 3 $(Leaf $g3) to Gen $($b.Gen) $(Leaf $b.Save)" $from $list $b.Save $false $b.MaxDex
  }
}

"== Out and back home"
$g1 = Fresh $Gen1 'gen1'; & $helper rename $g1 0 0 'SPARKY'
RoundTrip "Gen 1 SPARKY to Gen 3 and home" $g1 "0`t0" $Gen3[0]
RoundTrip "Gen 1 SPARKY home from an older build's kept file" $g1 "0`t0" $Gen3[0] -OldKeep
if ($Gen2 -and (Nicknamed $Gen2).Count) { RoundTrip "Gen 2 $((Nicknamed $Gen2)[0].Nick) to Gen 3 and home" $Gen2 (Nicknamed $Gen2)[0].At $Gen3[0] }

foreach ($f in $originals.Keys) { T "original untouched: $(Leaf $f)" ((Get-FileHash -LiteralPath $f).Hash -eq $originals[$f]) }
if (-not $Keep) { Remove-Item -LiteralPath $Work -Recurse -Force }
$failed = @($results | Where-Object { -not $_ }).Count
"$($results.Count - $failed) passed, $failed failed"
exit $(if ($failed) { 1 } else { 0 })

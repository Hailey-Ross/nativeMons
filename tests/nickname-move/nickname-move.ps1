<#
Nicknamed Pokemon across generations, on copies only: names two Gen 1 Pokemon, moves them to Gen 3 and Gen 5 through
OpenHome, moves the Gen 3 one back to Gen 1, and checks names, trainer and saves at each step. Exit code 0 when all pass.

  nickname-move.ps1 -Yellow <Gen 1 save> -FireRed <Gen 3 save> -White <Gen 5 save> [-App <built nativeMons>] [-Keep]
#>
param(
  [Parameter(Mandatory)][string]$Yellow, [Parameter(Mandatory)][string]$FireRed, [Parameter(Mandatory)][string]$White,
  [string]$App, [string]$Work, [switch]$Keep
)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
if (-not $App) {
  $choice = Join-Path $repo 'build-output.txt'
  $App = if (Test-Path $choice) { (Get-Content $choice | Where-Object { $_.Trim() } | Select-Object -First 1).Trim() } else { Join-Path $repo 'build\nativeMons' }
}
$mb = Join-Path $App 'tools\mon-bridge\mon-bridge.exe'
$env:NATIVEMONS_NODE = Join-Path $App 'tools\openhome\node.exe'
$env:NATIVEMONS_OPENHOME = Join-Path $App 'tools\openhome\convert.cjs'
foreach ($f in $mb, $env:NATIVEMONS_NODE, $env:NATIVEMONS_OPENHOME, $Yellow, $FireRed, $White) { if (-not (Test-Path -LiteralPath $f)) { throw "Missing: $f" } }
if (-not $Work) { $Work = Join-Path $env:TEMP 'nativemons-nickname-move' }
if (Test-Path $Work) { Remove-Item -LiteralPath $Work -Recurse -Force }
New-Item -ItemType Directory -Force "$Work\fwd\backups", "$Work\back\backups" | Out-Null

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
function Slots([string]$save) { @(& $mb list $save | ForEach-Object { $c = $_ -split "`t"; "$($c[0])`t$($c[1])" }) }
function Show([string]$save, [string]$at) { $b, $s = $at -split "`t"; (& $helper show $save $b $s) -split "`t" }
function Check([string]$save) { & $mb check $save | Out-Null; $LASTEXITCODE -eq 0 }
# Moves one Pokemon, then returns where it landed and the converter used.
function MoveOne([string]$from, [string]$at, [string]$to, [string]$backups) {
  $b, $s = $at -split "`t"
  $ErrorActionPreference = 'Continue'   # (5.1 turns its stderr line into an error)
  $before = Slots $to
  $out = & $mb move $from $b $s $to $backups dropitem 2>&1 | ForEach-Object { "$_" }
  $code = $LASTEXITCODE
  $landed = @(Slots $to | Where-Object { $before -notcontains $_ })
  [pscustomobject]@{ Code = $code; Landed = $(if ($landed.Count -eq 1) { $landed[0] } else { $null }); OpenHome = [bool]($out -match 'converter: OpenHome'); Text = ($out -join ' | ') }
}
# Into Gen 1/2, mon-bridge's own copy is used: OpenHome's output there has no names (notes/mon-bridge.md).
function Expect([string]$step, [string]$save, $move, [string]$nick, [string]$ot, [bool]$openHome = $true) {
  T "$step`: move made" ($move.Code -eq 0 -and $move.Landed) $move.Text
  if (-not $move.Landed) { return }
  if ($openHome) { T "$step`: converted by OpenHome" $move.OpenHome } else { T "$step`: converted by mon-bridge's own copy" (-not $move.OpenHome) }
  $m = Show $save $move.Landed
  T "$step`: nickname kept" ($m[1] -ceq $nick) "got '$($m[1])', want '$nick'"
  T "$step`: marked as nicknamed" ($m[2] -eq 'nicknamed') $m[2]
  T "$step`: trainer kept" ($m[3] -ceq $ot) "got '$($m[3])', want '$ot'"
  T "$step`: destination passes checks" (Check $save)
}

$originals = @{}
foreach ($f in $Yellow, $FireRed, $White) { $originals[$f] = (Get-FileHash -LiteralPath $f).Hash }
$y = "$Work\yellow.sav"; $y2 = "$Work\yellow-back.sav"; $fr = "$Work\firered.sav"; $wh = "$Work\white.sav"
Copy-Item -LiteralPath $Yellow $y; Copy-Item -LiteralPath $Yellow $y2; Copy-Item -LiteralPath $FireRed $fr; Copy-Item -LiteralPath $White $wh

# Gen 1 boxes close up after a move, so both are named first and each move takes the first slot.
$first = "0`t0"
$ot = (Show $y $first)[3]
& $helper rename $y 0 0 'SPARKY'; & $helper rename $y 0 1 'TENLETTERS'
T 'Gen 1 copy: named SPARKY and TENLETTERS' ((Show $y $first)[1] -ceq 'SPARKY' -and (Show $y "0`t1")[1] -ceq 'TENLETTERS')
T 'Gen 1 copy: passes checks after naming' (Check $y)

$toGen3 = MoveOne $y $first $fr "$Work\fwd\backups"
Expect 'Gen 1 to Gen 3' $fr $toGen3 'SPARKY' $ot
$toGen5 = MoveOne $y $first $wh "$Work\fwd\backups"
Expect 'Gen 1 to Gen 5' $wh $toGen5 'TENLETTERS' $ot
T 'Gen 1 source passes checks after both moves' (Check $y)

# Its own backups folder keeps the kept original out of reach, so this is a real Gen 3 to Gen 1 conversion.
if ($toGen3.Landed) {
  $gen3Ot = (Show $fr $toGen3.Landed)[3]
  $back = MoveOne $fr $toGen3.Landed $y2 "$Work\back\backups"
  Expect 'Gen 3 to Gen 1' $y2 $back 'SPARKY' $gen3Ot $false
  T 'Gen 3 source passes checks after the move' (Check $fr)
}

foreach ($f in $originals.Keys) { T "original untouched: $(Split-Path $f -Leaf)" ((Get-FileHash -LiteralPath $f).Hash -eq $originals[$f]) }
if (-not $Keep) { Remove-Item -LiteralPath $Work -Recurse -Force }
$failed = @($results | Where-Object { -not $_ }).Count
"$($results.Count - $failed) passed, $failed failed"
exit $(if ($failed) { 1 } else { 0 })

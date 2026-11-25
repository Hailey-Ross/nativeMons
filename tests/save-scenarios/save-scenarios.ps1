# Launcher save features on other players' saves, no windows: reading and checks, the Collection's caught list
# (Pokedex flags plus Pokemon held, so trades count), save slots, plain export, and a Pokemon moved in counting as caught.
# Every save is copied into a throwaway app folder; the originals and the ROMs are only read.
#   save-scenarios.ps1 -App <unpacked package> -Saves <folders of <game>[-anything].sav, e.g. red.sav, red-2.sav>
#                      -RomFolder <your ROMs> [-Targets <folders of <game>[-anything].sav>] [-CtrSaves <folders of <game>[-anything].main>] [-Work <folder>] [-Keep]
# A game that plays only on Gen1Recomp keeps its save inside Gen1Recomp: it's prepared from the ROM first (minutes, its
# window off-screen), the save imported into it, then tested through exports and imports as the launcher does.
param([Parameter(Mandatory)][string]$App, [Parameter(Mandatory)][string[]]$Saves, [Parameter(Mandatory)][string]$RomFolder,
  [string[]]$Targets, [string[]]$CtrSaves, [string]$Work, [switch]$Keep)
$ErrorActionPreference = 'Stop'
if (-not $Work) { $Work = Join-Path $env:TEMP "save-scenarios-$PID" }
$results = New-Object System.Collections.ArrayList
function T($name, $ok, $why) { $s = if ($ok -is [string] -and $ok -eq 'skip') { 'SKIP' } elseif ($ok) { 'PASS' } else { 'FAIL' }; [void]$results.Add($s); Write-Host "$s  $name$(if ($why) { "  ($why)" })" }
function Sha($f) { (Get-FileHash -LiteralPath $f -Algorithm SHA256).Hash }

$catalog = @((Get-Content (Join-Path $App 'games.json') -Raw -Encoding UTF8 | ConvertFrom-Json) | ForEach-Object { $_ })
function Entry($id) { $catalog | Where-Object { $_.id -eq $id } | Select-Object -First 1 }
function Game($file) { $file.BaseName -replace '-.*$', '' }
function Pick($folders) { @(foreach ($d in @($folders | ForEach-Object { $_ -split ',' })) { if ($d) { Get-ChildItem -LiteralPath $d -Filter *.sav -File | Where-Object { Entry (Game $_) } } }) }
$originals = Pick $Saves; $mine = Pick $Targets
$ctr = @(foreach ($d in @($CtrSaves | ForEach-Object { $_ -split ',' })) { if ($d) { Get-ChildItem -LiteralPath $d -Filter *.main -File | Where-Object { (Entry (Game $_)).ctr } } })
$mine += $ctr
if (-not $originals.Count) { throw "no <game>.sav in $Saves" }
$before = @{}; foreach ($f in $originals + $mine) { $before[$f.FullName] = Sha $f.FullName }

# A copy of the app, so its state and saves are this run's alone.
if (Test-Path $Work) { Remove-Item -LiteralPath $Work -Recurse -Force }
New-Item -ItemType Directory -Force $Work | Out-Null; $appDir = Join-Path $Work 'app'
# Its own APPDATA: the launcher would otherwise copy your old Gen1Recomp folder (APPDATA\LOVE\nativemons) into the test.
$env:APPDATA = Join-Path $Work 'appdata'; New-Item -ItemType Directory -Force $env:APPDATA | Out-Null
Copy-Item -LiteralPath $App $appDir -Recurse
Remove-Item -LiteralPath (Join-Path $appDir 'state'), (Join-Path $appDir 'engines\fulldex\saves') -Recurse -Force -ErrorAction SilentlyContinue
if (-not (Test-Path (Join-Path $appDir 'NativeMonsLauncher.exe'))) { throw "no NativeMonsLauncher.exe in $App" }
$state = Join-Path $appDir 'state'; $saveDir = Join-Path $appDir 'engines\fulldex\saves'
New-Item -ItemType Directory -Force $state, $saveDir, (Join-Path $Work 'src') | Out-Null
Set-Content (Join-Path $state 'rom-folders.txt') $RomFolder
$exe = Join-Path $appDir 'NativeMonsLauncher.exe'; $bridge = Join-Path $appDir 'tools\mon-bridge\mon-bridge.exe'
$out = Join-Path $Work 'out.txt'
function Said { if (Test-Path $out) { (Get-Content $out -Raw) -replace '\s+', ' ' } else { 'no result' } }
function Bridge { $o = & $bridge @args 2>&1 | ForEach-Object { "$_" }; [pscustomobject]@{ Code = $LASTEXITCODE; Out = @($o) } }
function Launcher([string[]]$a, [int]$seconds = 180) {
  $p = Start-Process $exe -ArgumentList (($a | ForEach-Object { if ($_ -match '\s') { "`"$_`"" } else { $_ } }) -join ' ') -PassThru -WindowStyle Hidden
  try { $p.PriorityClass = 'Idle' } catch { }
  if (-not $p.WaitForExit($seconds * 1000)) { $p.Kill(); return 99 }; $p.ExitCode
}
function Caught($id) {
  Remove-Item "$out.caught" -ErrorAction SilentlyContinue
  $c = Launcher @('--saves', $id, $out, 'caught')
  $t = if (Test-Path "$out.caught") { "$(Get-Content "$out.caught" -Raw)".Trim() } else { 'missing' }
  [pscustomobject]@{ Code = $c; Text = $t; Set = @(if ($t -match '^\d') { $t -split ',' | ForEach-Object { [int]$_ } }) }
}
function Rows($save) { @((Bridge list $save).Out | Where-Object { $_ -match '^\d+\t\d+\t' }) }
function Held($save) { @(Rows $save | ForEach-Object { [int]($_ -split "`t")[6] } | Sort-Object -Unique) }
function Numbers($r) { @(if ($r.Code -eq 0 -and "$($r.Out)".Trim()) { "$($r.Out)".Trim() -split ',' | ForEach-Object { [int]$_ } }) }

# The first start sets up Gen1Recomp in the state folder and reads the ROMs (minutes at Idle); later ones are quick.
$roms = Join-Path $Work 'roms.txt'; $w = Launcher @('--roms', $roms) 1200
$found = @(Get-Content $roms -ErrorAction SilentlyContinue | Where-Object { $_ -match '^(\S+) found=[1-9]' } | ForEach-Object { ($_ -split ' ')[0] })
T 'first start reads the ROMs' ($w -eq 0 -and $found.Count -gt 0) "exit $w, $($found.Count) games found"
if ($w -ne 0) { exit 1 }

# A game the launcher keeps a cartridge save for (games.json "save"), played on Fulldex so that save is the one in use.
# A Gen1Recomp-only game: prepared from its ROM (window off-screen, as behind the splash), the save imported into it.
# Returns 'gen1'; its save in use is then read through an export (Gen1Out).
$prepared = @{}
function Gen1Ready($id) {
  if (-not $prepared.ContainsKey($id)) { $c = Launcher @('--saves', $id, $out, 'prepare') 900; $prepared[$id] = $c -eq 0; T "${id}: prepared for Gen1Recomp" ($c -eq 0) "exit $c; $(Said)" }
  $prepared[$id]
}
function Gen1Out($id) {
  $x = Join-Path $Work "g1x-$id-$([guid]::NewGuid().ToString('N').Substring(0, 6))"; $c = Launcher @('--export', $id, 'plain', $x)
  $f = @(Get-ChildItem $x -Filter *.sav -Recurse -ErrorAction SilentlyContinue | Select-Object -First 1)
  if ($c -eq 0 -and $f.Count) { $f[0].FullName }
}
function Placed($id, $file) {
  $e = Entry $id
  if (-not $e.save -and -not $e.ctr) {
    if (-not $e.gen1 -or $found -notcontains $id -or -not (Gen1Ready $id)) { return $null }
    # A cartridge .sav goes in as the import screen does it (ImportBack); adopt is only for Gen1Recomp's own slot files.
    $c = Launcher @('--saves', $id, $out, 'import', $file)
    # A Gen 1 save with an emulator footer is held back so the player can be asked, then imported without it.
    if ($c -ne 0 -and (Said) -match '^footer:') {
      T "${id}: $(Split-Path $file -Leaf) has extra bytes, so it asks before importing" $true "$((Get-Item -LiteralPath $file).Length) bytes"
      $c = Launcher @('--saves', $id, $out, 'import-trim', $file); $x = Gen1Out $id
      T "${id}: imported without the extra bytes, the same Pokemon" ($c -eq 0 -and $x -and (Bridge compare $file $x).Code -eq 0) "exit $c; $(Said)"
    }
    if ($c -ne 0) { T "${id}: Gen1Recomp takes $(Split-Path $file -Leaf)" $false "exit $c; $(Said)"; return $null }
    return 'gen1'
  }
  # A 3DS game's save is the launcher's own copy, read and moved into once it exists (no dump needed for that).
  if ($e.ctr) { $d = Join-Path $state "3ds\saves\$id\data"; New-Item -ItemType Directory -Force $d | Out-Null; Copy-Item -LiteralPath $file (Join-Path $d 'main') -Force; return (Join-Path $d 'main') }
  if (-not $e.save -or $found -notcontains $id) { return $null }
  Set-Content (Join-Path $state "engine-$id.txt") 'fulldex'
  $to = Join-Path $saveDir $e.save; Copy-Item -LiteralPath $file $to -Force; $to
}

foreach ($f in $originals) {
  $id = Game $f; $tag = $f.BaseName; $copy = Join-Path $Work "src\$($f.Name)"; Copy-Item -LiteralPath $f.FullName $copy
  $info = Bridge info $copy; $check = Bridge check $copy
  T "${tag}: reads and passes checks" ($info.Code -eq 0 -and $check.Code -eq 0) (($info.Out + $check.Out) -join ' ')
  $dex = Numbers (Bridge caught $copy); $held = Held $copy
  $s = Placed $id $copy
  if (-not $s) { T "${tag}: Collection, save slots and export" 'skip' 'no ROM for it, or it was not prepared'; continue }
  $g1 = $s -eq 'gen1'
  if ($g1) {
    $s = Gen1Out $id
    T "${tag}: Gen1Recomp holds the same save it was given" ($s -and (Bridge compare $copy $s).Code -eq 0) "export $s"
    if (-not $s) { continue }
  }
  $h = Sha $s
  # The save in use is unchanged: the same file for Fulldex games, the same Pokemon in a fresh export for Gen1Recomp.
  function Kept { if ($g1) { $now = Gen1Out $id; [bool]($now -and (Bridge compare $s $now).Code -eq 0) } else { (Sha $s) -eq $h } }
  $c = Caught $id
  T "${tag}: Collection caught list" ($c.Code -eq 0 -and $c.Set.Count -gt 0) "exit $($c.Code), $($c.Set.Count) caught, $($c.Text.Substring(0, [Math]::Min(40, $c.Text.Length)))"
  $noFlag = @($dex | Where-Object { $c.Set -notcontains $_ })
  T "${tag}: it has every Pokedex caught flag ($($dex.Count))" ($noFlag.Count -eq 0) $(if ($noFlag.Count) { "missing $($noFlag.Count): $(($noFlag | Select-Object -First 15) -join ',')" })
  $noHeld = @($held | Where-Object { $c.Set -notcontains $_ })
  T "${tag}: it has every Pokemon held, traded ones too ($($held.Count) species)" ($noHeld.Count -eq 0) $(if ($noHeld.Count) { "missing $($noHeld.Count): $(($noHeld | Select-Object -First 15) -join ',')" })

  # Save slots: a new one, back to the first, copy, delete. The save in use never changes.
  $c1 = Launcher @('--saves', $id, $out, 'new', 'Scenario'); $l1 = Said
  $c2 = Launcher @('--saves', $id, $out, 'use', '1')
  T "${tag}: new save, then back to the first" ($c1 -eq 0 -and $c2 -eq 0 -and (Kept)) "exits $c1 $c2; $l1"
  $c3 = Caught $id
  T "${tag}: an empty second save leaves the caught list the same" (($c3.Set -join ',') -eq ($c.Set -join ',')) "$($c3.Set.Count) vs $($c.Set.Count)"
  $c4 = Launcher @('--saves', $id, $out, 'copy', '1'); $c5 = Launcher @('--saves', $id, $out, 'delete', '2'); $l5 = Said
  T "${tag}: copy a save, delete another, the one in use stays" ($c4 -eq 0 -and $c5 -eq 0 -and (Kept)) "exits $c4 $c5; $l5"
  $x = Join-Path $Work "export-$id"; $c6 = Launcher @('--export', $id, 'plain', $x)
  $file = @(Get-ChildItem $x -Filter *.sav -Recurse -ErrorAction SilentlyContinue | Select-Object -First 1)
  $same = if ($file.Count -eq 1) { (Bridge compare $s $file[0].FullName).Code } else { 'none' }
  T "${tag}: plain export holds the same save" ($c6 -eq 0 -and $same -eq 0) "exit $c6, compare $same, $(@(Get-ChildItem $x -Recurse -File -ErrorAction SilentlyContinue).Name -join ' ')"
}

# A Pokemon moved in counts as caught in the game it arrives in, though that game's Pokedex never saw it. Sources are
# the downloaded saves; targets are copies of your own saves, whose Pokedex isn't full.
$sources = @($originals | ForEach-Object { Join-Path $Work "src\$($_.Name)" })
foreach ($t in $mine) {
  $id = Game $t; $tag = $t.BaseName; $to = Placed $id $t.FullName
  if (-not $to) { T "move into ${id}: counts as caught" 'skip' 'no ROM for it, or it was not prepared'; continue }
  # Gen1Recomp, as Move Pokemon does it: its save exported, the Pokemon moved into that, then imported back.
  $g1 = $to -eq 'gen1'
  if ($g1) { $to = Gen1Out $id; if (-not $to) { T "move into ${id}: its Gen1Recomp save exports" $false; continue } }
  $have = (Caught $id).Set; $done = $false
  if ($t.Extension -eq '.main') {
    $dex = Numbers (Bridge caught $to); $miss = @(($dex + (Held $to)) | Sort-Object -Unique | Where-Object { $have -notcontains $_ })
    $all = @(($dex + (Held $to)) | Sort-Object -Unique)
    T "${tag} (3DS): Collection has every caught flag and Pokemon held" (($have.Count -gt 0 -or $all.Count -eq 0) -and $miss.Count -eq 0) "$($have.Count) caught$(if ($miss.Count) { ", missing $($miss.Count): $(($miss | Select-Object -First 15) -join ',')" })"
  }
  :next foreach ($from in $sources) {
    foreach ($row in Rows $from) {
      $col = $row -split "`t"; $dexNo = [int]$col[6]
      if ($have -contains $dexNo) { continue }
      $plan = Bridge plan $from $col[0] $col[1] $to
      if ($plan.Code -ne 0 -or ($plan.Out -match '^(forced|conflict|item)')) { continue }
      $boxHad = @(Rows $from | Where-Object { $_ -match "^$($col[0])\t" }).Count
      $m = Bridge move $from $col[0] $col[1] $to (Join-Path $Work 'move-backups')
      $name = "move $($col[2]) (#$dexNo) from $(Split-Path $from -Leaf) into $id"
      if ($m.Code -ne 0) { T $name $false "exit $($m.Code): $($m.Out -join ' ')"; $done = $true; break next }
      if ($g1) {
        $ic = Launcher @('--saves', $id, $out, 'import', $to); $back = Gen1Out $id
        T "${name}: Gen1Recomp takes the save back" ($ic -eq 0 -and $back -and (Bridge compare $to $back).Code -eq 0) "exit $ic; $(Said)"
      }
      $after = Caught $id
      T "${name}: counts as caught there" ($after.Set -contains $dexNo) "$($have.Count) to $($after.Set.Count) caught"
      T "  both saves pass checks after it" ((Bridge check $from).Code -eq 0 -and (Bridge check $to).Code -eq 0)
      # Gen 1/2 boxes close the gap (the next Pokemon moves up), so the box holds one fewer rather than an empty slot.
      $inBox = @(Rows $from | Where-Object { $_ -match "^$($col[0])\t" }).Count
      T "  the box it left holds one fewer" ($inBox -eq $boxHad - 1) "$boxHad to $inBox"
      $done = $true; break next
    }
  }
  if (-not $done) { T "move into ${id}: counts as caught" 'skip' 'no Pokemon new to it that moves without a choice' }
}

T 'the original saves are untouched' (@(($originals + $mine) | Where-Object { (Sha $_.FullName) -ne $before[$_.FullName] }).Count -eq 0)
Get-Process NativeMonsLauncher, lovec -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "$Work*" } | Stop-Process -Force
if (-not $Keep) { Remove-Item -LiteralPath $Work -Recurse -Force -ErrorAction SilentlyContinue }
$fail = @($results | Where-Object { $_ -eq 'FAIL' }).Count; $pass = @($results | Where-Object { $_ -eq 'PASS' }).Count; $skip = @($results | Where-Object { $_ -eq 'SKIP' }).Count
Write-Host "`n$pass passed, $fail failed, $skip skipped"
exit [int]($fail -gt 0)

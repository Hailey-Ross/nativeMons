<#
Decides whether a release package is fit to be the stable build. Run before moving main.

By default it's quick: one game per engine, the main screens, one update of each kind (about 10 minutes). -Full tries every
game on every engine and every screen; a release decision needs a full run of play, snapshots and updater.

The launcher is told not to take the keyboard, so you can keep working; the games' own windows may still take it briefly.

The suite is split into groups, and each group's latest result is kept. Retest a part without redoing the rest; the verdict
always covers every group, and a group that hasn't run counts against it.

  Everything:      stable-check.ps1 -Package <zip> -Work <empty scratch folder> -RomFolder <folder> -Report <folder> [options]
  What failed:     stable-check.ps1 -Report <folder> -Retest
  Chosen groups:   stable-check.ps1 -Report <folder> -Groups saves,sync
  Leave some out:  stable-check.ps1 -Report <folder> -Skip play
  Where things are: stable-check.ps1 -Report <folder> -List

Settings are remembered in the report folder, so later runs only need -Report. Give one again to change it.

  repo       what's published: no games or attribution, pins, committed and pushed, dev build green
  package    the zip: contents, licences, credit, nothing private
  scan       finding ROMs, several dumps of a game
  snapshots  every screen draws
  saves      checks, several saves, export, moving a Pokemon, on copies
  play       every game on every engine shows a picture, and the launcher comes back (long)
  sync       shared saves between engines, including damaged ones
  window     real key presses through the real window, checked by screen name
  updater    updates and Stable/Dev switching, from local releases (long)

  -Package           the release zip to test (from the bleeding-edge release, normally)
  -Work              scratch folder, deleted at the end unless -Keep
  -RomFolder         your ROMs. Only read, and listed before and after to prove it.
  -SaveFolder        folder of Fulldex .sav files. They're copied in; the originals are only read.
  -ArtFolder         a state\game-art folder to copy in, so the launcher doesn't spend the run capturing art.
  -Report            where results and screenshots are kept (kept after -Work is deleted)
  -LauncherOverride  a locally built launcher to test in place of the package's. The result then doesn't speak for the
                     package itself: run again without it before releasing.

Windows appear on screen during play and window: don't use the PC for games, or move the mouse over them, meanwhile.
Exit code: 0 every group has run with no failures, 1 otherwise.
#>
param(
  [string]$Package, [string]$Work, [string]$RomFolder,
  [string]$SaveFolder, [string]$ArtFolder, [string]$LauncherOverride, [string]$Report,
  [Alias('Stages')][string[]]$Groups,
  [string[]]$Skip = @(),
  [switch]$Retest, [switch]$List, [switch]$Keep, [switch]$Full
)
$ErrorActionPreference = 'Continue'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$AllGroups = @('repo','package','scan','snapshots','saves','play','sync','window','updater')
if (-not $Report) { $Report = Join-Path $env:TEMP 'nativemons-stable-check' }
New-Item -ItemType Directory -Force $Report, "$Report\shots" | Out-Null
$Report = (Resolve-Path $Report).Path
function GroupFile([string]$g) { Join-Path $Report "group-$g.txt" }
function GroupState([string]$g) {
  $f = GroupFile $g
  if (-not (Test-Path $f)) { return 'NOT RUN' }
  $lines = @(Get-Content $f)
  if (@($lines | Where-Object { $_ -like "FAIL`t*" }).Count) { return 'FAILED' }
  return 'passed'
}
function Overview { foreach ($g in $AllGroups + 'suite' + 'integrity') { if (($g -in 'suite','integrity') -and -not (Test-Path (GroupFile $g))) { continue }; $n = if (Test-Path (GroupFile $g)) { @(Get-Content (GroupFile $g)).Count } else { 0 }; '{0,-10} {1,-8} {2} checks' -f $g, (GroupState $g), $n } }
if ($List) { Overview; exit 0 }
$bad = @(@($Groups) + @($Skip) | Where-Object { $_ -and $AllGroups -notcontains $_ })
if ($bad) { throw "Unknown group: $($bad -join ', '). Groups: $($AllGroups -join ' ')" }
if ($Retest) {
  $Groups = @($AllGroups | Where-Object { (GroupState $_) -ne 'passed' })
  if ((Test-Path (GroupFile 'suite')) -and (GroupState 'suite') -eq 'FAILED' -and -not $Groups) { $Groups = $AllGroups }
  if (-not $Groups) { 'Nothing failed or left to run.'; Overview; exit 0 }
  "Retesting: $($Groups -join ' ')"
}
if (-not $Groups) { $Groups = $AllGroups }
$Stages = @($Groups)
# Settings from the last run, unless given again.
$remember = Join-Path $Report 'last-run.json'
$was = if (Test-Path $remember) { Get-Content $remember -Raw | ConvertFrom-Json } else { $null }
foreach ($k in 'Package','Work','RomFolder','SaveFolder','ArtFolder','LauncherOverride') {
  if (-not (Get-Variable $k -ValueOnly) -and $was -and $was.$k) { Set-Variable $k $was.$k }
}
foreach ($k in 'Package','Work','RomFolder') { if (-not (Get-Variable $k -ValueOnly)) { throw "-$k is needed the first time." } }
@{ Package = $Package; Work = $Work; RomFolder = $RomFolder; SaveFolder = $SaveFolder; ArtFolder = $ArtFolder; LauncherOverride = $LauncherOverride } | ConvertTo-Json | Set-Content $remember
if (Test-Path $Work) { if (Get-ChildItem $Work -Force | Select-Object -First 1) { throw "Work folder isn't empty: $Work" } }
New-Item -ItemType Directory -Force $Work | Out-Null
$Work = (Resolve-Path $Work).Path
# A group's old result goes when it runs again; the others stay.
foreach ($g in @($Stages | Where-Object { $Skip -notcontains $_ }) + 'suite' + 'integrity') { Remove-Item (GroupFile $g), "$(GroupFile $g).quick" -ErrorAction SilentlyContinue; if (-not $Full) { Set-Content "$(GroupFile $g).quick" '' } }
$app = Join-Path $Work 'app'; $exe = Join-Path $app 'NativeMonsLauncher.exe'; $st = Join-Path $Work 'state'
$results = New-Object System.Collections.ArrayList
$log = Join-Path $Report 'results.txt'
$header = "stable-check $(Get-Date -Format s)  package: $(Split-Path $Package -Leaf)$(if ($LauncherOverride) { '  LAUNCHER OVERRIDDEN' })"

function T([string]$stage, [string]$name, $ok, [string]$detail = '') {
  # (Only a string can be a skip: $true -eq 'skip' is true in PowerShell.)
  $state = if ($ok -is [string] -and $ok -eq 'skip') { 'SKIP' } elseif ($ok) { 'PASS' } else { 'FAIL' }
  [void]$results.Add([pscustomobject]@{ Stage = $stage; Name = $name; State = $state; Detail = $detail })
  $line = '{0,-4} {1,-10} {2}{3}' -f $state, $stage, $name, $(if ($detail) { "  [$detail]" })
  Write-Output $line
  Add-Content (GroupFile $stage) "$state`t$name`t$detail"
  $script:lastGroup = $stage
}
function On([string]$s) { ($Stages -contains $s) -and ($Skip -notcontains $s) }
# A game makes its save file on first start, all FF until the player saves. That's empty, not damaged.
function IsBlank([string]$f) { foreach ($b in [IO.File]::ReadAllBytes($f)) { if ($b -ne 255 -and $b -ne 0) { return $false } }; return $true }
function Sha([string]$f) { (Get-FileHash -LiteralPath $f -Algorithm SHA1).Hash }
# The launcher is a windowed program: it's waited for, with a limit.
function Launcher([string[]]$a, [int]$seconds = 120) {
  $line = ($a | ForEach-Object { if ($_ -match '[\s]') { '"' + $_ + '"' } else { $_ } }) -join ' '
  $p = Start-Process $exe -ArgumentList $line -PassThru -WindowStyle Hidden
  if (-not $p.WaitForExit($seconds * 1000)) { try { $p.Kill() } catch { }; return -999 }
  return $p.ExitCode
}
function Bridge { $b = Join-Path $app 'tools\mon-bridge\mon-bridge.exe'; $o = & $b @args 2>&1; [pscustomobject]@{ Code = $LASTEXITCODE; Out = ($o | ForEach-Object { "$_" }) } }
function GameProcs {
  foreach ($n in 'lovec','love','nativeplat','main','Pokemon Fulldex') {
    foreach ($p in (Get-Process $n -ErrorAction SilentlyContinue)) { try { if ($p.Path -and $p.Path.StartsWith($app, [StringComparison]::OrdinalIgnoreCase)) { $p } } catch { } }
  }
}
function ClearGames { foreach ($p in @(GameProcs)) { try { $p.Kill() } catch { } }; Start-Sleep 1 }
Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @'
using System; using System.Drawing; using System.Runtime.InteropServices;
public static class StableWin {
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint f);
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
  public struct RECT { public int L, T, R, B; }
  public struct POINT { public int X, Y; }
  public static string ScreenShot(IntPtr h, string f) { RECT r; if (!GetClientRect(h, out r) || r.R < 1 || r.B < 1) return ""; var o = new POINT(); ClientToScreen(h, ref o); using (var b = new Bitmap(r.R, r.B)) { using (var g = Graphics.FromImage(b)) g.CopyFromScreen(o.X, o.Y, 0, 0, new Size(r.R, r.B)); b.Save(f); } return r.R + "x" + r.B; }
  public static string Shot(IntPtr h, string f) { RECT r; if (!GetClientRect(h, out r) || r.R < 1 || r.B < 1) return ""; using (var b = new Bitmap(r.R, r.B)) { using (var g = Graphics.FromImage(b)) { var dc = g.GetHdc(); PrintWindow(h, dc, 3); g.ReleaseHdc(dc); } b.Save(f); } return r.R + "x" + r.B; }
  public static void Key(IntPtr h, int k) { PostMessage(h, 0x100, (IntPtr)k, IntPtr.Zero); PostMessage(h, 0x101, (IntPtr)k, IntPtr.Zero); System.Threading.Thread.Sleep(350); }
  // How many colours in a sample of the picture: a blank or failed drawing has one.
  public static int Colours(string f) { using (var b = new Bitmap(f)) { var seen = new System.Collections.Generic.HashSet<int>(); for (int y = 0; y < b.Height; y += Math.Max(1, b.Height / 40)) for (int x = 0; x < b.Width; x += Math.Max(1, b.Width / 40)) seen.Add(b.GetPixel(x, y).ToArgb()); return seen.Count; } }
}
'@
function RomList { Get-ChildItem -LiteralPath $RomFolder -Recurse -File -ErrorAction SilentlyContinue | Where-Object { $_.Extension -match '^\.(gb|gbc|gba|nds)$' } | ForEach-Object { "$($_.FullName)|$($_.Length)|$($_.LastWriteTimeUtc.Ticks)" } | Sort-Object }

$script:played = @(); $oldAppData = $env:APPDATA; $romsBefore = RomList; $saveHashes = @{}
if ($SaveFolder) { Get-ChildItem -LiteralPath $SaveFolder -Filter *.sav -File | ForEach-Object { $saveHashes[$_.FullName] = Sha $_.FullName } }
try {
# ---------- repo: what's published is allowed to be ----------
if (On 'repo') {
  Push-Location $repo
  $tracked = git ls-files
  $bad = $tracked | Where-Object { $_ -match '\.(gb|gbc|gba|nds|sav)$' -or $_ -match '^\.claude/' -or $_ -match 'key1\.bin$' -or $_ -match '(^|/)state/' }
  T repo 'no games, saves, keys or private folders are tracked' (-not $bad) ($bad -join ', ')
  $attr = git grep -il -E 'co-authored-by|claude-session|generated with claude' -- ':!tests/stable-check'
  T repo 'no attribution lines in the tree' (-not $attr) ($attr -join ', ')
  $msgs = git log -60 --format=%B | Select-String -Pattern 'co-authored-by|claude-session' -SimpleMatch:$false
  T repo 'no attribution lines in recent commits' (-not $msgs)
  # Git for Windows' sh, which usually isn't on PATH.
  $sh = Join-Path (Split-Path (Split-Path (Get-Command git).Source)) 'bin\sh.exe'
  $removed = if (Test-Path $sh) { & $sh engines/gen1recomp/tools/nativemons/check-removed.sh 2>&1 | Select-Object -Last 1 } else { "no sh at $sh" }
  T repo 'Gen1Recomp proprietary launcher files are absent' ("$removed" -match '^OK') "$removed"
  $ba = Get-Content launcher\build-all.ps1 -Raw; $ay = Get-Content .github\actions\build-package\action.yml -Raw
  foreach ($pin in @(@('fulldexRelease','FULLDEX_RELEASE'), @('nativeplatRelease','NATIVEPLAT_RELEASE'))) {
    $a = [regex]::Match($ba, '\$' + $pin[0] + "\s*=\s*'([^']+)'").Groups[1].Value; $b = [regex]::Match($ay, $pin[1] + ':\s*"([^"]+)"').Groups[1].Value
    T repo "$($pin[0]) pin matches in both build files" ($a -and $a -eq $b) "$a / $b"
  }
  $sub = git submodule status 2>&1 | Where-Object { $_ -match '^[+\-U]' }
  T repo 'submodules match their pins' (-not $sub) ($sub -join '; ')
  # A locally built launcher is by nature ahead of what's published, so these only mean something on a published package.
  if ($LauncherOverride) {
    foreach ($n in 'everything that ships is committed','nothing unpushed','the package came from a green dev build','nothing that ships changed since the package was built') { T repo $n 'skip' 'testing a local launcher' }
  } else {
    # What matters is the package: built from a green dev run, with nothing that ships changed since. The suite itself doesn't ship.
    $shipped = @('.', ':!tests')
    $dirty = git status --porcelain -- @shipped
    T repo 'everything that ships is committed' (-not $dirty) ($dirty -join '; ')
    $ahead = git rev-list --count origin/dev..HEAD 2>$null
    T repo 'nothing unpushed' ("$ahead" -eq '0') "$ahead ahead of origin/dev"
    $pkgRev = [regex]::Match(((Get-Content (Join-Path $app 'version.txt') -ErrorAction SilentlyContinue) -join ' '), 'revision=([0-9a-f]{40})').Groups[1].Value
    if (-not $pkgRev) {
      # (The package isn't unpacked until later: read its version.txt straight from the zip.)
      Add-Type -AssemblyName System.IO.Compression.FileSystem
      $z = [IO.Compression.ZipFile]::OpenRead($Package)
      try { $v = $z.Entries | Where-Object { $_.FullName -match '(^|/)version\.txt$' } | Select-Object -First 1; if ($v) { $r = New-Object IO.StreamReader($v.Open()); $pkgRev = [regex]::Match($r.ReadToEnd(), 'revision=([0-9a-f]{40})').Groups[1].Value; $r.Close() } } finally { $z.Dispose() }
    }
    # (Not --jq: Windows PowerShell drops the quotes inside it on the way to gh.)
    $runs = @(gh run list -R Hailey-Ross/nativeMons --branch dev --limit 20 --json conclusion,headSha 2>$null | ConvertFrom-Json)
    $built = $runs | Where-Object { $_.headSha -eq $pkgRev } | Select-Object -First 1
    T repo 'the package came from a green dev build' ($built -and $built.conclusion -eq 'success') "package $($pkgRev.Substring(0, [Math]::Min(8, $pkgRev.Length))): $(if ($built) { $built.conclusion } else { 'no run found' })"
    $since = if ($pkgRev) { git diff --name-only $pkgRev HEAD -- @shipped 2>$null } else { 'unknown' }
    T repo 'nothing that ships changed since the package was built' (-not $since) ($since -join ', ')
  }
  Pop-Location
}
# ---------- package ----------
Expand-Archive -LiteralPath $Package "$Work\unzipped"
$top = Get-ChildItem "$Work\unzipped" -Directory | Select-Object -First 1
Move-Item $top.FullName $app; Remove-Item "$Work\unzipped" -Recurse -Force
if (On 'package') {
  $side = "$Package.sha256"
  if (Test-Path -LiteralPath $side) { T package 'zip matches its published SHA-256' ((Get-Content -LiteralPath $side -Raw) -match (Get-FileHash -LiteralPath $Package -Algorithm SHA256).Hash) } else { T package 'zip matches its published SHA-256' 'skip' 'no .sha256 beside it' }
  foreach ($f in 'NativeMonsLauncher.exe','games.json','version.txt','LICENSE','README.md','love\lovec.exe','engines\gen1recomp\main.lua','engines\gen1recomp\LICENSE.MD','engines\fulldex\Pokemon Fulldex.exe','engines\fulldex\LICENSE-mGBA.txt','engines\nativeplat\nativeplat.exe','engines\pokeplatinum\main.exe','tools\mon-bridge\mon-bridge.exe','tools\openhome\node.exe') {
    T package "has $f" (Test-Path (Join-Path $app $f))
  }
  $all = Get-ChildItem $app -Recurse -File -Force
  $bad = $all | Where-Object { $_.Extension -match '^\.(gb|gbc|gba|nds|sav)$' -or $_.Name -eq 'key1.bin' -or $_.FullName -match '\\\.claude\\' -or $_.FullName -match [regex]::Escape($app) + '\\state\\' }
  T package 'no games, saves, keys or state inside' (-not $bad) (($bad | Select-Object -First 5 | ForEach-Object Name) -join ', ')
  $prop = $all | Where-Object { $_.Name -match '^(LauncherView|LauncherSettings|OnlinePanel|CartLabelArt|CartShape|LauncherMods)\.lua$' -or $_.FullName -match '\\assets\\(launcher|labels)\\' }
  T package 'no Gen1Recomp proprietary launcher files inside' (-not $prop) (($prop | Select-Object -First 5 | ForEach-Object Name) -join ', ')
  # .NET keeps its strings as UTF-16, which can start on an odd byte: read it both ways.
  $raw = [IO.File]::ReadAllBytes($exe)
  $text = [Text.Encoding]::Unicode.GetString($raw) + [Text.Encoding]::Unicode.GetString($raw, 1, $raw.Length - 1)
  T package 'launcher carries the Gen1Recomp credit' ($text.Contains('BOIS CLUB GAMES, LLC') -and $text.Contains('github.com/bryanthaboi/gen1recomp'))
  $v = Get-Content (Join-Path $app 'version.txt') -ErrorAction SilentlyContinue
  T package 'version.txt has channel, build and revision' (($v -join ' ') -match 'channel=\S+ build=\d+ revision=[0-9a-f]{40}') ($v -join ' ')
  $games = Get-Content (Join-Path $app 'games.json') -Raw | ConvertFrom-Json
  T package 'games.json reads, with every game' (@($games).Count -ge 20 -or @($games.games).Count -ge 20)
}
if ($LauncherOverride) { Copy-Item -LiteralPath $LauncherOverride $exe -Force }
# Everything from here is isolated: its own state, its own LOVE saves, quiet and in a window.
New-Item -ItemType Directory -Force $st, "$Work\appdata" | Out-Null
$env:NATIVEMONS_STATE = $st; $env:APPDATA = "$Work\appdata"; $env:NATIVEMONS_TEST_QUIET = '1'
Set-Content "$st\rom-folders.txt" $RomFolder
Set-Content "$st\game-options.txt" "full=off`nvol=0"
if ($ArtFolder -and (Test-Path -LiteralPath $ArtFolder)) { Copy-Item -LiteralPath $ArtFolder "$st\game-art" -Recurse }
$saves = Join-Path $app 'engines\fulldex\saves'; New-Item -ItemType Directory -Force $saves | Out-Null
if ($SaveFolder) { Get-ChildItem -LiteralPath $SaveFolder -Filter *.sav -File | Copy-Item -Destination $saves }
# ---------- scan ----------
$found = @(); $romLines = @()
if ((On 'scan') -or (On 'play') -or (On 'window') -or (On 'snapshots') -or (On 'saves') -or (On 'sync')) {
  $s = Get-Date; $c = Launcher @('--roms', "$Report\roms.txt") 600
  $first = [int]((Get-Date) - $s).TotalSeconds
  $romLines = Get-Content "$Report\roms.txt" -ErrorAction SilentlyContinue
  $found = @($romLines | Where-Object { $_ -match '^(\S+) found=([1-9]\d*) ' } | ForEach-Object { $_ -match '^(\S+) ' | Out-Null; $Matches[1] })
  if (On 'scan') {
    T scan 'scan finishes' ($c -eq 0) "exit $c, ${first}s"
    T scan 'finds at least one game' ($found.Count -gt 0) "$($found.Count) games: $($found -join ' ')"
    $s = Get-Date; $c = Launcher @('--roms', "$Report\roms2.txt") 300; $second = [int]((Get-Date) - $s).TotalSeconds
    T scan 'a second scan gives the same answer' ((Get-Content "$Report\roms.txt" -Raw) -eq (Get-Content "$Report\roms2.txt" -Raw))
    T scan 'second scan uses the hash cache' ($second -le [Math]::Max(5, $first)) "${first}s then ${second}s"
    $dups = @($romLines | Where-Object { $_ -match 'copy=True' })
    T scan 'no engine folder copies counted as dumps' ($dups.Count -eq 0) "$($dups.Count)"
    $picks = @($found | Where-Object { Test-Path "$st\rom-$_.txt" })
    $multi = @($romLines | Where-Object { $_ -match '^\S+ found=([2-9]|\d\d)' } | ForEach-Object { ($_ -split ' ')[0] })
    T scan 'each game with several dumps remembers one' (@($multi | Where-Object { $picks -notcontains $_ }).Count -eq 0) "$($multi -join ' ')"
    Remove-Item "$Report\roms2.txt" -ErrorAction SilentlyContinue
  }
}
# ---------- snapshots: every screen draws ----------
if (On 'snapshots') {
  $sample = if ($Full) { $found } else { @('red','crystal','firered','platinum','black') | Where-Object { $found -contains $_ } }
  $screens = @('main','splash','games','folders','credits','sprites','updatebanner') + @($sample | ForEach-Object { "main:$_"; "gamepage:$_"; "saves:$_" })
  $screens += @($romLines | Where-Object { $_ -match '^\S+ found=([2-9]|\d\d)' } | ForEach-Object { 'roms:' + ($_ -split ' ')[0] } | Select-Object -First $(if ($Full) { 99 } else { 1 }))
  $failed = @()
  foreach ($scr in $screens) {
    $png = "$Report\shots\" + ($scr -replace ':', '-') + '.png'
    $c = Launcher @('--snapshot', $png, $scr, '600x1000') 90
    $colours = 0; if (Test-Path $png) { try { $colours = [StableWin]::Colours($png) } catch { } }
    if ($c -ne 0 -or $colours -lt 4) { $failed += "$scr(exit $c, colours $colours)" }
  }
  T snapshots "$($screens.Count) screens draw" ($failed.Count -eq 0) ($failed -join ', ')
}
# ---------- saves ----------
if (On 'saves') {
  $sv = @(Get-ChildItem $saves -Filter *.sav -File -ErrorAction SilentlyContinue)
  if ($sv.Count -eq 0) { T saves 'save tests' 'skip' 'no -SaveFolder' } else {
    foreach ($f in $sv) {
      if (IsBlank $f.FullName) { T saves "$($f.Name)" 'skip' 'blank: never saved in-game'; continue }
      $h = Sha $f.FullName
      $r = Bridge check $f.FullName; if ($r.Code -eq 0) { $a = Bridge summary $f.FullName; $b = Bridge compare $f.FullName $f.FullName }
      T saves "$($f.Name): reads, passes checks, equals itself" ($r.Code -eq 0 -and $a.Code -eq 0 -and $b.Code -eq 0) "check $($r.Code)"
      T saves "$($f.Name): checks leave it untouched" ($h -eq (Sha $f.FullName))
    }
    $plat = Join-Path $saves 'platinum.sav'
    if (Test-Path $plat) {
      $port = "$Work\port.bin"; $back = "$Work\back.sav"
      [void](Launcher @('--platinum-convert','to-port',$plat,$port) 60); [void](Launcher @('--platinum-convert','to-ds',$port,$back) 60)
      $r = if (Test-Path $back) { Bridge compare $plat $back } else { [pscustomobject]@{ Code = 99 } }
      T saves 'Platinum: to the port and back keeps the same Pokemon and trainer' ($r.Code -eq 0) "compare $($r.Code)"
    }
    # Several saves, on the first game with one.
    $g = @($found | Where-Object { Test-Path (Join-Path $saves "$_.sav") }) | Select-Object -First 1
    if ($g) {
      $m = Join-Path $saves "$g.sav"; $h = Sha $m; $o = "$Work\sets.txt"
      $c1 = Launcher @('--saves',$g,$o,'new','Stable test') 120; $after = (Get-Content $o -Raw)
      $c2 = Launcher @('--saves',$g,$o,'use','1') 120
      T saves "${g}: new save, then back to the first" ($c1 -eq 0 -and $c2 -eq 0 -and (Sha $m) -eq $h) "exits $c1 $c2; $($after -replace '\s+',' ')"
      $c3 = Launcher @('--saves',$g,$o,'copy','1') 120; $setList = Get-Content $o -Raw
      T saves "${g}: copy a save" ($c3 -eq 0) ($setList -replace '\s+',' ')
      $c4 = Launcher @('--saves',$g,$o,'delete','2') 120
      T saves "${g}: delete another, and the one in use stays the same" ($c4 -eq 0 -and (Sha $m) -eq $h) "exit $c4"
      $x = "$Work\export"; $c5 = Launcher @('--export',$g,'plain',$x) 120
      $file = @(Get-ChildItem $x -Filter *.sav -ErrorAction SilentlyContinue | Select-Object -First 1)
      T saves "${g}: plain export" ($c5 -eq 0 -and $file.Count -eq 1) "exit $c5"
      if ($file.Count -eq 1) { $r = Bridge compare $m $file[0].FullName; T saves "${g}: export holds the same save" ($r.Code -eq 0) "compare $($r.Code)" }
    }
    # A real move between copies, from a save with something boxed to the next readable one.
    $real = @($sv | Where-Object { $_.Name -notmatch 'rocket' -and -not (IsBlank $_.FullName) })
    $source = $real | Where-Object { @((Bridge list $_.FullName).Out | Where-Object { $_ -match '^\d+\t\d+\t' }).Count -gt 0 } | Select-Object -First 1
    $target = $real | Where-Object { $source -and $_.Name -ne $source.Name } | Select-Object -First 1
    if ($source -and $target) {
      $f2 = "$Work\from.sav"; $t2 = "$Work\to.sav"; Copy-Item $source.FullName $f2; Copy-Item $target.FullName $t2
      $name = "move a Pokemon, $($source.BaseName) to $($target.BaseName)"
      (Bridge list $f2).Out | Where-Object { $_ -match '^(\d+)\t(\d+)\t' } | Select-Object -First 1 | Out-Null
      $mon = (Bridge list $f2).Out | Where-Object { $_ -match '^\d+\t\d+\t' } | Select-Object -First 1
      $mon -match '^(\d+)\t(\d+)\t' | Out-Null; $box = $Matches[1]; $slot = $Matches[2]
      $before = @((Bridge list $t2).Out | Where-Object { $_ -match '^\d+\t\d+\t' }).Count
      $r = Bridge move $f2 $box $slot $t2 "$Work\move-backups"
      $nowCount = @((Bridge list $t2).Out | Where-Object { $_ -match '^\d+\t\d+\t' }).Count
      if ($r.Code -eq 2) { T saves $name 'skip' 'this Pokemon needs a choice of moves' }
      else {
        T saves "$name, with a backup" ($r.Code -eq 0 -and $nowCount -eq $before + 1 -and (Test-Path "$Work\move-backups")) "exit $($r.Code), boxes $before to $nowCount"
        T saves 'the save moved to passes checks' ((Bridge check $t2).Code -eq 0)
        T saves 'the save moved from passes checks' ((Bridge check $f2).Code -eq 0)
      }
    } else { T saves 'move a Pokemon' 'skip' 'needs a save with a boxed Pokemon and another save' }
  }
}
# ---------- play: every game, on every engine it has ----------
if (On 'play') {
  $matrix = @()
  foreach ($l in $romLines) {
    if ($l -notmatch '^(\S+) found=[1-9]') { continue }
    $id = $Matches[1]
    $m = [regex]::Match($l, 'fulldex=(.*?) \| gen1=(.*?) \| nativeplat=(.*?) \|')
    $enc = $false; $i = [array]::IndexOf($romLines, $l) + 1
    while ($i -lt $romLines.Count -and $romLines[$i].StartsWith('    ')) { if ($romLines[$i] -match 'enc=True' -and $romLines[$i].Contains($m.Groups[1].Value) -and $m.Groups[1].Value) { $enc = $true }; $i++ }
    if ($m.Groups[1].Value) { $matrix += ,@($id, 'fulldex', $enc) }
    if ($m.Groups[2].Value) { $matrix += ,@($id, 'gen1', $false) }
    if ($m.Groups[3].Value) { $matrix += ,@($id, 'nativeplat', $false) }
    if ($id -eq 'platinum' -and $m.Groups[1].Value) { $matrix += ,@($id, 'port', $false) }
  }
  if (-not $Full) {
    $ds = @('diamond','pearl','platinum','heartgold','soulsilver','black','white','black2','white2')
    $kinds = @{}; $pickRows = @()
    foreach ($row in $matrix) { $k = $row[1] + $(if ($row[1] -eq 'fulldex') { if ($ds -contains $row[0]) { '-ds' } else { '-gba' } }); if (-not $kinds.ContainsKey($k) -and -not $row[2]) { $kinds[$k] = 1; $pickRows += ,$row } }
    $matrix = $pickRows
  }
  foreach ($row in $matrix) {
    # (Fulldex's game list, and Gen1Recomp's window while it prepares a game, aren't the game.)
    $id = $row[0]; $en = $row[1]; $name = "$id on $en"; $script:played += @($name)
    if ($row[2]) { T play $name 'skip' 'ROM is encrypted: this needs the patch prompt'; continue }
    ClearGames
    Set-Content "$st\engine-$id.txt" $en
    $limit = if ($en -eq 'gen1') { 420 } elseif ($en -eq 'port') { 240 } else { 120 }
    $p = Start-Process $exe -ArgumentList "--play $id" -PassThru
    $t = Get-Date; $steady = $null; $game = $null; $bad = ''
    while (((Get-Date) - $t).TotalSeconds -lt $limit) {
      Start-Sleep 1
      if ($p.HasExited) { $bad = "launcher exited $($p.ExitCode) before the game settled"; break }
      $game = @(GameProcs | Where-Object { $_.MainWindowHandle -ne 0 -and $_.MainWindowTitle -and $_.MainWindowTitle -notmatch 'Fulldex' -and $_.MainWindowTitle -notmatch '^gen1recomp v' }) | Select-Object -First 1
      if ($game) { if (-not $steady) { $steady = Get-Date } elseif (((Get-Date) - $steady).TotalSeconds -ge 4) { break } } else { $steady = $null }
    }
    $took = [int]((Get-Date) - $t).TotalSeconds
    if ($game -and $steady -and -not $bad) {
      $title = $game.MainWindowTitle; $png = "$Report\shots\play-$id-$en.png"; $picture = 0; $w = Get-Date
      while (((Get-Date) - $w).TotalSeconds -lt 45) {
        try { [void][StableWin]::Shot($game.MainWindowHandle, $png); $picture = [StableWin]::Colours($png) } catch { $picture = 0 }
        if ($picture -ge 8) { break }
        try { [void][StableWin]::ScreenShot($game.MainWindowHandle, $png); $picture = [StableWin]::Colours($png); if ($picture -ge 8) { $title += ' (read from the screen)'; break } } catch { }
        Start-Sleep 2
      }
      $title += ", $picture colours after $([int]((Get-Date) - $w).TotalSeconds)s"
      [void]$game.CloseMainWindow()
      $clean = $p.WaitForExit(40000)
      if (-not $clean) { [void][StableWin]::Shot($p.MainWindowHandle, "$Report\shots\after-$id-$en.png"); foreach ($g in @(GameProcs)) { try { $g.Kill() } catch { } } ; try { if (-not $p.HasExited) { [void]$p.CloseMainWindow(); Start-Sleep 2; if (-not $p.HasExited) { $p.Kill() } } } catch { } }
      T play $name ($clean -and $picture -ge 8) "started in ${took}s, '$title'$(if (-not $clean) { '; launcher did not come back after the game closed' } elseif ($p.ExitCode -ne 0) { "; exit $($p.ExitCode)" })"
    } else {
      if (-not $p.HasExited) { $p.Refresh(); [void][StableWin]::Shot($p.MainWindowHandle, "$Report\shots\failed-$id-$en.png"); try { [void]$p.CloseMainWindow(); Start-Sleep 2; if (-not $p.HasExited) { $p.Kill() } } catch { } }
      T play $name $false $(if ($bad) { $bad } else { "no steady game window in ${limit}s" })
    }
    ClearGames
  }
  if ($SaveFolder) {
    $cfg = Join-Path $app 'engines\fulldex\settings.cfg'
    T play 'global options reach the engine: windowed' ((Test-Path $cfg) -and ((Get-Content $cfg) -contains 'fullscreen=0'))
    T play 'global options reach the engine: volume' ((Test-Path $cfg) -and ((Get-Content $cfg) -contains 'volume=0'))
  }
}
# ---------- sync: shared saves across engines. Needs the play stage to have prepared the game. ----------
if (On 'sync') {
  $g = @('firered','leafgreen','emerald') | Where-Object { (Test-Path (Join-Path $saves "$_.sav")) -and ($found -contains $_) } | Select-Object -First 1
  if (-not $g) { T sync 'shared save tests' 'skip' 'needs a FireRed, LeafGreen or Emerald save and ROM' } else {
    $o = "$Work\sync.txt"; $m = Join-Path $saves "$g.sav"; $record = "$st\save-sync\$g.txt"
    if ($script:played -notcontains "$g on gen1") {
      Set-Content "$st\engine-$g.txt" 'gen1'
      $q = Start-Process $exe -ArgumentList "--play $g" -PassThru; $w = Get-Date; $shown = $null
      while (((Get-Date) - $w).TotalSeconds -lt 420 -and -not $q.HasExited -and -not $shown) { Start-Sleep 1; $shown = @(GameProcs | Where-Object { $_.MainWindowHandle -ne 0 -and $_.MainWindowTitle }) | Select-Object -First 1 }
      T sync "${g}: prepared for Gen1Recomp" ([bool]$shown)
      Start-Sleep 5; if ($shown) { [void]$shown.CloseMainWindow() }; if (-not $q.WaitForExit(40000)) { try { $q.Kill() } catch { } }; ClearGames
    }
    [void](Launcher @('--sync',$g,$o) 180); $one = (Get-Content $o -Raw -ErrorAction SilentlyContinue) -split "`n" | Select-Object -First 1
    T sync "${g}: first sync completes" ($one -in 'level','changed') "$one"
    # Without this, a game that isn't sharing at all would pass every check below.
    T sync "${g}: its save really is shared between the engines" (Test-Path $record) $(if (-not (Test-Path $record)) { 'no sync record: the checks below would prove nothing' })
    [void](Launcher @('--sync',$g,$o) 180); $two = (Get-Content $o -Raw -ErrorAction SilentlyContinue) -split "`n" | Select-Object -First 1
    T sync "${g}: nothing changed, so a second sync does nothing" ($two -eq 'level') "$two"
    $goodCopy = "$Work\good.sav"; Copy-Item $m $goodCopy
    $record = "$st\save-sync\$g.txt"
    function Sync1 { [void](Launcher @('--sync',$g,$o) 180); ((Get-Content $o -Raw -ErrorAction SilentlyContinue) -split "`n" | Select-Object -First 1).Trim() }
    function Hurt([int]$from, [int]$to) { $bytes = [IO.File]::ReadAllBytes($m); for ($sector = $from; $sector -lt $to; $sector += 0x1000) { for ($i = $sector + 0x100; $i -lt $sector + 0x300; $i++) { $bytes[$i] = $bytes[$i] -bxor 0x5A } }; [IO.File]::WriteAllBytes($m, $bytes) }
    # Both of a GBA save's copies ruined: nothing in it can be trusted, and the other engine's save must stay as it was.
    $gen1Before = (Get-Content $record -ErrorAction SilentlyContinue | Where-Object { $_ -match '^gen1=' })
    $recordBefore = Get-Content $record -Raw -ErrorAction SilentlyContinue
    Hurt 0 0x1C000
    $badCheck = (Bridge check $m).Code
    T sync "${g}: a ruined save is seen as damaged" ($badCheck -ne 0) "check $badCheck"
    $three = Sync1
    $recordAfter = Get-Content $record -Raw -ErrorAction SilentlyContinue
    T sync "${g}: a ruined save isn't spread to the other engine" ($three -in 'kept','conflict' -and $recordBefore -eq $recordAfter) "$three; record $(if ($recordBefore -eq $recordAfter) { 'unchanged' } else { 'CHANGED' })"
    Copy-Item $goodCopy $m -Force
    $four = Sync1
    T sync "${g}: with the save restored, sync is level" ($four -eq 'level') "$four"
    # One copy ruined: the game itself would load the other, and the launcher may too, as long as what it shares is sound.
    Hurt 0 0xE000
    $oneCheck = (Bridge check $m).Code; $five = Sync1
    T sync "${g}: one copy ruined: it's kept back, or the sound copy is used" (($five -in 'kept','conflict') -or ($oneCheck -eq 0 -and $five -in 'changed','level')) "check $oneCheck, $five"
    Copy-Item $goodCopy $m -Force
    $six = Sync1; $seven = Sync1
    T sync "${g}: restored again, it settles" ($seven -eq 'level') "$six then $seven"
    # A small patch of damage. A save that fails its checks must never go across: only one that still reads sound may.
    $bytes = [IO.File]::ReadAllBytes($m); for ($i = 0x100; $i -lt 0x1100; $i++) { $bytes[$i] = $bytes[$i] -bxor 0x5A }; [IO.File]::WriteAllBytes($m, $bytes)
    $smallCheck = (Bridge check $m).Code; $same = (Bridge compare $goodCopy $m).Code; $eight = Sync1
    T sync "${g}: small damage: nothing unsound is shared" (($eight -in 'kept','conflict','level') -or ($smallCheck -eq 0 -and $eight -eq 'changed')) "check $smallCheck, same as good copy: $($same -eq 0), sync $eight"
    Copy-Item $goodCopy $m -Force; [void](Sync1); $nine = Sync1
    T sync "${g}: restored after that too, it settles" ($nine -eq 'level') "$nine"
    $bk = @(Get-ChildItem "$st\save-backups" -Recurse -File -ErrorAction SilentlyContinue).Count
    T sync 'backups were kept along the way' ($bk -gt 0) "$bk files"
  }
}
# ---------- window: real key presses through the real window ----------
if (On 'window') {
  ClearGames
  $pagesLog = "$Work\pages.log"; $env:NATIVEMONS_TEST_LOG = $pagesLog
  $p = Start-Process $exe -PassThru
  $t = Get-Date; do { Start-Sleep -Milliseconds 400; $p.Refresh() } while ($p.MainWindowHandle -eq 0 -and ((Get-Date) - $t).TotalSeconds -lt 40); $p.Refresh(); $h = $p.MainWindowHandle
  $w = Get-Date; do { Start-Sleep -Milliseconds 250 } while ((Get-Content $pagesLog -Tail 1 -ErrorAction SilentlyContinue) -notmatch '^1\|nativeMons' -and ((Get-Date) - $w).TotalSeconds -lt 40); Start-Sleep 1
  $U=0x26;$D=0x28;$L=0x25;$R=0x27;$E=0x0D;$B=0x1B
  function Alive { $p.Refresh(); (-not $p.HasExited) -and $p.Responding }
  function Now { Get-Content $pagesLog -Tail 1 -ErrorAction SilentlyContinue }
  function WaitPage([string]$rx, [int]$secs = 30) { $w = Get-Date; do { if ((Now) -match $rx) { return $true }; Start-Sleep -Milliseconds 250 } while (((Get-Date) - $w).TotalSeconds -lt $secs); return $false }
  # The launcher ignores keys while it's busy, so each step waits to see the screen it expects, by name.
  function Visit([string]$name, [string]$rx, [int]$depth) {
    [StableWin]::Key($h,$E)
    $ok = WaitPage "^$depth\|$rx"
    [void][StableWin]::Shot($h, "$Report\shots\win-$($name -replace ' ','-').png")
    $alive = Alive; T window "opens $name" ($ok -and $alive) "$(Now)$(if (-not $alive) { '; not responding, exited: ' + $p.HasExited })"
    $n = 0; do { [StableWin]::Key($h,$B); $back = WaitPage "^$($depth - 1)\|" 6; $n++ } while (-not $back -and $n -lt 4)
    if (-not $back) { T window "back from $name" $false "$(Now)" }
  }
  T window 'starts on the home screen' ((Alive) -and (WaitPage '^1\|nativeMons' 10)) "$(Now)"
  [StableWin]::Key($h,$D); Visit 'Games' 'Games$' 2
  [StableWin]::Key($h,$R); Visit 'Collection' 'Pokedex$' 2
  [StableWin]::Key($h,$R); Visit 'Mods' 'Mods' 2
  [StableWin]::Key($h,$R); Visit 'ROMs' 'ROM folders$' 2
  1..3 | ForEach-Object { [StableWin]::Key($h,$L) }; [StableWin]::Key($h,$D); Visit 'Move' 'Move Pokemon' 2
  [StableWin]::Key($h,$R); [StableWin]::Key($h,$E)
  T window 'opens Settings' (WaitPage '^2\|Settings$') "$(Now)"
  # Every row but the ones that download or open a file dialog.
  $rows = @(@('Fulldex settings','Fulldex$'), @('Gen1Recomp settings','Gen1Recomp$'), $null, @('Export saves','Export saves'), $null, @('Game options','Game options$'), @('Updates','Updates$'), @('Credits','Credits$'), @('Last session','Last session$'))
  for ($i = 0; $i -lt $rows.Count; $i++) { if ($rows[$i]) { Visit $rows[$i][0] $rows[$i][1] 3 }; if ($i -lt $rows.Count - 1) { [StableWin]::Key($h,$D) } }
  [StableWin]::Key($h,$B); T window 'back on the home screen' (WaitPage '^1\|nativeMons' 10) "$(Now)"
  # Change a ROM version through its page, if a game has two: from a fresh start, so the keys land where they're meant to.
  $idx = [array]::IndexOf($found, 'platinum')
  $two = @($romLines | Where-Object { $_ -match '^platinum found=([2-9])' }).Count -gt 0
  $pick = "$st\rom-platinum.txt"
  if ($two -and $idx -gt 0 -and (Alive)) {
    [void]$p.CloseMainWindow(); [void]$p.WaitForExit(8000)
    $p = Start-Process $exe -PassThru
    Remove-Item $pagesLog -ErrorAction SilentlyContinue
    $t = Get-Date; do { Start-Sleep -Milliseconds 400; $p.Refresh() } while ($p.MainWindowHandle -eq 0 -and ((Get-Date) - $t).TotalSeconds -lt 40); $p.Refresh(); $h = $p.MainWindowHandle
    [void](WaitPage '^1\|nativeMons' 40); Start-Sleep 1
    $before = Get-Content $pick
    [StableWin]::Key($h,$D); [StableWin]::Key($h,$E); Start-Sleep 1
    1..$idx | ForEach-Object { [StableWin]::Key($h,$D) }
    [void][StableWin]::Shot($h,"$Report\shots\win-list.png"); [StableWin]::Key($h,$E); Start-Sleep 1
    [void][StableWin]::Shot($h,"$Report\shots\win-gamepage.png")
    1..2 | ForEach-Object { [StableWin]::Key($h,$D) }; [StableWin]::Key($h,$E); Start-Sleep 1
    [void][StableWin]::Shot($h,"$Report\shots\win-picker.png"); [StableWin]::Key($h,$E); Start-Sleep 10
    $after = Get-Content $pick
    T window 'ROM version: choosing another saves it' ($after -ne $before) "$($before.Substring(0,8)) to $($after.Substring(0,8))"
    [void][StableWin]::Shot($h,"$Report\shots\win-after-version.png")
    T window 'ROM version: still running afterwards' (Alive)
  } else { T window 'ROM version' 'skip' 'needs two Platinum dumps' }
  # No more key presses here: Play would be the selected row.
  $games = @(GameProcs)
  T window 'nothing was started by accident' ($games.Count -eq 0) (($games | ForEach-Object ProcessName) -join ',')
  ClearGames
  if (-not $p.HasExited) { [void]$p.CloseMainWindow(); if (-not $p.WaitForExit(8000)) { T window 'closes cleanly' $false; $p.Kill() } else { T window 'closes cleanly' $true } }
}
# ---------- updater: a fake old install, updated from local releases on both channels ----------
if (On 'updater') {
  ClearGames
  $env:NATIVEMONS_STATE = $null; Remove-Item env:NATIVEMONS_STATE -ErrorAction SilentlyContinue
  $old = "$Work\old"; $rel = "$Work\releases"
  New-Item -ItemType Directory -Force "$rel\stable","$rel\bleeding-edge" | Out-Null
  $tag = Split-Path $Package -Leaf
  Copy-Item -LiteralPath $Package "$rel\stable\$tag"; Copy-Item -LiteralPath $Package "$rel\bleeding-edge\$tag"
  $sha = (Get-FileHash -LiteralPath $Package -Algorithm SHA256).Hash.ToLower()
  function Rel([string]$c, [int]$b, [string]$r, [string]$s = $sha) { Set-Content "$rel\$c\release.json" (@{ channel = $c; build = $b; revision = $r; package = $tag; sha256 = $s } | ConvertTo-Json) -Encoding Ascii }
  $env:NATIVEMONS_UPDATE_URL = 'file:///' + ($rel -replace '\\','/') + '/'
  New-Item -ItemType Directory -Force "$old\state\save-backups","$old\engines\fulldex\saves","$old\engines\fulldex\roms","$old\engines\nativeplat\userdata\saves\diamond","$old\engines\gen1recomp\src\old","$old\engines\pokeplatinum" | Out-Null
  $mine = @{ 'state\rom-folders.txt'='I:\none'; 'state\save-backups\keep.sav'='backup'; 'engines\fulldex\saves\platinum.sav'='SAVE'; 'engines\fulldex\roms\x.gba'='ROMCOPY'; 'engines\fulldex\settings.cfg'='volume=33'; 'engines\nativeplat\userdata\saves\diamond\nativemons.sav'='NPSAVE'; 'engines\nativeplat\userdata\options.ini'='volume = 7'; 'engines\pokeplatinum\save.bin'='PORTSAVE' }
  foreach ($k in $mine.Keys) { Set-Content "$old\$k" $mine[$k] -NoNewline }
  Set-Content "$old\engines\gen1recomp\src\old\stale.lua" 'stale'; Set-Content "$old\games.json" '[]'
  $oexe = "$old\NativeMonsLauncher.exe"
  function Ver([string]$c, [int]$b, [string]$r) { Set-Content "$old\version.txt" "channel=$c`nbuild=$b`nrevision=$r" -Encoding Ascii }
  function Up { Copy-Item -LiteralPath $exe $oexe -Force; Remove-Item "$oexe.old","$Work\u.txt" -ErrorAction SilentlyContinue; $q = Start-Process $oexe -ArgumentList "--update `"$Work\u.txt`"" -PassThru -WindowStyle Hidden; if (-not $q.WaitForExit(540000)) { $q.Kill(); return 'TIMED OUT' }; (Get-Content "$Work\u.txt" -ErrorAction SilentlyContinue) -join ' | ' }
  function Channel { ((Get-Content "$old\version.txt") -join ' ') }

  Ver 'bleeding-edge' 1 'aaaa'; Rel 'bleeding-edge' 100 'bbbb'
  $r = Up; T updater 'dev: a newer build is applied' ($r -match 'ready True' -and $r -match 'result ok' -and (Channel) -match 'channel=bleeding-edge build=100') "$r"
  $kept = @($mine.Keys | Where-Object { (Get-Content "$old\$_" -Raw) -ne $mine[$_] })
  T updater 'saves, ROM copies, settings and state are kept' ($kept.Count -eq 0) ($kept -join ', ')
  foreach ($f in 'engines\fulldex\Pokemon Fulldex.exe','engines\nativeplat\nativeplat.exe','engines\gen1recomp\main.lua','love\lovec.exe','tools\mon-bridge\mon-bridge.exe') { T updater "installs $f" (Test-Path "$old\$f") }
  T updater 'files gone from the release are removed' (-not (Test-Path "$old\engines\gen1recomp\src\old\stale.lua"))
  T updater 'its work folder is cleaned up' (-not (Test-Path "$old\state\update"))
  T updater 'games.json came from the release' ((Get-Item "$old\games.json").Length -gt 100)
  $r = Up; T updater 'dev: same build, nothing to do' ($r -match 'ready False' -and (Channel) -match 'build=100') "$r"

  Ver 'bleeding-edge' 100 'bbbb'; Rel 'stable' 5 'cccc'; Set-Content "$old\state\update-channel.txt" 'stable'
  $r = Up; T updater 'switch to Stable works though its build number is lower' ($r -match 'ready True' -and (Channel) -match 'channel=stable build=5') "$r"
  $kept = @($mine.Keys | Where-Object { (Get-Content "$old\$_" -Raw) -ne $mine[$_] })
  T updater 'still kept after switching channel' ($kept.Count -eq 0) ($kept -join ', ')
  $r = Up; T updater 'Stable: up to date afterwards' ($r -match 'ready False') "$r"
  if ($Full) { Rel 'stable' 6 'dddd'; $r = Up; T updater 'Stable: a newer stable build is applied' ($r -match 'ready True' -and (Channel) -match 'build=6') "$r" } else { Ver 'stable' 6 'dddd' }
  Set-Content "$old\state\update-channel.txt" 'bleeding-edge'; Rel 'bleeding-edge' 100 'dddd'
  $r = Up; T updater 'switch back to Dev at the same commit: nothing to download' ($r -match 'ready False') "$r"
  Set-Content "$old\state\update-channel.txt" 'stable'
  Rel 'stable' 7 'eeee' ('0' * 64); $before = Channel
  $r = Up; T updater 'a download failing its hash changes nothing' ($r -notmatch 'result ok' -and (Channel) -eq $before) "$r"
  $kept = @($mine.Keys | Where-Object { (Get-Content "$old\$_" -Raw) -ne $mine[$_] })
  T updater 'still kept after the failed update' ($kept.Count -eq 0) ($kept -join ', ')
  Remove-Item "$rel\stable\release.json"
  $r = Up; T updater 'no release to read: reports it, changes nothing' ($r -notmatch 'result ok' -and (Channel) -eq $before) "$r"
  Remove-Item env:NATIVEMONS_UPDATE_URL -ErrorAction SilentlyContinue
}
# ---------- nothing of the player's changed ----------
$env:APPDATA = $oldAppData
T integrity 'ROM folder is exactly as it was' ((($romsBefore) -join "`n") -eq ((RomList) -join "`n")) "$(@($romsBefore).Count) ROMs"
foreach ($k in $saveHashes.Keys) { if ($saveHashes[$k] -ne (Sha $k)) { T integrity "source save changed: $k" $false } }
if ($saveHashes.Count) { T integrity 'source saves are exactly as they were' $true "$($saveHashes.Count) saves" }
$left = @(GameProcs); T integrity 'no game left running' ($left.Count -eq 0) (($left | ForEach-Object ProcessName) -join ',')
} catch {
  # A suite that stopped early has not passed anything.
  # What it finished doesn't make the group it stopped in a pass, or -Retest would leave it out.
  if ($script:lastGroup -and $script:lastGroup -ne 'suite') { T $script:lastGroup 'stopped before finishing' $false }
  T suite 'ran to the end' $false "$($_.Exception.Message) at line $($_.InvocationInfo.ScriptLineNumber)"
} finally {
  $env:APPDATA = $oldAppData; Remove-Item env:NATIVEMONS_STATE, env:NATIVEMONS_UPDATE_URL, env:NATIVEMONS_TEST_LOG, env:NATIVEMONS_TEST_QUIET -ErrorAction SilentlyContinue
  try { ClearGames; foreach ($p in Get-Process NativeMonsLauncher -ErrorAction SilentlyContinue) { if ($p.Path -and $p.Path.StartsWith($Work, [StringComparison]::OrdinalIgnoreCase)) { $p.Kill() } } } catch { }
}
# The verdict covers every group's latest result, not only this run's.
$lines = @(foreach ($g in $AllGroups + 'suite' + 'integrity') { if (Test-Path (GroupFile $g)) { Get-Content (GroupFile $g) | ForEach-Object { $f = $_ -split "`t", 3; '{0,-4} {1,-10} {2}{3}' -f $f[0], $g, $f[1], $(if ($f[2]) { "  [$($f[2])]" }) } } })
$pass = @($lines | Where-Object { $_ -like 'PASS*' }).Count; $fail = @($lines | Where-Object { $_ -like 'FAIL*' }).Count; $skipped = @($lines | Where-Object { $_ -like 'SKIP*' }).Count
$notRun = @($AllGroups | Where-Object { (GroupState $_) -eq 'NOT RUN' })
$failedGroups = @($AllGroups + 'suite' + 'integrity' | Where-Object { (GroupState $_) -eq 'FAILED' })
$manual = @(
  'Play a real save in Fulldex, then Gen1Recomp, and back, checking story progress and map position',
  'A controller through every screen', 'New save and import name dialogs, and the file dialogs',
  'Encrypted ROM: the patch prompt at import', 'Mod zip install', 'Open in PKHeX and OpenHome, then Bring back',
  'A romhack played, saved and resumed through the launcher', 'Restore an exported save on a real 3DS')
$end = @('', 'Groups:') + (Overview | ForEach-Object { "  $_" }) + @('', "PASS $pass  FAIL $fail  SKIP $skipped", 'Needs a person, not covered above:') + ($manual | ForEach-Object { "  - $_" })
if ($LauncherOverride) { $end += 'The launcher was overridden: run again on the published package before releasing.' }
$ready = $fail -eq 0 -and $pass -gt 0 -and $notRun.Count -eq 0
$quickGroups = @('play','snapshots','updater' | Where-Object { Test-Path "$(GroupFile $_).quick" })
if ($failedGroups) { $end += "Retest with -Retest, which runs: $(@($failedGroups + $notRun | Select-Object -Unique) -join ' ')" }
$end += $(if ($ready -and $quickGroups) { "VERDICT: no automated failures, $skipped skipped, in a quick run of $($quickGroups -join ' '). A release needs those with -Full." } elseif ($ready -and $LauncherOverride) { "VERDICT: no automated failures, $skipped skipped, on a local launcher. Not a release decision." } elseif ($ready) { "VERDICT: no automated failures, $skipped skipped." } elseif ($fail -eq 0 -and $notRun.Count) { "VERDICT: NOT READY. Not run yet: $($notRun -join ' ')" } else { 'VERDICT: NOT READY.' })
Set-Content $log (@($header) + $lines + $end)
$end | ForEach-Object { Write-Output $_ }
if (-not $Keep) { Remove-Item $Work -Recurse -Force -ErrorAction SilentlyContinue }
exit $(if ($ready) { 0 } else { 1 })

# The C# -> Rust switchover, headless: a C# install (the app's program files, test saves and settings, no player
# data) takes an update package whose NativeMonsLauncher.exe is the Rust launcher, through the C# exe's own --update
# against a local fake release. Then the Rust and the C# exe each read the updated install (same path, one after
# the other) with --roms, --saves list and --sync, and their result files and the state they leave are compared
# byte for byte. No window, no game; ROMs and saves are copied (originals hashed before and after).
# switchover-test.ps1 -Exe <Rust exe, the `shipped` build> [-Root <scratch folder>] [-Keep]
# -Live (needs the user's OK: windows on monitor 3 for about a minute): step 3 goes through the C# window instead,
# as a player does (Settings > Updates > Update now, by posted keys), then the restart into the Rust exe, its first
# start on the install (the lock wait, .old removed, home), a plain start of it by the shell (no console), and a
# C# start on a copy of the install from before the update as the control for what a start writes. Step 4 is skipped.
param([string]$Exe = "", [string]$Root = "", [switch]$Keep, [switch]$Live)
$ErrorActionPreference = "Stop"
. "$PSScriptRoot\work.ps1"
if (-not $Exe) { $Exe = "$TargetDir\release\nativemons.exe" }
if (-not $Root) { $Root = "$Work\switchover" }
$csExe = "$App\NativeMonsLauncher.exe"
foreach ($f in $Exe, $csExe) { if (-not (Test-Path -LiteralPath $f)) { throw "missing $f" } }
$originals = @{ red = (Rom-Path "red"); firered = (Rom-Path "firered"); frSave = "$TestData\billspc\firered.sav"; redSave = "$TestData\billspc\red.sav" }
function Sha([string]$f) { (Get-FileHash -LiteralPath $f -Algorithm SHA256).Hash }
$before = @{}; foreach ($k in $originals.Keys) { $before[$k] = Sha $originals[$k] }
if (Test-Path $Root) { Remove-Item -Recurse -Force $Root }
$install = "$Root\run"; $roms = "$Root\roms"; $report = New-Object Collections.Generic.List[string]
New-Item -ItemType Directory -Force $install, $roms, "$Root\appdata", "$Root\results" | Out-Null
function Say([string]$s) { $report.Add($s); Write-Host $s }

# The program files of a package, from the app folder: no state, saves, ROM copies, game data or engine settings.
function Copy-Program([string]$to) {
    New-Item -ItemType Directory -Force $to | Out-Null
    foreach ($f in 'NativeMonsLauncher.exe', 'games.json', 'LICENSE', 'README.md', 'nativemons.sh') { Copy-Item -LiteralPath "$App\$f" $to }
    foreach ($d in 'fonts', 'docs', 'love', 'tools') { robocopy "$App\$d" "$to\$d" /E /NFL /NDL /NJH /NJS /NP | Out-Null }
    $fd = "$App\engines\fulldex"
    robocopy "$App\engines" "$to\engines" /E /XD "$fd\saves" "$fd\roms" "$fd\states" "$fd\cheats" "$fd\3ds" "$App\engines\nativeplat\userdata" "$App\engines\pokeplatinum" `
        /XF settings.cfg game.log fulldex-playtime.txt last-game.txt gba-settings.ini /NFL /NDL /NJH /NJS /NP | Out-Null
    # The Platinum port: the program only (its other files come from the player's ROM).
    New-Item -ItemType Directory -Force "$to\engines\pokeplatinum" | Out-Null
    Get-ChildItem -LiteralPath "$App\engines\pokeplatinum" -File -Force | Where-Object { $_.Extension -in '.exe', '.dll' -or $_.Name -in 'LICENSE-libntr.txt', '.nativemons-version' } | Copy-Item -Destination "$to\engines\pokeplatinum"
}
function Run-Exe([string]$exe, [string[]]$argv) {
    $psi = New-Object Diagnostics.ProcessStartInfo $exe
    $psi.Arguments = ($argv | ForEach-Object { '"' + $_ + '"' }) -join ' '
    $psi.UseShellExecute = $false; $psi.CreateNoWindow = $true; $psi.WorkingDirectory = Split-Path $exe
    $p = [Diagnostics.Process]::Start($psi)
    if (-not $p.WaitForExit(600000)) { $p.Kill(); return -999 }
    $p.ExitCode
}
# Every file under a folder: relative path -> sha256.
function Tree([string]$dir) {
    $t = @{}; Get-ChildItem -LiteralPath $dir -Recurse -File -Force | ForEach-Object { $t[$_.FullName.Substring($dir.Length + 1)] = Sha $_.FullName }; $t
}
function Diff-Trees($a, $b) {
    @($a.Keys + $b.Keys | Sort-Object -Unique | Where-Object { $a[$_] -ne $b[$_] } | ForEach-Object { "$_ ($(if (-not $a.ContainsKey($_)) { 'new' } elseif (-not $b.ContainsKey($_)) { 'gone' } else { 'changed' }))" })
}

# 1. The C# install, with test saves and settings, used a little by the C# launcher.
Copy-Program $install
Set-Content -LiteralPath "$install\version.txt" -Encoding ascii -Value "channel=bleeding-edge`nbuild=178`nrevision=before-switch`nversion=0.0.8d"
Copy-Item -LiteralPath $originals.red, $originals.firered $roms
$st = "$install\state"; New-Item -ItemType Directory -Force $st, "$install\engines\fulldex\saves", "$install\engines\nativeplat\userdata", "$Root\inputs" | Out-Null
Set-Content -LiteralPath "$st\rom-folders.txt" -Value $roms -Encoding utf8
$settings = @{ 'layout.txt' = 'phone'; 'game-options.txt' = "text=fast`r`nscene=on`r`nvol=60"; 'discord.txt' = 'off'; '3ds-games.txt' = 'on'; '3ds-emulator.txt' = 'azaharplus'; 'wallpaper.txt' = 'play'; 'engine-platinum.txt' = 'port'; 'update-dismissed.txt' = '26' }
if ($Live) { $settings['quick-start.txt'] = 'off'; $settings['last-game.txt'] = 'firered' }
foreach ($k in $settings.Keys) { Set-Content -LiteralPath "$st\$k" -Value $settings[$k] -Encoding ascii }
Copy-Item -LiteralPath $originals.frSave "$install\engines\fulldex\saves\firered.sav"
Copy-Item -LiteralPath $originals.redSave "$Root\inputs\red.sav"
Set-Content -LiteralPath "$install\engines\fulldex\settings.cfg" -Value "volume=33" -Encoding ascii
Set-Content -LiteralPath "$install\engines\nativeplat\userdata\options.ini" -Value "volume = 7" -Encoding ascii
$env:APPDATA = "$Root\appdata"; $env:LOCALAPPDATA = "$Root\appdata\local"; New-Item -ItemType Directory -Force $env:LOCALAPPDATA | Out-Null
Remove-Item env:NATIVEMONS_STATE, env:NATIVEMONS_UPDATE_URL -ErrorAction SilentlyContinue
$use = @(@("--roms", "<out>"), @("--saves", "firered", "<out>", "list"), @("--saves", "firered", "<out>", "new", "Second"), @("--sync", "firered", "<out>"), @("--saves", "red", "<out>", "list"))
$n = 0; foreach ($a in $use) { $n++; $e = Run-Exe "$install\NativeMonsLauncher.exe" @($a | ForEach-Object { $_ -replace '<out>', "$Root\results\use-$n.txt" }); Say ("C# use {0,-28} exit {1}: {2}" -f ($a -join ' ' -replace [regex]::Escape($Root), ''), $e, ((Get-Content "$Root\results\use-$n.txt" -ErrorAction SilentlyContinue) -join ' / ')) }

# 2. The update package: the same program files with the Rust launcher in the C#'s place (package-rust.ps1).
$pkg = "$Root\pkg-src\nativeMons"; Copy-Program $pkg
& "$PSScriptRoot\package-rust.ps1" -Out $pkg -Exe $Exe | Out-Null
$rel = "$Root\releases\bleeding-edge"; New-Item -ItemType Directory -Force $rel | Out-Null
Add-Type -AssemblyName System.IO.Compression.FileSystem
[IO.Compression.ZipFile]::CreateFromDirectory("$Root\pkg-src", "$rel\nativeMons.zip", [IO.Compression.CompressionLevel]::Fastest, $false)
$zsha = (Sha "$rel\nativeMons.zip").ToLowerInvariant()
Set-Content -LiteralPath "$rel\release.json" -Encoding ascii -Value ('{"channel":"bleeding-edge","build":179,"revision":"after-switch","version":"0.0.8d","package":"nativeMons.zip","sha256":"' + $zsha + '"}')
Say "package: $([math]::Round((Get-Item "$rel\nativeMons.zip").Length / 1MB)) MB zip, $((Get-ChildItem $pkg -Recurse -File).Count) files"

# 3. The C# launcher updates itself, as from Settings > Updates, with no window.
$treeBefore = Tree $install
if ($Live) {
    Add-Type -Namespace SW -Name U -MemberDefinition @"
public delegate bool EnumProc(System.IntPtr h, System.IntPtr l);
[DllImport("user32.dll")] public static extern bool PostMessageW(System.IntPtr h, uint m, System.IntPtr w, System.IntPtr l);
[DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, System.IntPtr l);
[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(System.IntPtr h, out uint pid);
[DllImport("user32.dll")] public static extern bool IsWindowVisible(System.IntPtr h);
[DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassNameW(System.IntPtr h, System.Text.StringBuilder s, int n);
[DllImport("user32.dll")] public static extern bool GetWindowRect(System.IntPtr h, out RECT r);
public struct RECT { public int L, T, R, B; }
"@
    $log = "$Root\live.log"
    # Every visible top-level window of these processes: "pid class x,y wxh".
    function Windows-Of([int[]]$pids) {
        $script:found = @()
        [void][SW.U]::EnumWindows({ param($h, $l) $p = [uint32]0; [void][SW.U]::GetWindowThreadProcessId($h, [ref]$p); if ($pids -contains [int]$p -and [SW.U]::IsWindowVisible($h)) { $c = New-Object Text.StringBuilder 256; [void][SW.U]::GetClassNameW($h, $c, 256); $r = New-Object SW.U+RECT; [void][SW.U]::GetWindowRect($h, [ref]$r); $script:found += [pscustomobject]@{ pid = [int]$p; hwnd = $h; cls = "$c"; rect = "$($r.L),$($r.T) $($r.R - $r.L)x$($r.B - $r.T)" } }; return $true }, [IntPtr]::Zero)
        $script:found
    }
    # Console hosts that are children of these processes (a console window would come with one).
    function Consoles-Of([int[]]$pids) { @(Get-CimInstance Win32_Process -Filter "Name='conhost.exe' OR Name='OpenConsole.exe' OR Name='WindowsTerminal.exe'" | Where-Object { $pids -contains [int]$_.ParentProcessId }) }
    function Key($h, [int]$vk) { [void][SW.U]::PostMessageW($h, 0x0100, [IntPtr]$vk, [IntPtr]::Zero); Start-Sleep -Milliseconds 350 }
    function Last-Page { (Get-Content -LiteralPath $log -ErrorAction SilentlyContinue | Where-Object { $_ -match '^\d+\|' -or $_ -eq 'job' } | Select-Object -Last 1) }
    function Wait-Line([string]$rx, [int]$s) { $t = Get-Date; do { if (Get-Content -LiteralPath $log -ErrorAction SilentlyContinue | Where-Object { $_ -match $rx } | Select-Object -First 1) { return $true }; Start-Sleep -Milliseconds 200 } while (((Get-Date) - $t).TotalSeconds -lt $s); $false }
    function Wait-Page([string]$rx, [int]$s) { $t = Get-Date; do { if ("$(Last-Page)" -match $rx) { return $true }; Start-Sleep -Milliseconds 200 } while (((Get-Date) - $t).TotalSeconds -lt $s); $false }
    function Start-Launcher([string]$exe, [bool]$shell) {
        $psi = New-Object Diagnostics.ProcessStartInfo $exe; $psi.WorkingDirectory = Split-Path $exe
        $psi.UseShellExecute = $shell; if (-not $shell) { $psi.CreateNoWindow = $true }
        [Diagnostics.Process]::Start($psi)
    }
    function Launchers { @(Get-Process -Name NativeMonsLauncher -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "$Root\*" }) }
    $env:NATIVEMONS_UPDATE_URL = 'file:///' + ($Root -replace '\\', '/') + '/releases/'
    $env:NATIVEMONS_TEST_SCREEN = "DISPLAY3"; $env:NATIVEMONS_TEST_LOG = $log; $env:NATIVEMONS_TEST_NO_CAPTURE = "1"
    $env:NATIVEMONS_TEST_DISCORD = "$Root\results\discord.txt"; $env:NATIVEMONS_TEST_FRAMES = "$Root\frames"
    robocopy $install "$Root\pre-update" /E /NFL /NDL /NJH /NJS /NP | Out-Null
    $onScreen = [Diagnostics.Stopwatch]::StartNew()
    try {
        # C#: home, Settings (home's third row), Updates, Update now; the restart starts the Rust exe and closes.
        $cs = Start-Launcher "$install\NativeMonsLauncher.exe" $false
        $ok = Wait-Page '^1\|nativeMons' 60; Start-Sleep 2
        $t = Get-Date; do { $cs.Refresh(); $h = $cs.MainWindowHandle; if ($h -eq [IntPtr]::Zero) { Start-Sleep -Milliseconds 200 } } while ($h -eq [IntPtr]::Zero -and ((Get-Date) - $t).TotalSeconds -lt 10)
        Say "C# window: $(@(Windows-Of @($cs.Id) | ForEach-Object { "$($_.cls) at $($_.rect)" }) -join '; '); home $ok"
        if ($ok -and $h -ne [IntPtr]::Zero) {
            Key $h 0x28; Key $h 0x28; Key $h 0x27; Key $h 0x27; Key $h 0x0D
            $ok = Wait-Page '^2\|Settings$' 10
        }
        # Settings rows above Updates on this install: Fulldex, Gen1Recomp, 3DS games, the two Zakuro rows, Import,
        # Export, Download Pokedex, Game options, Wallpapers, Controls, Screenshots, Wide layout, Quick start, Discord.
        if ($ok) { 1..15 | ForEach-Object { Key $h 0x28 }; Start-Sleep -Milliseconds 400; Key $h 0x0D; $ok = Wait-Page '^3\|Updates$' 20 }
        Say "C# pages: $((Get-Content $log | Where-Object { $_ -match '^\d+\|' -or $_ -eq 'job' }) -join ' > ')"
        if (-not $ok) { Say "C#: did not reach Updates (last: $(Last-Page)); stopped here"; Key $h 0x1B; Key $h 0x1B; Key $h 0x1B }
        else {
            Start-Sleep 1; Key $h 0x0D
            $clicked = Get-Date
            if (-not $cs.WaitForExit(120000)) { Say "C# did not close within 120 s" }
            $csGone = Get-Date
            Say ("C# closed {0:0.0} s after Update now (exit {1})" -f ($csGone - $clicked).TotalSeconds, $cs.ExitCode)
            $came = Wait-Line '^\d+ page 1\|nativeMons' 30
            $rs = Launchers | Select-Object -First 1
            $hwndLine = Get-Content $log | Where-Object { $_ -match '^\d+ hwnd ' } | Select-Object -First 1
            Say "Rust came up by itself: $came (process $(if ($rs) { $rs.Id } else { 'none' }), $hwndLine)"
            Start-Sleep 4
            if ($rs) {
                Say "Rust windows: $(@(Windows-Of @($rs.Id) | ForEach-Object { "$($_.cls) at $($_.rect)" }) -join '; '); console hosts under it: $((Consoles-Of @($rs.Id)).Count)"
                Say "Rust start: $((Get-Content $log | Where-Object { $_ -match '^\d+ (display|page|items 0: |pads)' } | Select-Object -First 5) -join ' / ')"
                Say "  .old still there: $(Test-Path "$install\NativeMonsLauncher.exe.old"); version.txt: $((Get-Content "$install\version.txt") -join ' ')"
                $rh = [IntPtr][long](($hwndLine -split ' ')[2]); [void][SW.U]::PostMessageW($rh, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)
                if (-not $rs.WaitForExit(15000)) { Say "Rust did not close within 15 s: stopped"; $rs.Kill() } else { Say "Rust closed by WM_CLOSE: $((Get-Content $log | Where-Object { $_ -match '^\d+ exit ' } | Select-Object -Last 1))" }
            }
        }
        $treeLive = Tree $install
        # The shipped exe started as Explorer starts it (shell execute, no CreateNoWindow): no console may come with it.
        $consBefore = @(Get-Process conhost, OpenConsole, WindowsTerminal -ErrorAction SilentlyContinue | ForEach-Object Id)
        $direct = Start-Launcher "$install\NativeMonsLauncher.exe" $true
        Start-Sleep 5
        $wins = @(Windows-Of @($direct.Id)); $kids = Consoles-Of @($direct.Id)
        $newCons = @(Get-Process conhost, OpenConsole, WindowsTerminal -ErrorAction SilentlyContinue | Where-Object { $consBefore -notcontains $_.Id })
        Say "shell start: windows $(($wins | ForEach-Object { "$($_.cls) at $($_.rect)" }) -join '; '); console windows: $(@($wins | Where-Object { $_.cls -match 'Console|CASCADIA' }).Count); console hosts under it: $($kids.Count); new console processes anywhere: $($newCons.Count)"
        foreach ($w in $wins) { [void][SW.U]::PostMessageW($w.hwnd, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero) }
        if (-not $direct.WaitForExit(15000)) { $direct.Kill(); Say "shell start: did not close, stopped" } else { Say "shell start closed (exit $($direct.ExitCode))" }
        $treeShell = Tree $install
        # Control: the C# started and closed on a copy of the install from before the update (what a plain start writes).
        $ctl = "$Root\control"; robocopy "$Root\pre-update" $ctl /E /NFL /NDL /NJH /NJS /NP | Out-Null
        $env:NATIVEMONS_TEST_LOG = "$Root\control.log"
        $c2 = Start-Launcher "$ctl\NativeMonsLauncher.exe" $false
        $t = Get-Date; do { Start-Sleep -Milliseconds 300 } while (-not (Select-String -LiteralPath "$Root\control.log" -Pattern '^1\|nativeMons' -Quiet -ErrorAction SilentlyContinue) -and ((Get-Date) - $t).TotalSeconds -lt 60)
        Start-Sleep 4; $c2.Refresh(); [void][SW.U]::PostMessageW($c2.MainWindowHandle, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)
        if (-not $c2.WaitForExit(15000)) { $c2.Kill(); Say "control C#: did not close, stopped" }
        $ctlChanged = Diff-Trees (Tree "$Root\pre-update") (Tree $ctl)
        Say "a plain C# start and close wrote: $(if ($ctlChanged.Count) { $ctlChanged -join ', ' } else { 'nothing' })"
    } finally {
        $onScreen.Stop()
        foreach ($p in (Launchers)) { Say "left running, stopped: $($p.Id)"; Stop-Process -Id $p.Id -Force }
        Remove-Item env:NATIVEMONS_UPDATE_URL, env:NATIVEMONS_TEST_SCREEN, env:NATIVEMONS_TEST_LOG, env:NATIVEMONS_TEST_NO_CAPTURE, env:NATIVEMONS_TEST_DISCORD, env:NATIVEMONS_TEST_FRAMES -ErrorAction SilentlyContinue
    }
    Say ("windows on screen: about {0:0} s" -f $onScreen.Elapsed.TotalSeconds)
    foreach ($pair in @(@("the live update and the Rust start", $treeLive), @("then the shell start", $treeShell))) {
        $ch = Diff-Trees $treeBefore $pair[1]
        Say "files changed by $($pair[0]) (from before the update): $($ch -join ', ')"
    }
    $playerBefore = @($treeBefore.Keys | Where-Object { $_ -like 'state\*' -or $_ -like 'engines\fulldex\saves\*' -or $_ -in 'engines\fulldex\settings.cfg', 'engines\nativeplat\userdata\options.ini' })
    Say "  state, saves and settings ($($playerBefore.Count) files) byte-identical after: $(@($playerBefore | Where-Object { $treeBefore[$_] -ne $treeShell[$_] }) -join ', ')"
    Say "  the Rust exe in place: $((Sha "$install\NativeMonsLauncher.exe") -eq (Sha $Exe))"
    $bad = @($originals.Keys | Where-Object { (Sha $originals[$_]) -ne $before[$_] })
    Say "originals (2 ROMs, 2 saves) unchanged: $($bad.Count -eq 0)"
    $report | Set-Content -LiteralPath "$Root\..\switchover-live-report.txt" -Encoding utf8
    if (-not $Keep) { Remove-Item -Recurse -Force $Root }
    return
}
$env:NATIVEMONS_UPDATE_URL = 'file:///' + ($Root -replace '\\', '/') + '/releases/'
$e = Run-Exe "$install\NativeMonsLauncher.exe" @("--update", "$Root\results\update.txt")
Remove-Item env:NATIVEMONS_UPDATE_URL
Say "C# --update exit ${e}: $((Get-Content "$Root\results\update.txt") -join ' / ')"
$treeAfter = Tree $install
$changed = Diff-Trees $treeBefore $treeAfter
Say "files the update changed: $($changed -join ', ')"
$expected = 'NativeMonsLauncher.exe (changed)', 'NativeMonsLauncher.exe.old (new)', 'version.txt (changed)', 'fonts\NotoSansSymbols2-OFL.txt (new)', 'fonts\NotoSansSymbols2-Regular.ttf (new)', 'licenses\rust-crates.txt (new)'
$extra = @($changed | Where-Object { $expected -notcontains $_ }); $missing = @($expected | Where-Object { $changed -notcontains $_ })
Say "  only the launcher, its fonts and licenses and version.txt: $(if (-not $extra.Count -and -not $missing.Count) { 'yes' } else { "NO (extra: $($extra -join ', '); missing: $($missing -join ', '))" })"
Say "  NativeMonsLauncher.exe is the Rust exe: $((Sha "$install\NativeMonsLauncher.exe") -eq (Sha $Exe)); .old is the C# exe: $((Sha "$install\NativeMonsLauncher.exe.old") -eq (Sha $csExe))"
$playerBefore = @($treeBefore.Keys | Where-Object { $_ -like 'state\*' -or $_ -like 'engines\fulldex\saves\*' -or $_ -in 'engines\fulldex\settings.cfg', 'engines\nativeplat\userdata\options.ini' })
Say "  state, saves and settings ($($playerBefore.Count) files) byte-identical after the update: $(@($playerBefore | Where-Object { $treeBefore[$_] -ne $treeAfter[$_] }).Count -eq 0)"
Say "  update work folder gone: $(-not (Test-Path "$st\update"))"
$pe = [IO.File]::ReadAllBytes("$install\NativeMonsLauncher.exe"); Say "  the new exe is a window program (PE subsystem $([BitConverter]::ToUInt16($pe, [BitConverter]::ToInt32($pe, 0x3C) + 0x5C)), C#'s is 2)"
Rename-Item $install "updated"
robocopy "$Root\appdata" "$Root\appdata-updated" /E /NFL /NDL /NJH /NJS /NP | Out-Null

# 4. Each launcher reads the updated install from the same path: Rust as installed, then C# with its exe put back.
$read = @(@("roms1", "--roms", "<out>"), @("roms2", "--roms", "<out>"), @("list-firered", "--saves", "firered", "<out>", "list"), @("list-red", "--saves", "red", "<out>", "list"),
          @("sync-firered", "--sync", "firered", "<out>"), @("sync-red", "--sync", "red", "<out>"), @("list-firered2", "--saves", "firered", "<out>", "list"))
$exit = @{}
foreach ($side in "rs", "cs") {
    if (Test-Path $install) { Remove-Item -Recurse -Force $install }
    robocopy "$Root\updated" $install /E /NFL /NDL /NJH /NJS /NP | Out-Null
    if (Test-Path "$Root\appdata") { Remove-Item -Recurse -Force "$Root\appdata" }
    robocopy "$Root\appdata-updated" "$Root\appdata" /E /NFL /NDL /NJH /NJS /NP | Out-Null
    if ($side -eq "cs") { Copy-Item -LiteralPath $csExe "$install\NativeMonsLauncher.exe" -Force }
    New-Item -ItemType Directory -Force "$Root\results-$side" | Out-Null
    foreach ($s in $read) { $exit["$side|$($s[0])"] = Run-Exe "$install\NativeMonsLauncher.exe" @($s[1..($s.Count - 1)] | ForEach-Object { $_ -replace '<out>', "$Root\results-$side\$($s[0]).txt" }) }
    Rename-Item $install "done-$side"
    robocopy "$Root\appdata" "$Root\appdata-$side" /E /NFL /NDL /NJH /NJS /NP | Out-Null
}
foreach ($s in $read) {
    $a = "$Root\results-cs\$($s[0]).txt"; $b = "$Root\results-rs\$($s[0]).txt"
    $same = (Test-Path $a) -and (Test-Path $b) -and ((Sha $a) -eq (Sha $b))
    Say ("{0,-14} exit C# {1} Rust {2}  {3}: {4}" -f $s[0], $exit["cs|$($s[0])"], $exit["rs|$($s[0])"], $(if ($same) { 'identical' } else { 'DIFFERS' }), ((Get-Content $b -ErrorAction SilentlyContinue | Select-Object -First 3) -join ' / '))
}
$u = Tree "$Root\updated"; $dr = Tree "$Root\done-rs"; $dc = Tree "$Root\done-cs"
foreach ($t in $dr, $dc) { foreach ($k in 'NativeMonsLauncher.exe') { $t.Remove($k) } }
$u.Remove('NativeMonsLauncher.exe')
$rw = Diff-Trees $u $dr; $cw = Diff-Trees $u $dc
Say "files the Rust reads wrote: $(if ($rw.Count) { $rw -join ', ' } else { 'none' })"
Say "files the C# reads wrote: $(if ($cw.Count) { $cw -join ', ' } else { 'none' })"
$both = Diff-Trees $dc $dr
Say "install after the Rust reads vs after the C# reads: $(if ($both.Count) { "DIFFERS: $($both -join ', ')" } else { "identical ($($dr.Count) files)" })"
$ab = Diff-Trees (Tree "$Root\appdata-cs") (Tree "$Root\appdata-rs")
Say "APPDATA after each: $(if ($ab.Count) { "DIFFERS: $($ab -join ', ')" } else { 'identical' })"
$bad = @($originals.Keys | Where-Object { (Sha $originals[$_]) -ne $before[$_] })
Say "originals (2 ROMs, 2 saves) unchanged: $($bad.Count -eq 0)"
$report | Set-Content -LiteralPath "$Root\..\switchover-report.txt" -Encoding utf8
if (-not $Keep) { Remove-Item -Recurse -Force $Root }

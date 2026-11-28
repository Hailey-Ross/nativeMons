# The dialogs live (needs the user's OK: a window on monitor 3 for about 3 minutes): each flow that opens a system
# picker or the save-name box, reached by scripted input (NATIVEMONS_TEST_INPUT), its dialog found by the launcher's
# process id and canceled or answered by window messages (a path typed into the picker's name box, then OK; the
# save-name box's text set, then OK). Six short runs in a scratch copy of the boxes fixture plus Gen1Recomp (copied
# from the app, for Red's mods), <work>\window-test-dialogs (deleted after unless -Keep). Nothing is recorded, no real
# input is sent, no game starts (the "PKHeX" picked is an empty file that can't start). The same flows go through
# the C# and the Rust replays (--replay, headless, answers from the scripts) for the comparison, and the saves the
# Rename touched are listed by both launchers' --saves.
# -Headless: the runs without a window (--live-headless; the dialogs never open, so only the way to them is checked).
# Needs: goldens-boxes.ps1, goldens-fixture.ps1 and flows-data.ps1 made, the release window build, a C# exe
# (-CSharpExe, default <work>\goldens\csharp\NativeMonsLauncher.exe) and NATIVEMONS_APP for Gen1Recomp and LOVE.
param([string]$Exe = "", [string]$CSharpExe = "", [switch]$Keep, [string]$Only = "", [string]$Data = "", [switch]$Headless)
$ErrorActionPreference = "Stop"
. "$PSScriptRoot\work.ps1"
if (-not $Exe) { $Exe = "$TargetDir\release\nativemons.exe" }
if (-not $CSharpExe) { $CSharpExe = "$Work\goldens\csharp\NativeMonsLauncher.exe" }
if (-not $Data) { $Data = "$Work\flows-data" }
$root = "$Work\window-test-dialogs"
$boxes = "$Work\goldens-boxes\rust"
$fixture = "$Work\goldens\fixture\roms"
foreach ($need in $Exe, $CSharpExe, "$boxes\engines\fulldex\saves\firered.sav", "$Data\PKHeX\PKHeX.exe", $fixture, "$App\engines\gen1recomp\main.lua") { if (-not (Test-Path -LiteralPath $need)) { throw "missing $need" } }
if (Test-Path $root) { Remove-Item -Recurse -Force $root }
$side = "$root\side"; $d = "$root\data"
robocopy $boxes $side /E /XD "$boxes\out" "$boxes\replay" "$boxes\heat" "$boxes\perf" /NFL /NDL /NJH /NJS /NP | Out-Null
New-Item -ItemType Directory -Force "$side\state", "$side\appdata", "$side\fonts", "$root\logs", "$d\savedrop", "$d\addrom", "$d\mod\window_test_mod", "$d\pc\Window Test Game" | Out-Null
Copy-Item $Exe "$side\nativemons.exe" -Force
Copy-Item $CSharpExe "$side\NativeMonsLauncher.exe" -Force
Copy-Item "$Work\goldens-boxes\csharp\games.json" $side -Force
Copy-Item "$Work\goldens\csharp\fonts\*", "$PSScriptRoot\..\fonts\*" "$side\fonts" -Force
New-Item -ItemType Directory -Force "$side\engines\fulldex", "$side\engines\pokeplatinum" | Out-Null
foreach ($f in "$side\engines\fulldex\Pokemon Fulldex.exe", "$side\engines\pokeplatinum\main.exe") { if (-not (Test-Path -LiteralPath $f)) { New-Item -ItemType File $f | Out-Null } }
robocopy "$App\engines\gen1recomp" "$side\engines\gen1recomp" /E /XD "$App\engines\gen1recomp\.git" /NFL /NDL /NJH /NJS /NP | Out-Null
robocopy "$App\love" "$side\love" /E /NFL /NDL /NJH /NJS /NP | Out-Null
$st = "$side\state"
[IO.File]::WriteAllText("$st\rom-folders.txt", "$fixture`r`n")
foreach ($kv in @{ 'wallpaper.txt' = 'drawn'; 'last-game.txt' = 'firered'; 'quick-start.txt' = 'off'; 'light-mode.txt' = 'off'; 'discord.txt' = 'off'; 'browse-last.txt' = "$d\addrom" }.GetEnumerator()) { [IO.File]::WriteAllText("$st\$($kv.Key)", $kv.Value) }
# Answers: a save in a folder of its own, a ROM copy to add, a mod and a PC game as .zip files (the game's program is an empty file).
$redSave = "$TestData\billspc\firered.sav"; $saveSha = (Get-FileHash -LiteralPath $redSave).Hash
Copy-Item -LiteralPath $redSave "$d\savedrop\Pokemon FireRed.sav"
$romSrc = Rom-Path "emerald"; $romSha = (Get-FileHash -LiteralPath $romSrc).Hash
Copy-Item -LiteralPath $romSrc "$d\addrom\"
[IO.File]::WriteAllText("$d\mod\window_test_mod\main.lua", "return {}`n")
[IO.File]::WriteAllText("$d\mod\window_test_mod\manifest.json", '{"id": "window_test_mod", "name": "Window Test Mod", "version": "1.0.0", "entry": "main.lua", "category": "QOL", "priority": 100, "dependencies": [], "optional_dependencies": [], "conflicts": [], "games": ["gen1"], "description": "A sample mod for the dialog test."}')
New-Item -ItemType File "$d\pc\Window Test Game\Game.exe" | Out-Null
Add-Type -AssemblyName System.IO.Compression.FileSystem
[IO.Compression.ZipFile]::CreateFromDirectory("$d\mod", "$d\window-test-mod.zip")
[IO.Compression.ZipFile]::CreateFromDirectory("$d\pc", "$d\window-test-game.zip")

Add-Type -Namespace WD -Name U -MemberDefinition @"
public delegate bool EnumProc(System.IntPtr h, System.IntPtr l);
[DllImport("user32.dll")] public static extern bool PostMessageW(System.IntPtr h, uint m, System.IntPtr w, System.IntPtr l);
[DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern System.IntPtr SendMessageW(System.IntPtr h, uint m, System.IntPtr w, string l);
[DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, System.IntPtr l);
[DllImport("user32.dll")] public static extern bool EnumChildWindows(System.IntPtr p, EnumProc f, System.IntPtr l);
[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(System.IntPtr h, out uint pid);
[DllImport("user32.dll")] public static extern bool IsWindowVisible(System.IntPtr h);
[DllImport("user32.dll")] public static extern bool IsWindow(System.IntPtr h);
[DllImport("user32.dll")] public static extern System.IntPtr GetParent(System.IntPtr h);
[DllImport("user32.dll")] public static extern int GetDlgCtrlID(System.IntPtr h);
[DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassNameW(System.IntPtr h, System.Text.StringBuilder s, int n);
[DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowTextW(System.IntPtr h, System.Text.StringBuilder s, int n);
[DllImport("user32.dll")] public static extern bool GetWindowRect(System.IntPtr h, out RECT r);
public struct RECT { public int L, T, R, B; }
"@
function Class-Of($h) { $s = New-Object Text.StringBuilder 256; [void][WD.U]::GetClassNameW($h, $s, 256); "$s" }
function Text($h) { $s = New-Object Text.StringBuilder 512; [void][WD.U]::GetWindowTextW($h, $s, 512); "$s" }
# A visible top-level window of the launcher's process other than its own (a dialog), or zero.
function Find-Dialog($main) {
    $mainPid = [uint32]0; [void][WD.U]::GetWindowThreadProcessId($main, [ref]$mainPid); $script:found = [IntPtr]::Zero
    [void][WD.U]::EnumWindows({ param($h, $l) $p = [uint32]0; [void][WD.U]::GetWindowThreadProcessId($h, [ref]$p); if ($p -eq $mainPid -and $h -ne $main -and [WD.U]::IsWindowVisible($h)) { $script:found = $h; return $false }; return $true }, [IntPtr]::Zero)
    $script:found
}
# The dialog's text box for a name or path: the picker's file/folder name box (an Edit inside a ComboBox), else the first Edit.
function Name-Box($dlg) {
    $script:edits = @()
    [void][WD.U]::EnumChildWindows($dlg, { param($h, $l) if ((Class-Of $h) -eq 'Edit' -and [WD.U]::IsWindowVisible($h)) { $script:edits += $h }; return $true }, [IntPtr]::Zero)
    $inCombo = @($script:edits | Where-Object { (Class-Of ([WD.U]::GetParent($_))) -eq 'ComboBox' })
    if ($inCombo.Count) { $inCombo[0] } elseif ($script:edits.Count) { $script:edits[0] } else { [IntPtr]::Zero }
}
# Each action waits for the next dialog (the first up to 30 s, the start included; then 12 s), notes it, cancels it (WM_CLOSE) or answers it (text, then IDOK),
# and waits for it to go.
function Do-Dialogs($h, $log, $actions) {
    $first = $true
    foreach ($a in $actions) {
        $t0 = Get-Date; $dlg = [IntPtr]::Zero; $limit = if ($first) { 30 } else { 12 }; $first = $false
        while (((Get-Date) - $t0).TotalSeconds -lt $limit -and $dlg -eq [IntPtr]::Zero) { Start-Sleep -Milliseconds 250; $dlg = Find-Dialog $h }
        if ($dlg -eq [IntPtr]::Zero) { Add-Content $log "harness: no dialog within $limit s (for $($a.do))"; continue }
        Start-Sleep -Milliseconds 1200
        $r = New-Object WD.U+RECT; [void][WD.U]::GetWindowRect($dlg, [ref]$r)
        Add-Content $log "harness: dialog up, class $(Class-Of $dlg), title '$(Text $dlg)', at $($r.L),$($r.T) $($r.R - $r.L)x$($r.B - $r.T)"
        if ($a.do -eq 'cancel') { [void][WD.U]::PostMessageW($dlg, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero); Add-Content $log "harness: canceled (WM_CLOSE)" }
        else {
            $box = Name-Box $dlg
            if ($box -eq [IntPtr]::Zero) { Add-Content $log "harness: no text box, canceled"; [void][WD.U]::PostMessageW($dlg, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero) }
            else { [void][WD.U]::SendMessageW($box, 0x000C, [IntPtr]::Zero, $a.text); Start-Sleep -Milliseconds 300; [void][WD.U]::PostMessageW($dlg, 0x0111, [IntPtr]1, [IntPtr]::Zero); Add-Content $log "harness: answered '$($a.text)' (text set, IDOK)" }
        }
        $t1 = Get-Date; while (((Get-Date) - $t1).TotalSeconds -lt 6 -and [WD.U]::IsWindow($dlg) -and [WD.U]::IsWindowVisible($dlg)) { Start-Sleep -Milliseconds 200 }
        if ([WD.U]::IsWindow($dlg) -and [WD.U]::IsWindowVisible($dlg)) { Add-Content $log "harness: dialog still up after 6 s, closed"; [void][WD.U]::PostMessageW($dlg, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero) }
    }
}
$onScreen = 0.0
function Start-Run($name, [string]$inputText, [string]$open, [int]$limitS, $actions) {
    if ($Only -and -not (@($Only -split ';') -contains $name)) { return }
    $log = "$root\logs\$name.log"
    $env:NATIVEMONS_STATE = "$side\state"; $env:APPDATA = "$side\appdata"; $env:NATIVEMONS_FONTS_DIR = "$side\fonts"
    $env:NATIVEMONS_TEST_SCREEN = "DISPLAY3"; $env:NATIVEMONS_TEST_LOG = $log; $env:NATIVEMONS_TEST_NO_CAPTURE = "1"; $env:NATIVEMONS_TEST_QUIET = "1"
    $env:NATIVEMONS_TEST_DISCORD = "$root\logs\discord.txt"
    [IO.File]::WriteAllText("$root\logs\$name.input.txt", $inputText); $env:NATIVEMONS_TEST_INPUT = "$root\logs\$name.input.txt"
    if ($open) { $env:NATIVEMONS_TEST_OPEN = $open } else { Remove-Item env:NATIVEMONS_TEST_OPEN -ErrorAction SilentlyContinue }
    $psi = New-Object Diagnostics.ProcessStartInfo "$side\nativemons.exe"
    $psi.Arguments = if ($Headless) { "--live-headless $($limitS - 3)" } else { "--launcher" }
    $psi.UseShellExecute = $false; $psi.CreateNoWindow = $true
    $p = [Diagnostics.Process]::Start($psi)
    $t0 = Get-Date; $hwnd = [IntPtr]::Zero
    while (-not $Headless -and ((Get-Date) - $t0).TotalSeconds -lt 10 -and $hwnd -eq [IntPtr]::Zero) {
        Start-Sleep -Milliseconds 200
        $l = if (Test-Path $log) { Select-String -Path $log -Pattern '^\d+ hwnd (\d+)' | Select-Object -First 1 } else { $null }
        if ($l) { $hwnd = [IntPtr][long]$l.Matches[0].Groups[1].Value }
    }
    if ($actions -and $hwnd -ne [IntPtr]::Zero) { Do-Dialogs $hwnd $log $actions }
    $ended = $p.WaitForExit($limitS * 1000)
    if (-not $ended) { Stop-Process -Id $p.Id -Force; Add-Content $log "harness: stopped after $limitS s" }
    $script:onScreen += ((Get-Date) - $t0).TotalSeconds
    Remove-Item env:NATIVEMONS_TEST_INPUT, env:NATIVEMONS_TEST_OPEN -ErrorAction SilentlyContinue
    [pscustomobject]@{ name = $name; ended = $ended; log = $log; seconds = [int]((Get-Date) - $t0).TotalSeconds }
}
# The start reads Gen1Recomp's games (~10 s): two waits in a row cover it.
$w2 = { param($t) "0 waitpage $t`n1 waitpage $t`n" }
$runs = @()
# 1. Open in PKHeX on FireRed's page: the picker canceled, then answered with the (empty) PKHeX.exe: "Can't start PKHeX".
$runs += Start-Run "pkhex" ((& $w2 "Pokemon FireRed") + "1500 clicktext Open in PKHeX`n8000 clicktext Open in PKHeX`n16000 close`n") "game:firered" 40 @(@{ do = 'cancel' }, @{ do = 'answer'; text = "$Data\PKHeX\PKHeX.exe" })
# 2. Settings, Import saves: Add a save folder answered with a folder holding a FireRed save; Choose a file canceled.
$runs += Start-Run "import" ((& $w2 "Settings") + "1500 clicktext Import saves`n1600 waitpage Import saves`n3000 clicktext Add a save folder`n9000 waitpage Import saves`n10000 clicktext Choose a file`n16000 close`n") "settings" 40 @(@{ do = 'answer'; text = "$d\savedrop" }, @{ do = 'cancel' })
# 3. ROM folders, Add a ROM file: the launcher's own browser (no system dialog), at the folder with an Emerald copy;
#    the ROM selected, then added.
$runs += Start-Run "addrom" ((& $w2 "nativeMons") + "1000 clickitem 4`n1100 waitpage ROM folders`n2000 key down`n2200 key down`n2400 key enter`n2500 waitpage Choose ROMs`n3500 key down`n3700 key enter`n4500 key enter`n6000 waitpage Added`n9000 close`n") "" 35 $null
# 4. Red's Mods, Install from a .zip file: canceled, then a sample mod's .zip.
$runs += Start-Run "modzip" ((& $w2 "Pokemon Red") + "1500 clicktext Mods`n1600 waitpage Mods`n3000 clicktext Install from a .zip file`n9000 clicktext Install from a .zip file`n17000 close`n") "game:red" 40 @(@{ do = 'cancel' }, @{ do = 'answer'; text = "$d\window-test-mod.zip" })
# 5. Games (Down to its last row, Add a PC game), Add a PC game: From its folder canceled, From a .zip answered with a game's .zip (its program never runs).
$runs += Start-Run "pcgame" ((& $w2 "nativeMons") + "1000 clickitem 1`n1100 waitpage Games`n" + ((0..31 | ForEach-Object { "$(1500 + $_ * 50) key down`n" }) -join "") + "3500 key enter`n3600 waitpage Add a PC game`n4500 clicktext From its folder`n10500 clicktext From a .zip`n19500 close`n") "" 40 @(@{ do = 'cancel' }, @{ do = 'answer'; text = "$d\window-test-game.zip" })
# 6. FireRed's saves: Save 1, Rename answered "Window test"; New save canceled.
$runs += Start-Run "savename" ((& $w2 "Pokemon FireRed") + "1500 clicktext Saves`n1600 waitpage Pokemon FireRed saves`n3000 key enter`n3100 waitpage Save 1`n4000 clicktext Rename`n9000 waitpage Pokemon FireRed saves`n10000 clicktext New save`n16000 close`n") "game:firered" 40 @(@{ do = 'answer'; text = "Window test" }, @{ do = 'cancel' })

# Summary: pages in order, dialogs and their answers, what the launcher made of them.
$out = @()
foreach ($r in $runs) {
    $lines = @(Get-Content $r.log -ErrorAction SilentlyContinue)
    $pages = (@($lines | Where-Object { $_ -match '^\d+ page ' } | ForEach-Object { ($_ -split ' page ', 2)[1] }) -join ' > ')
    $dlg = @($lines | Where-Object { $_ -match '^(\d+ )?(dialog |asktext |harness: |scripted: )' } | ForEach-Object { $_ -replace '^\d+ ', '' }) -join '; '
    $exit = $lines | Where-Object { $_ -match '^\d+ exit ' } | Select-Object -Last 1
    $out += "$($r.name): ended by itself $($r.ended) after $($r.seconds) s | pages $pages | $dlg | $exit"
}
# What the Rename and the import left: both launchers list FireRed's saves the same way.
if (-not $Only -or @($Only -split ';') -contains 'savename') {
    $env:NATIVEMONS_STATE = "$side\state"; $env:APPDATA = "$side\appdata"
    foreach ($x in @(@("rust", "$side\nativemons.exe"), @("c#", "$side\NativeMonsLauncher.exe"))) {
        $psi = New-Object Diagnostics.ProcessStartInfo $x[1], "--saves firered `"$root\logs\saves-$($x[0]).txt`" list"; $psi.UseShellExecute = $false; $psi.CreateNoWindow = $true
        $p = [Diagnostics.Process]::Start($psi); [void]$p.WaitForExit(120000)
    }
    $a = Get-Content "$root\logs\saves-rust.txt" -ErrorAction SilentlyContinue; $b = Get-Content "$root\logs\saves-c#.txt" -ErrorAction SilentlyContinue
    $out += "saves firered list: rust '$($a -join ' / ')' | c# '$($b -join ' / ')' | identical $((($a -join "`n") -eq ($b -join "`n")))"
}
$out += "originals unchanged: firered.sav $((Get-FileHash -LiteralPath $redSave).Hash -eq $saveSha), emerald ROM $((Get-FileHash -LiteralPath $romSrc).Hash -eq $romSha)"
$out += ("windows on screen: about {0:0} s" -f $onScreen)
$out | Set-Content -Encoding utf8 "$root\summary.txt"
$out
if (-not $Keep) { Copy-Item "$root\summary.txt" "$env:TEMP\nativemons-window-test-dialogs-summary.txt" -Force; Remove-Item -Recurse -Force $root; "scratch folder removed; summary kept at $env:TEMP\nativemons-window-test-dialogs-summary.txt" }

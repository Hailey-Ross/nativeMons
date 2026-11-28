# The Rust launcher's window test (needs the user's OK: it puts a window on monitor 3 for about 2 minutes).
# Six short runs, one after another, each in a scratch copy of the boxes fixture (<work>\window-test,
# deleted after unless -Keep): idle frame rate with Light mode off and on, posted keys, scripted mouse, power-off by a
# posted WM_CLOSE, and the tablet morph. No real input is sent (the mouse and keys stay the user's), nothing is
# recorded, no game is started, and the launcher has no sound. Each run ends by itself; one that doesn't within its
# time limit is stopped (only the process this script started). Results: summary.txt in the scratch folder, printed.
# Build first (headless): cargo build --release -j 4 -p nativemons --features window (CMAKE = VS's cmake.exe).
# -Only: a semicolon list of run names (a confirm rerun of single checks).
param([string]$Exe = "", [switch]$Keep, [string]$Only = "")
$ErrorActionPreference = "Stop"
. "$PSScriptRoot\work.ps1"
if (-not $Exe) { $Exe = "$TargetDir\release\nativemons.exe" }
$root = "$Work\window-test"
$boxes = "$Work\goldens-boxes\rust"
$fixture = "$Work\goldens\fixture\roms"
if (-not (Test-Path $Exe)) { throw "no window build at $Exe" }
if (-not (Test-Path "$boxes\engines\fulldex\saves\firered.sav")) { throw "make the boxes fixture first (goldens-boxes.ps1 make)" }
if (Test-Path $root) { Remove-Item -Recurse -Force $root }
$side = "$root\side"
robocopy $boxes $side /E /XD "$boxes\out" "$boxes\replay" "$boxes\heat" "$boxes\perf" /NFL /NDL /NJH /NJS /NP | Out-Null
New-Item -ItemType Directory -Force "$side\state", "$side\appdata", "$side\fonts", "$root\logs" | Out-Null
Copy-Item $Exe "$side\nativemons.exe" -Force
Copy-Item "$Work\goldens-boxes\csharp\games.json" $side -Force
Copy-Item "$Work\goldens\csharp\fonts\*", "$PSScriptRoot\..\fonts\*" "$side\fonts" -Force
if (Test-Path $fixture) { [IO.File]::WriteAllText("$side\state\rom-folders.txt", "$fixture`r`n") }
# Placeholder engines (empty files, never started) so Fulldex and the port count as installed, as in the goldens.
New-Item -ItemType Directory -Force "$side\engines\fulldex", "$side\engines\pokeplatinum" | Out-Null
foreach ($f in "$side\engines\fulldex\Pokemon Fulldex.exe", "$side\engines\pokeplatinum\main.exe") { if (-not (Test-Path -LiteralPath $f)) { New-Item -ItemType File $f | Out-Null } }
# The one-time wallpaper question would hold the start on its page: answered as the drawn scene.
[IO.File]::WriteAllText("$side\state\wallpaper.txt", "drawn")
Add-Type -Namespace W -Name U -MemberDefinition '[DllImport("user32.dll")] public static extern bool PostMessageW(System.IntPtr h, uint m, System.IntPtr w, System.IntPtr l);'

function Start-Run($name, [string]$light, [string]$inputText, [string]$open, [int]$limitS, [scriptblock]$during) {
    if ($Only -and -not (@($Only -split ';') -contains $name)) { return }
    $log = "$root\logs\$name.log"
    [IO.File]::WriteAllText("$side\state\light-mode.txt", $light)
    $env:NATIVEMONS_STATE = "$side\state"; $env:APPDATA = "$side\appdata"; $env:NATIVEMONS_FONTS_DIR = "$side\fonts"
    $env:NATIVEMONS_TEST_SCREEN = "DISPLAY3"; $env:NATIVEMONS_TEST_LOG = $log; $env:NATIVEMONS_TEST_NO_CAPTURE = "1"
    Remove-Item env:NATIVEMONS_TEST_T, env:NATIVEMONS_TEST_INPUT, env:NATIVEMONS_TEST_OPEN, env:NATIVEMONS_TEST_FRAMES -ErrorAction SilentlyContinue
    if ($inputText) { [IO.File]::WriteAllText("$root\logs\$name.input.txt", $inputText); $env:NATIVEMONS_TEST_INPUT = "$root\logs\$name.input.txt" }
    if ($open) { $env:NATIVEMONS_TEST_OPEN = $open }
    # No console window for the console program (the launcher window still shows).
    $psi = New-Object Diagnostics.ProcessStartInfo "$side\nativemons.exe", "--launcher"
    $psi.UseShellExecute = $false; $psi.CreateNoWindow = $true
    $p = [Diagnostics.Process]::Start($psi)
    $hwnd = [IntPtr]::Zero
    $t0 = Get-Date
    while (((Get-Date) - $t0).TotalSeconds -lt 10 -and $hwnd -eq [IntPtr]::Zero) {
        Start-Sleep -Milliseconds 200
        $l = if (Test-Path $log) { Select-String -Path $log -Pattern '^\d+ hwnd (\d+)' | Select-Object -First 1 } else { $null }
        if ($l) { $hwnd = [IntPtr][long]$l.Matches[0].Groups[1].Value }
    }
    if ($during -and $hwnd -ne [IntPtr]::Zero) { & $during $hwnd }
    $ended = $p.WaitForExit($limitS * 1000)
    if (-not $ended) { Stop-Process -Id $p.Id -Force; Add-Content $log "harness: stopped after $limitS s" }
    Remove-Item env:NATIVEMONS_TEST_INPUT, env:NATIVEMONS_TEST_OPEN -ErrorAction SilentlyContinue
    [pscustomobject]@{ name = $name; ended = $ended; log = $log; seconds = [int]((Get-Date) - $t0).TotalSeconds }
}
function Post-Key($h, [int]$vk, [long]$lp) { [void][W.U]::PostMessageW($h, 0x0100, [IntPtr]$vk, [IntPtr]$lp) }

$runs = @()
# 1-2. The idle home screen for 15 s, Light mode off and on (ends by a scripted close).
$runs += Start-Run "idle-normal" "off" "15000 close`n" "" 30 $null
$runs += Start-Run "idle-light" "on" "15000 close`n" "" 30 $null
# 3. Posted WM_KEYDOWN as stable-check posts them (lParam 0) and with lParam 1: Down, Down, Enter, Escape.
$runs += Start-Run "keys-posted" "off" "12000 close`n" "" 25 { param($h) Start-Sleep 3; Post-Key $h 0x28 0; Start-Sleep -Milliseconds 500; Post-Key $h 0x28 1; Start-Sleep -Milliseconds 500; Post-Key $h 0x0D 0; Start-Sleep 2; Post-Key $h 0x1B 1 }
# 4. Scripted mouse through NATIVEMONS_TEST_INPUT: hover, wheel, click Settings, right click back (no drag: it would take the real mouse).
$runs += Start-Run "mouse-scripted" "off" "2500 move 300 500`n2600 wheel -120`n2800 wheel 120`n3200 clickitem 7`n5500 click 300 600 right`n8000 close`n" "" 25 $null
# 5. Power-off from a posted WM_CLOSE: the lights, the fall, then the exit.
$runs += Start-Run "power-off" "off" "" "" 20 { param($h) Start-Sleep 4; [void][W.U]::PostMessageW($h, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero) }
# 6. The tablet morph: Move Pokemon's boxes open at start (the phone grows), Escape folds it back, then close.
$runs += Start-Run "tablet-morph" "off" "4000 key escape`n7000 close`n" "boxes:firered,platinum" 25 $null

# 7-9 (added 2026-11-25, rust-launcher-features.md): run with -Only "guide-overlay;dialogs;dialogs-picker;idle-light", about 2 minutes.
# 7. The guide overlay (test hook: opened over the launcher's own screen at 4 s, as if a game ran): posted Down, Enter
#    (Controls shown), Escape (hidden), Escape (resume: the overlay closes); then close.
Add-Type -Namespace W -Name E -MemberDefinition @"
public delegate bool EnumProc(System.IntPtr h, System.IntPtr l);
[DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, System.IntPtr l);
[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(System.IntPtr h, out uint pid);
[DllImport("user32.dll")] public static extern bool IsWindowVisible(System.IntPtr h);
[DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern System.IntPtr FindWindowW(string c, string t);
[DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern System.IntPtr FindWindowExW(System.IntPtr p, System.IntPtr a, string c, string t);
[DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern System.IntPtr SendMessageW(System.IntPtr h, uint m, System.IntPtr w, string l);
"@
function Find-Other($main) {
    $mainPid = [uint32]0; [void][W.E]::GetWindowThreadProcessId($main, [ref]$mainPid); $found = [IntPtr]::Zero
    [void][W.E]::EnumWindows({ param($h, $l) $p = [uint32]0; [void][W.E]::GetWindowThreadProcessId($h, [ref]$p); if ($p -eq $mainPid -and $h -ne $main -and [W.E]::IsWindowVisible($h)) { $script:found = $h; return $false }; return $true }, [IntPtr]::Zero)
    $script:found
}
$env:NATIVEMONS_TEST_GUIDE = "4000"
$runs += Start-Run "guide-overlay" "off" "14000 close`n" "" 25 { param($h) Start-Sleep 6; $o = Find-Other $h; if ($o -ne [IntPtr]::Zero) { foreach ($k in 0x28, 0x0D, 0x1B, 0x1B) { Post-Key $o $k 0; Start-Sleep -Milliseconds 800 } } }
Remove-Item env:NATIVEMONS_TEST_GUIDE -ErrorAction SilentlyContinue
# 8. Dialogs (test hook): the save-name box at 3 s, its text set and OK pressed by messages; then the system folder
#    picker at 6 s, closed (a cancel). The answers are logged ("asktext", "dialog").
$env:NATIVEMONS_TEST_DIALOG = "3000 text"
$runs += Start-Run "dialogs" "off" "9000 close`n" "" 25 { param($h) Start-Sleep 5; $d = [W.E]::FindWindowW("NativeMonsAskText", "Window test"); if ($d -ne [IntPtr]::Zero) { $e = [W.E]::FindWindowExW($d, [IntPtr]::Zero, "Edit", $null); [void][W.E]::SendMessageW($e, 0x000C, [IntPtr]::Zero, "Red save"); [void][W.U]::PostMessageW($d, 0x0111, [IntPtr]1, [IntPtr]::Zero) } }
$env:NATIVEMONS_TEST_DIALOG = "2500 folder"
$runs += Start-Run "dialogs-picker" "off" "9000 close`n" "" 25 { param($h) Start-Sleep 5; $d = [W.E]::FindWindowW($null, "Window test: pick a folder"); if ($d -ne [IntPtr]::Zero) { [void][W.U]::PostMessageW($d, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero) } }
Remove-Item env:NATIVEMONS_TEST_DIALOG -ErrorAction SilentlyContinue

# Summary from the logs: frames and their render times, events, power-off, window changes.
$out = @()
foreach ($r in $runs) {
    $lines = @(Get-Content $r.log -ErrorAction SilentlyContinue)
    $frames = @($lines | Where-Object { $_ -match '^(\d+) frame ([0-9.]+) ms' } | ForEach-Object { [pscustomobject]@{ t = [int]$Matches[1]; ms = [double]$Matches[2] } })
    $homeLine = $lines | Where-Object { $_ -match '^(\d+) page 1\|' } | Select-Object -First 1
    $from = if ($homeLine -match '^(\d+)') { [int]$Matches[1] + 2000 } else { 3000 }
    $mid = @($frames | Where-Object { $_.t -ge $from -and $_.t -lt $from + 10000 })
    $fps = if ($mid.Count) { "{0:0.0} fps, {1:0.00} ms a frame (max {2:0.00})" -f ($mid.Count / 10.0), (($mid | Measure-Object ms -Average).Average), (($mid | Measure-Object ms -Maximum).Maximum) } else { "no frames" }
    $keys = @($lines | Where-Object { $_ -match ' event Key\(' -and $_ -notmatch 'guide event' }).Count
    $guideKeys = @($lines | Where-Object { $_ -match 'guide event Key\(' }).Count
    $mouse = @($lines | Where-Object { $_ -match 'event (MouseMove|MouseDown|MouseUp|Click|Wheel)' }).Count
    $pages = (@($lines | Where-Object { $_ -match ' page ' } | ForEach-Object { ($_ -split ' page ', 2)[1] }) -join ' > ')
    $power = $lines | Where-Object { $_ -match ' power-off$' } | Select-Object -First 1
    $exit = $lines | Where-Object { $_ -match ' exit ' } | Select-Object -Last 1
    $bounds = @($lines | Where-Object { $_ -match 'effect SetBounds' }).Count
    $extra = @($lines | Where-Object { $_ -match ' (guide |asktext |dialog )' -and $_ -notmatch 'guide event' } | ForEach-Object { ($_ -split ' ', 2)[1] }) -join '; '
    $out += "$($r.name): $extra | ended by itself $($r.ended) after $($r.seconds) s | $fps | keys $keys (guide $guideKeys), mouse events $mouse | pages $pages | $power | $exit | SetBounds $bounds"
}
$out | Set-Content -Encoding utf8 "$root\summary.txt"
$out
if (-not $Keep) { Copy-Item "$root\summary.txt" "$env:TEMP\nativemons-window-test-summary.txt" -Force; Remove-Item -Recurse -Force $root; "scratch folder removed; summary kept at $env:TEMP\nativemons-window-test-summary.txt" }

# Controller check by hand (about 5 minutes): starts the Rust launcher on monitor 3 (DISPLAY3) with its test log on,
# which notes every change of the pad as read (buttons, Guide, focus, controllers), the top page and its selection.
# The console says which button to press next and checks each one in the log; at the end it prints PASS/FAIL per
# step, keeps the report in %TEMP%\nativemons-pad-check.txt, closes the launcher and deletes its scratch copy.
# Nothing else is started; ROMs and saves are copies (FireRed and Platinum, as Fulldex's, with the app's mon-bridge for
# Move Pokemon; the Fulldex program is an empty placeholder), the launcher's state is its own scratch folder.
#   pad-check.ps1 -Work <scratch folder> [-Exe <release window build>] [-Roms <key=path list>] [-Screen DISPLAY3]
# -Dry: the same setup with no window (--live-headless): checks the start and the log lines, asks for no presses.
param([string]$Work = $env:NATIVEMONS_WORK, [string]$Exe = "", [string]$Roms = "", [string]$Screen = "DISPLAY3", [int]$StepSeconds = 30, [switch]$Dry)
$ErrorActionPreference = "Stop"
if (-not $Work) { throw "give -Work <scratch folder>" }
$env:NATIVEMONS_WORK = $Work
if ($Roms) { $env:NATIVEMONS_ROMS = $Roms } elseif (-not $env:NATIVEMONS_ROMS -and -not (Test-Path "$Work\fixture-roms.txt")) { $env:NATIVEMONS_ROMS = Join-Path (Split-Path $Work) "fixture-roms.txt" }
. "$PSScriptRoot\work.ps1"
if (-not $Exe) { $Exe = "$TargetDir\release\nativemons.exe" }
if (-not (Test-Path $Exe)) { throw "no window build at $Exe (cargo build --release -p nativemons --features window)" }
$repo = Split-Path (Split-Path $PSScriptRoot)
$root = "$Work\pad-check"; $side = "$root\side"; $log = "$root\pad.log"
if (Test-Path $root) { Remove-Item -Recurse -Force $root }
New-Item -ItemType Directory -Force "$side\state", "$side\appdata", "$side\fonts", "$root\roms", "$side\engines\fulldex\saves", "$side\tools\mon-bridge" | Out-Null
New-Item -ItemType File "$side\engines\fulldex\Pokemon Fulldex.exe" | Out-Null
Copy-Item "$App\tools\mon-bridge\mon-bridge.exe" "$side\tools\mon-bridge\"
Copy-Item $Exe "$side\nativemons.exe"; Copy-Item "$repo\launcher\games.json" $side
Copy-Item "$repo\launcher\fonts\*", "$repo\launcher-rs\fonts\*" "$side\fonts" -Recurse -Force
$originals = @{}; foreach ($k in "firered", "platinum") { $p = Rom-Path $k; $originals[$p] = (Get-FileHash -LiteralPath $p).Hash; Copy-Item -LiteralPath $p "$root\roms\" }
foreach ($s in "firered.sav", "platinum.sav") { $p = "$App\engines\fulldex\saves\$s"; $originals[$p] = (Get-FileHash -LiteralPath $p).Hash; Copy-Item -LiteralPath $p "$side\engines\fulldex\saves\" }
[IO.File]::WriteAllText("$side\state\rom-folders.txt", "$root\roms`r`n")
foreach ($kv in @{ 'wallpaper.txt' = 'drawn'; 'last-game.txt' = 'firered'; 'quick-start.txt' = 'off'; 'light-mode.txt' = 'off'; 'discord.txt' = 'off'; 'layout.txt' = 'phone' }.GetEnumerator()) { [IO.File]::WriteAllText("$side\state\$($kv.Key)", $kv.Value) }
$env:NATIVEMONS_STATE = "$side\state"; $env:APPDATA = "$side\appdata"; $env:NATIVEMONS_FONTS_DIR = "$side\fonts"
$env:NATIVEMONS_TEST_SCREEN = $Screen; $env:NATIVEMONS_TEST_LOG = $log; $env:NATIVEMONS_TEST_NO_CAPTURE = "1"; $env:NATIVEMONS_TEST_DISCORD = "$root\discord.txt"
Add-Type -Namespace PC -Name U -MemberDefinition '[DllImport("user32.dll")] public static extern bool PostMessageW(System.IntPtr h, uint m, System.IntPtr w, System.IntPtr l);'

$script:seen = 0
function New-Lines { $all = @(Get-Content -LiteralPath $log -ErrorAction SilentlyContinue); $n = @($all | Select-Object -Skip $script:seen); $script:seen = $all.Count; $n }
# Waits for a line matching $rx (or $count of them) since the step began; returns the matching lines or $null.
function Wait-For([string]$rx, [int]$count = 1, [int]$secs = $StepSeconds) {
    $hits = @(); $t = Get-Date
    while (((Get-Date) - $t).TotalSeconds -lt $secs) {
        $hits += @(New-Lines | Where-Object { $_ -match $rx })
        if ($hits.Count -ge $count) { return $hits }
        Start-Sleep -Milliseconds 150
    }
    $null
}
$results = New-Object Collections.Generic.List[string]
function Step([string]$name, [string]$say, [string]$rx, [int]$count = 1) {
    [void](New-Lines)
    Write-Host ""; Write-Host ">> $say" -ForegroundColor Cyan
    $h = Wait-For $rx $count
    $r = if ($h) { "PASS  $name" } else { "FAIL  $name (nothing seen in $StepSeconds s)" }
    Write-Host "   $r" -ForegroundColor $(if ($h) { "Green" } else { "Red" })
    $results.Add($r)
    Start-Sleep -Milliseconds 600
}
function Bit([string]$b) { "^\d+ pad 0x[0-9a-f]+ \[[a-z,]*\b$b\b" }

Write-Host "Controller check: the launcher opens on monitor 3 ($Screen). Keep it in front: don't click this console."
Write-Host "If a press isn't seen, click the launcher window once and press again. Each step waits $StepSeconds s."
$psi = New-Object Diagnostics.ProcessStartInfo "$side\nativemons.exe", $(if ($Dry) { "--live-headless 25" } else { "--launcher" }); $psi.UseShellExecute = $false; $psi.CreateNoWindow = $true
$p = [Diagnostics.Process]::Start($psi)
$started = Get-Date
try {
    if (-not (Wait-For '^\d+ page 1\|nativeMons' 1 60)) { throw "the launcher did not reach its home screen in 60 s" }
    $hwnd = if ($Dry) { [IntPtr]::Zero } else { [IntPtr][long]((Get-Content $log | Where-Object { $_ -match '^\d+ hwnd (\d+)' } | Select-Object -First 1) -split ' ')[2] }
    $script:seen = @(Get-Content $log).Count
    if ($Dry) { $l = @(Get-Content $log); $results.Add("DRY   home reached; pad lines $(@($l | Where-Object { $_ -match '^\d+ pad ' }).Count) (first: $(@($l | Where-Object { $_ -match '^\d+ pad ' })[0])); sel lines $(@($l | Where-Object { $_ -match '^\d+ sel ' }).Count)"); return }
    Step "controller connected" "Turn the controller on / plug it in (or press any button on it)." '^\d+ pad .* pads [1-9]'
    Step "launcher has focus" "Click the launcher window once if it isn't in front (title bar area at the top)." '^\d+ pad .* focused true'
    Step "D-pad Down moves the selection" "Press D-pad DOWN once." '^\d+ sel 1:'
    Step "D-pad Right" "Press D-pad RIGHT once." (Bit "right")
    Step "D-pad Left" "Press D-pad LEFT once." (Bit "left")
    Step "D-pad Up" "Press D-pad UP once." (Bit "up")
    Step "left stick counts as the D-pad" "Push the LEFT STICK down, let go, then up, let go." (Bit "down") 1
    Step "A opens Settings" "With the D-pad, go to Settings (the gear, bottom row) and press A." '^\d+ page 2\|Settings'
    Step "hold repeats" "HOLD D-pad DOWN for 2 seconds, then let go." '^\d+ sel 2:' 3
    Step "B goes back" "Press B." '^\d+ page 1\|nativeMons'
    Step "X" "Press X." (Bit "x")
    Step "Y" "Press Y." (Bit "y")
    Step "LB" "Press LB (left bumper)." (Bit "lb")
    Step "RB" "Press RB (right bumper)." (Bit "rb")
    Step "Back / View / Select" "Press BACK (View / Select / Share)." (Bit "back")
    Step "Start / Menu" "Press START (Menu / Options)." (Bit "start")
    Step "Guide button seen" "Press the GUIDE button (Xbox / PS / Home) once. With no game running it does nothing else." '^\d+ pad .* guide true'
    Step "Move Pokemon by pad" "Go to Move (bottom row, second) and press A. The window grows into the tablet." '^\d+ page \d+\|Move Pokemon'
    Step "RB / LB / X in the boxes" "Press RB, then LB, then X, then B to go back (the tablet folds back)." '^\d+ page 1\|nativeMons'
} catch { $results.Add("FAIL  $($_.Exception.Message)") } finally {
    $p.Refresh()
    if (-not $p.HasExited -and $hwnd -ne [IntPtr]::Zero) { [void][PC.U]::PostMessageW($hwnd, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero); if (-not $p.WaitForExit(15000)) { $p.Kill() } } elseif (-not $p.HasExited -and -not $p.WaitForExit(30000)) { $p.Kill() }
    $pads = @(Get-Content $log -ErrorAction SilentlyContinue | Where-Object { $_ -match '^\d+ pad ' })
    $report = @("nativeMons controller check, $(Get-Date -Format 'yyyy-MM-dd HH:mm'), $([int]((Get-Date) - $started).TotalSeconds) s") + $results + @("", "pad lines in the log: $($pads.Count); last ones:") + @($pads | Select-Object -Last 12)
    $bad = @($originals.Keys | Where-Object { (Get-FileHash -LiteralPath $_).Hash -ne $originals[$_] })
    $report += "ROM and save originals unchanged: $($bad.Count -eq 0)"
    $report | Set-Content -Encoding utf8 "$env:TEMP\nativemons-pad-check.txt"
    Write-Host ""; $report | ForEach-Object { Write-Host $_ }
    Write-Host ""; Write-Host "$(@($results | Where-Object { $_ -like 'PASS*' }).Count) of $($results.Count) passed. Report: $env:TEMP\nativemons-pad-check.txt"
    Remove-Item -Recurse -Force $root -ErrorAction SilentlyContinue
}

# The new window checks in one go (needs the user's OK: windows on monitor 3 for about 2 minutes): makes the boxes
# fixture, runs window-test.ps1 -Only "guide-overlay;dialogs;dialogs-picker;idle-light" through agentmon-run with the
# release window build, prints the summary, then deletes the fixture and the test folder. Nothing is left running.
#   window-test-run.ps1 [-Work <folder>] [-Exe <release nativemons.exe>]
# Build first: cargo build --release -j 4 -p nativemons --features window
param([string]$Work = $env:NATIVEMONS_WORK, [string]$Exe = "")
$ErrorActionPreference = "Stop"
if (-not $Work) { throw "give -Work or set NATIVEMONS_WORK (the folder holding goldens\csharp and app)" }
$env:NATIVEMONS_WORK = $Work
. "$PSScriptRoot\work.ps1"
if (-not $Exe) { $Exe = "$TargetDir\release\nativemons.exe" }
if (-not (Test-Path $Exe)) { throw "no window build at $Exe" }
$gate = Join-Path $env:LOCALAPPDATA "agentMon\agentmon-run.exe"
try {
    & powershell -NoProfile -ExecutionPolicy Bypass -File "$PSScriptRoot\goldens-boxes.ps1" make | Out-Null
    & $gate --session "nativemons window-test" -- powershell -NoProfile -ExecutionPolicy Bypass -File "$PSScriptRoot\window-test.ps1" -Exe $Exe -Only "guide-overlay;dialogs;dialogs-picker;idle-light" -Keep
    if (Test-Path "$Work\window-test\summary.txt") { "=== summary"; Get-Content "$Work\window-test\summary.txt"; Copy-Item "$Work\window-test\summary.txt" "$env:TEMP\nativemons-window-test-summary.txt" -Force; "(also in $env:TEMP\nativemons-window-test-summary.txt; logs deleted)" }
} finally {
    # Only launchers started from the test folder are stopped, if any is still there.
    Get-Process nativemons -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "$Work\window-test\*" } | Stop-Process -Force -ErrorAction SilentlyContinue
    if (Test-Path "$Work\window-test") { Remove-Item -Recurse -Force "$Work\window-test" }
    & powershell -NoProfile -ExecutionPolicy Bypass -File "$PSScriptRoot\goldens-boxes.ps1" clean | Out-Null
}

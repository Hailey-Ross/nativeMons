# The flows window checks in one go (needs the user's OK: windows on monitor 3 for about 3 minutes): makes the boxes
# fixture, the ROM fixture (if missing) and the flows data, runs window-test-flows.ps1 through agentmon-run with the
# release window build, prints the summary, then deletes what it made and the test folder. Nothing is left running.
#   window-test-flows-run.ps1 [-Work <folder>] [-Exe <release nativemons.exe>] [-Only "<run>;<run>"] [-Headless]
# -Headless: the same runs without any window (a dry run of the scripts; no OK needed).
# Build first: cargo build --release -j 4 -p nativemons --features window
param([string]$Work = $env:NATIVEMONS_WORK, [string]$Exe = "", [string]$Only = "", [switch]$Headless)
$ErrorActionPreference = "Stop"
if (-not $Work) { throw "give -Work or set NATIVEMONS_WORK (the folder holding goldens\csharp and app)" }
$env:NATIVEMONS_WORK = $Work
. "$PSScriptRoot\work.ps1"
if (-not $Exe) { $Exe = "$TargetDir\release\nativemons.exe" }
if (-not (Test-Path $Exe)) { throw "no window build at $Exe" }
$gate = Join-Path $env:LOCALAPPDATA "agentMon\agentmon-run.exe"
$madeRoms = -not (Test-Path "$Work\goldens\fixture\roms")
try {
    if ($madeRoms) { & powershell -NoProfile -ExecutionPolicy Bypass -File "$PSScriptRoot\goldens-fixture.ps1" make }
    & powershell -NoProfile -ExecutionPolicy Bypass -File "$PSScriptRoot\goldens-boxes.ps1" make | Out-Null
    & powershell -NoProfile -ExecutionPolicy Bypass -File "$PSScriptRoot\flows-data.ps1" make | Out-Null
    $a = @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "$PSScriptRoot\window-test-flows.ps1", "-Exe", $Exe, "-Keep")
    if ($Only) { $a += @("-Only", $Only) }
    if ($Headless) { $a += "-Headless" }
    & $gate --session "nativemons window-test" -- powershell @a
    if (Test-Path "$Work\window-test-flows\summary.txt") { "=== summary"; Get-Content "$Work\window-test-flows\summary.txt"; Copy-Item "$Work\window-test-flows\summary.txt" "$env:TEMP\nativemons-window-test-flows-summary.txt" -Force; "(also in $env:TEMP\nativemons-window-test-flows-summary.txt; logs deleted)" }
} finally {
    # Only launchers started from the test folder are stopped, if any is still there.
    Get-Process nativemons -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "$Work\window-test-flows\*" } | Stop-Process -Force -ErrorAction SilentlyContinue
    if (Test-Path "$Work\window-test-flows") { Remove-Item -Recurse -Force "$Work\window-test-flows" }
    & powershell -NoProfile -ExecutionPolicy Bypass -File "$PSScriptRoot\flows-data.ps1" clean | Out-Null
    & powershell -NoProfile -ExecutionPolicy Bypass -File "$PSScriptRoot\goldens-boxes.ps1" clean | Out-Null
    if ($madeRoms) { & powershell -NoProfile -ExecutionPolicy Bypass -File "$PSScriptRoot\goldens-fixture.ps1" clean | Out-Null }
}

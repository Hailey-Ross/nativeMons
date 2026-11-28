# Render timings of one or more builds (release, ideally): `--bench <screen> <WxH> <frames> [still]` on each screen, in
# a golden side's environment, through the gate with --exclusive so nothing else runs beside it. Prints the median ms
# for full frames and for the live (recorded, redrawn where changed) idle frame, and how many frames differed.
#   bench.ps1 -Exe <a.exe>[,<b.exe>] [-Frames 120] [-Screens "main@600x1000;main@1600x1000;boxes@1600x1000[boxes]"]
param([string]$Exe, [int]$Frames = 120, [string]$Screens = "main@600x1000;main@1600x1000;games@600x1000;credits@600x1000;boxes@1600x1000[boxes];boxheld@1600x1000[boxes];dex@600x1000[gen1]", [string]$Session = "nativemons bench")
$ErrorActionPreference = "Stop"
. "$PSScriptRoot\work.ps1"
$gate = Join-Path $env:LOCALAPPDATA "agentMon\agentmon-run.exe"
$env:NATIVEMONS_TEST_T = "7.25"; $env:NATIVEMONS_TEST_SEED = "3"; $env:NATIVEMONS_TEST_BATTERY = "60"; $env:NATIVEMONS_TEST_FONTS = "bundled"
$env:NATIVEMONS_TEST_QUIET = "1"; $env:NATIVEMONS_TEST_DEX = "sample"; $env:NATIVEMONS_TEST_NO_CAPTURE = "1"; $env:NATIVEMONS_TEST_CSHARP_ROWS = "1"
Remove-Item env:NATIVEMONS_SNAPSHOT_TEXTMASK -ErrorAction SilentlyContinue
foreach ($e in @($Exe -split ',')) {
    "=== $e"
    foreach ($s in @($Screens -split ';' | Where-Object { $_ })) {
        if ($s -notmatch '^([^@]+)@(\d+x\d+)(?:\[(\w+)\])?$') { throw "bad screen $s" }
        $screen = $Matches[1]; $size = $Matches[2]; $root = if ($Matches[3]) { "$Work\goldens-$($Matches[3])" } else { "$Work\goldens" }
        $side = "$root\rust"
        # The exe runs beside the side's games.json, as the launcher does.
        Copy-Item $e "$side\nativemons-bench.exe" -Force
        $env:NATIVEMONS_STATE = "$side\state"; $env:APPDATA = "$side\appdata"; $env:NATIVEMONS_FONTS_DIR = "$side\fonts"
        $said = & $gate --session $Session --exclusive -- "$side\nativemons-bench.exe" --bench $screen $size $Frames 2>&1
        Remove-Item "$side\nativemons-bench.exe" -Force
        "{0,-28} {1}" -f $s, (($said | Where-Object { $_ -match 'ms|differ' }) -join ' | ')
    }
}

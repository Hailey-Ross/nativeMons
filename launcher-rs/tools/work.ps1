# Where the test scripts keep their output, dot-sourced by each (. "$PSScriptRoot\work.ps1"). No machine paths here:
# NATIVEMONS_WORK (else CARGO_TARGET_DIR's parent folder, else %TEMP%\nativemons-work) holds the golden roots and
# scratch copies; CARGO_TARGET_DIR (else $Work\launcher-rs-target) the builds; NATIVEMONS_APP (else $Work\app) a
# built launcher folder whose engines are copied (never changed); NATIVEMONS_TEST_DATA (else $Work\test-data) sample
# saves. The player's ROMs are only named in NATIVEMONS_ROMS (else $Work\fixture-roms.txt): "key=path" per line.
$Work = if ($env:NATIVEMONS_WORK) { $env:NATIVEMONS_WORK } elseif ($env:CARGO_TARGET_DIR) { Split-Path $env:CARGO_TARGET_DIR -Parent } else { Join-Path $env:TEMP "nativemons-work" }
$TargetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { "$Work\launcher-rs-target" }
$App = if ($env:NATIVEMONS_APP) { $env:NATIVEMONS_APP } else { "$Work\app" }
$TestData = if ($env:NATIVEMONS_TEST_DATA) { $env:NATIVEMONS_TEST_DATA } else { "$Work\test-data" }
function Rom-Path([string]$key) {
    $list = if ($env:NATIVEMONS_ROMS) { $env:NATIVEMONS_ROMS } else { "$Work\fixture-roms.txt" }
    if (-not (Test-Path -LiteralPath $list)) { throw "no ROM list: set NATIVEMONS_ROMS to a file of key=path lines (missing $list)" }
    foreach ($l in Get-Content -LiteralPath $list -Encoding utf8) { $i = $l.IndexOf('='); if ($i -gt 0 -and $l.Substring(0, $i).Trim() -eq $key) { return $l.Substring($i + 1).Trim() } }
    throw "no ROM named $key in $list"
}

# Puts the Rust launcher into a package folder in the C# launcher's place: NativeMonsLauncher.exe (the `shipped`
# build: window, no console, the C#'s icon), its fonts beside the C#'s, and licenses\rust-crates.txt.
# package-rust.ps1 -Out <package folder> [-Exe <a built exe>]. Builds into CARGO_TARGET_DIR (else launcher-rs\target).
param([Parameter(Mandatory = $true)][string]$Out, [string]$Exe = "")
$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
if (-not (Test-Path -LiteralPath (Join-Path $Out 'games.json'))) { throw "No launcher package at $Out (build the C# launcher there first)." }
if (-not $Exe) {
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { throw 'The Rust launcher needs Rust (rustup.rs), CMake and the MSVC Build Tools.' }
    & cargo build --release --locked --manifest-path (Join-Path $root 'Cargo.toml') -p nativemons --features shipped
    if ($LASTEXITCODE -ne 0) { throw 'The Rust launcher did not build.' }
    $target = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $root 'target' }
    $Exe = Join-Path $target 'release\nativemons.exe'
}
Copy-Item -LiteralPath $Exe -Destination (Join-Path $Out 'NativeMonsLauncher.exe') -Force
New-Item -ItemType Directory -Force (Join-Path $Out 'fonts'), (Join-Path $Out 'licenses') | Out-Null
Copy-Item -Path (Join-Path $root 'fonts\*') -Destination (Join-Path $Out 'fonts') -Force
& (Join-Path $PSScriptRoot 'crate-licenses.ps1') -Out (Join-Path $Out 'licenses\rust-crates.txt') -Features shipped
Write-Host "The Rust launcher is NativeMonsLauncher.exe in $Out"

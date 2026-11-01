# Builds the OpenHome converter into one folder: convert.cjs, its wasm, and a Node runtime to run them.
# Needs Git, Node and Rust. Used by launcher\build-all.ps1 and the release build. Versions are pinned here.
param([Parameter(Mandatory)][string]$Out, [Parameter(Mandatory)][string]$Work)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$openHome = 'v1.18.1'; $node = '24.16.0'; $wasmPack = '0.13.1'; $esbuild = '0.25.0'
$here = $PSScriptRoot
$mark = Join-Path $Out '.nativemons-version'
$version = "$openHome-$node"
if ((Test-Path -LiteralPath $mark) -and (Get-Content -LiteralPath $mark) -eq $version) { Write-Host 'Already there.'; return }

function Run($exe) {
    & $exe @args
    if ($LASTEXITCODE -ne 0) { throw "$exe $($args -join ' ') failed." }
}

New-Item -ItemType Directory -Force $Work, $Out | Out-Null
$Work = (Resolve-Path -LiteralPath $Work).Path
$Out = (Resolve-Path -LiteralPath $Out).Path
$src = Join-Path $Work 'src'
if (-not (Test-Path -LiteralPath (Join-Path $src 'package.json'))) {
    Run git clone -q --depth 1 --branch $openHome https://github.com/andrewbenington/OpenHome.git $src
}
$env:CARGO_TARGET_DIR = Join-Path $Work 'target'
$env:CARGO_BUILD_JOBS = '4'
$env:COREPACK_ENABLE_DOWNLOAD_PROMPT = '0'
$env:COREPACK_HOME = Join-Path $Work 'corepack'
$env:npm_config_cache = Join-Path $Work 'npm-cache'
$env:CI = '1'

Push-Location $src
try {
    Run rustup target add wasm32-unknown-unknown
    Run corepack pnpm install --frozen-lockfile --ignore-scripts --store-dir (Join-Path $Work 'pnpm-store')
    # Not through Run: PowerShell would swallow the -- on its way through a function.
    & npx --yes "wasm-pack@$wasmPack" build --release ./pkm_rs --target web -- --features wasm
    if ($LASTEXITCODE -ne 0) { throw 'wasm-pack failed.' }
    Run node scripts/patch-types.mjs
    # It builds inside OpenHome's tree, which has the path names its code imports by.
    Copy-Item -LiteralPath (Join-Path $here 'convert.ts') (Join-Path $src 'nativemons-convert.ts') -Force
    $bundle = Join-Path $Out 'convert.cjs'
    & npx --yes "esbuild@$esbuild" nativemons-convert.ts --bundle --platform=node --format=cjs --target=node22 '--define:import.meta.env={}' '--define:import.meta.url=undefined' "--outfile=$bundle" --log-level=error
    if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $bundle)) { throw 'The converter did not bundle.' }
    Copy-Item -LiteralPath (Join-Path $src 'pkm_rs\pkg\pkm_rs_bg.wasm') $Out -Force
    Copy-Item -LiteralPath (Join-Path $src 'LICENSE') (Join-Path $Out 'LICENSE-OpenHome.txt') -Force
}
finally { Pop-Location }

$zip = Join-Path $Work 'node.zip'
Invoke-WebRequest "https://nodejs.org/dist/v$node/node-v$node-win-x64.zip" -OutFile $zip -UseBasicParsing
Expand-Archive -LiteralPath $zip -DestinationPath (Join-Path $Work 'node') -Force
Copy-Item -LiteralPath (Join-Path $Work "node\node-v$node-win-x64\node.exe") $Out -Force
Copy-Item -LiteralPath (Join-Path $Work "node\node-v$node-win-x64\LICENSE") (Join-Path $Out 'LICENSE-Node.txt') -Force
Set-Content -LiteralPath $mark $version
Write-Host 'Built.'

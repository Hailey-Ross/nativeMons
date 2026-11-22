# Builds the OpenHome converter into one folder: convert.cjs, its wasm, and a Node runtime to run them.
# Needs Git, Node and Rust. Used by launcher\build-all.ps1 and the release build. Versions are pinned here.
param([Parameter(Mandatory)][string]$Out, [Parameter(Mandatory)][string]$Work)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

# OpenHome comes from our fork, at the commit of the release tag (forks don't carry upstream's tags).
$openHome = 'v1.19.2'; $openHomeCommit = 'd5939596ebf67a1c9e63ceed9f846047e4efbedb'
$node = '24.16.0'; $wasmPack = '0.15.1'; $esbuild = '0.25.0'
# wasm-pack's own release, not its npm wrapper (that pulls in deprecated glob, rimraf and tar to fetch the same file).
$wasmPackSha = '80afe1f95aaa3f7442917c4457c8316bd1a383c1f27eb0e40601ff3ee3217583'
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
    Run git init -q $src
    Run git -C $src fetch -q --depth 1 https://github.com/Hailey-Ross/OpenHome.git $openHomeCommit
    Run git -C $src checkout -q FETCH_HEAD
}
$env:CARGO_TARGET_DIR = Join-Path $Work 'target'
$env:CARGO_BUILD_JOBS = '4'
$env:COREPACK_ENABLE_DOWNLOAD_PROMPT = '0'
$env:COREPACK_HOME = Join-Path $Work 'corepack'
$env:npm_config_cache = Join-Path $Work 'npm-cache'
$env:CI = '1'

$wpName = "wasm-pack-v$wasmPack-x86_64-pc-windows-msvc"
$wpExe = Join-Path $Work "$wpName\wasm-pack.exe"
if (-not (Test-Path -LiteralPath $wpExe)) {
    $wpTar = Join-Path $Work 'wasm-pack.tar.gz'
    Invoke-WebRequest "https://github.com/wasm-bindgen/wasm-pack/releases/download/v$wasmPack/$wpName.tar.gz" -OutFile $wpTar -UseBasicParsing
    if ((Get-FileHash -LiteralPath $wpTar -Algorithm SHA256).Hash -ne $wasmPackSha) { throw 'The wasm-pack download did not match its checksum.' }
    # Windows' own tar (System32), which reads .tar.gz.
    & "$env:SystemRoot\System32\tar.exe" -xzf $wpTar -C $Work
    if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $wpExe)) { throw 'wasm-pack did not unpack.' }
}

Push-Location $src
try {
    Run rustup target add wasm32-unknown-unknown
    Run corepack pnpm install --frozen-lockfile --ignore-scripts --store-dir (Join-Path $Work 'pnpm-store')
    # Not through Run: PowerShell would swallow the -- on its way through a function.
    & $wpExe build --release ./pkm_rs --target web -- --features wasm
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

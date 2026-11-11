# Builds all of nativeMons into one ready-to-run folder: the one named in build-output.txt, else build\nativeMons.
# The versions below match .github\actions\build-package\action.yml: change them together.
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$loveVersion = '11.5'
$fulldexRelease = 'fulldex-2026-11-08'
$platinumRelease = 'v0.2.1'
$nativeplatRelease = 'windows-095b8f537'

$here = $PSScriptRoot
$repo = Split-Path $here -Parent
$choice = Join-Path $repo 'build-output.txt'
$out = if (Test-Path -LiteralPath $choice) { (Get-Content -LiteralPath $choice | Where-Object { $_.Trim() } | Select-Object -First 1).Trim() } else { Join-Path $repo 'build\nativeMons' }
$work = Join-Path $out '.build'

function Step($text) { Write-Host "`n== $text" }

# Unpacks a zip from the web into a temporary folder and returns that folder.
function Fetch($url, $name) {
    New-Item -ItemType Directory -Force $work | Out-Null
    $zip = Join-Path $work "$name.zip"
    $unpacked = Join-Path $work $name
    Invoke-WebRequest $url -OutFile $zip -UseBasicParsing
    Expand-Archive -LiteralPath $zip -DestinationPath $unpacked -Force
    return $unpacked
}

Write-Host "Building nativeMons into $out"
New-Item -ItemType Directory -Force $out | Out-Null

Step 'Source'
foreach ($needed in 'engines\gen1recomp\main.lua', 'tools\nds-patcher\hails_patcher.py') {
    if (-not (Test-Path -LiteralPath (Join-Path $repo $needed))) {
        & git -C $repo submodule update --init
        if ($LASTEXITCODE -ne 0) { throw 'Could not fetch the engine source. Is Git installed?' }
        break
    }
}
Write-Host 'Present.'

Step 'Launcher'
& (Join-Path $here 'build.bat') $out
if ($LASTEXITCODE -ne 0) { throw 'The launcher did not build.' }
Copy-Item -LiteralPath (Join-Path $repo 'LICENSE'), (Join-Path $repo 'README.md') -Destination $out

# Records which published build this already contains, so the updater only offers newer ones.
Step 'Version'
$versionFile = Join-Path $out 'version.txt'
$channel = 'bleeding-edge'
if (Test-Path -LiteralPath $versionFile) {
    $line = Get-Content -LiteralPath $versionFile | Where-Object { $_ -like 'channel=*' } | Select-Object -First 1
    if ($line -eq 'channel=stable') { $channel = 'stable' }
}
$head = (& git -C $repo rev-parse HEAD).Trim()
$build = 0; $version = ''
try {
    $release = Invoke-RestMethod "https://github.com/Hailey-Ross/nativeMons/releases/download/$channel/release.json"
    & git -C $repo merge-base --is-ancestor $release.revision HEAD 2>$null
    if ($LASTEXITCODE -eq 0) { $build = [int]$release.build; $version = "$($release.version)" }
} catch { }
"channel=$channel`nbuild=$build`nrevision=$head`nversion=$version`nlocal=$head" | Set-Content -LiteralPath $versionFile -Encoding ascii
Write-Host "Local build of $($head.Substring(0, 7)), includes $channel build $build."

Step 'Gen1Recomp'
$gen1 = Join-Path $out 'engines\gen1recomp'
if (Test-Path -LiteralPath $gen1) { Remove-Item -LiteralPath $gen1 -Recurse -Force }
New-Item -ItemType Directory -Force $gen1 | Out-Null
$skip = '.git', '.github', 'docs', 'test', 'tests', 'mobile', 'ports', 'flatpak', 'scripts'
Get-ChildItem -LiteralPath (Join-Path $repo 'engines\gen1recomp') -Force | Where-Object { $skip -notcontains $_.Name } | Copy-Item -Destination $gen1 -Recurse
Write-Host 'Copied.'

Step "LOVE $loveVersion"
$love = Join-Path $out 'love'
$mark = Join-Path $love '.nativemons-version'
if ((Test-Path -LiteralPath $mark) -and (Get-Content -LiteralPath $mark) -eq $loveVersion) { Write-Host 'Already there.' }
else {
    $unpacked = Fetch "https://github.com/love2d/love/releases/download/$loveVersion/love-$loveVersion-win64.zip" 'love'
    if (Test-Path -LiteralPath $love) { Remove-Item -LiteralPath $love -Recurse -Force }
    Move-Item -LiteralPath (Join-Path $unpacked "love-$loveVersion-win64") $love
    Set-Content -LiteralPath $mark $loveVersion
    Write-Host 'Downloaded.'
}

# Fulldex keeps saves and copied ROMs in its own folder, so a new release goes over the top and nothing is removed.
Step "Fulldex $fulldexRelease"
$fulldex = Join-Path $out 'engines\fulldex'
$mark = Join-Path $fulldex '.nativemons-version'
# Upstream replaces this tag's files in place, so the mark holds their digest too. Offline, the tag alone decides.
$fulldexZip = 'pokemon-ds-wide-pc-windows-x86_64.zip'
$fulldexDigest = $null
try { $fulldexDigest = ((Invoke-RestMethod "https://api.github.com/repos/mamesj/pokemon-fulldex/releases/tags/$fulldexRelease").assets | Where-Object { $_.name -eq $fulldexZip }).digest } catch { }
$fulldexMark = if ($fulldexDigest) { "$fulldexRelease $fulldexDigest" } else { $fulldexRelease }
$haveMark = if (Test-Path -LiteralPath $mark) { [string](Get-Content -LiteralPath $mark) } else { '' }
if ($haveMark -eq $fulldexMark -or (-not $fulldexDigest -and $haveMark -like "$fulldexRelease*")) { Write-Host 'Already there.' }
else {
    $unpacked = Fetch "https://github.com/mamesj/pokemon-fulldex/releases/download/$fulldexRelease/pokemon-ds-wide-pc-windows-x86_64.zip" 'fulldex'
    New-Item -ItemType Directory -Force $fulldex | Out-Null
    Copy-Item -Path (Join-Path $unpacked 'pokemon-ds-wide\*') -Destination $fulldex -Recurse -Force
    Set-Content -LiteralPath $mark $fulldexMark
    Write-Host 'Downloaded.'
}

# The Platinum PC port: the program only. Its game data comes from the player's ROM.
Step "Platinum PC port $platinumRelease"
$port = Join-Path $out 'engines\pokeplatinum'
$mark = Join-Path $port '.nativemons-version'
if ((Test-Path -LiteralPath $mark) -and (Get-Content -LiteralPath $mark) -eq $platinumRelease) { Write-Host 'Already there.' }
else {
    $unpacked = Fetch "https://github.com/cybervisi0n/pokeplatinum/releases/download/$platinumRelease/release_win64.zip" 'pokeplatinum'
    New-Item -ItemType Directory -Force $port | Out-Null
    Copy-Item -Path (Join-Path $unpacked '*') -Destination $port -Recurse -Force
    Copy-Item -LiteralPath (Join-Path $here 'LICENSE-libntr.txt') -Destination $port -Force
    Set-Content -LiteralPath $mark $platinumRelease
    Write-Host 'Downloaded.'
}

# nativeplat: the program only, from our fork's workflow. ROMs and saves go in userdata.
Step "nativeplat $nativeplatRelease"
$nativeplat = Join-Path $out 'engines\nativeplat'
$mark = Join-Path $nativeplat '.nativemons-version'
if ((Test-Path -LiteralPath $mark) -and (Get-Content -LiteralPath $mark) -eq $nativeplatRelease) { Write-Host 'Already there.' }
else {
    $unpacked = Fetch "https://github.com/Hailey-Ross/gen4recomp/releases/download/$nativeplatRelease/nativeplat-windows-x64.zip" 'nativeplat'
    New-Item -ItemType Directory -Force $nativeplat | Out-Null
    Copy-Item -Path (Join-Path $unpacked 'nativeplat-windows-x64\*') -Destination $nativeplat -Recurse -Force
    Set-Content -LiteralPath (Join-Path $nativeplat 'portable.txt') ''
    Set-Content -LiteralPath $mark $nativeplatRelease
    Write-Host 'Downloaded.'
}

Step 'hails.nds-Patcher\'
$patcher = Join-Path $out 'tools\nds-patcher'
New-Item -ItemType Directory -Force $patcher | Out-Null
Copy-Item -Path (Join-Path $repo 'tools\nds-patcher\*.py') -Destination $patcher -Force
Copy-Item -LiteralPath (Join-Path $repo 'tools\nds-patcher\LICENSE') -Destination $patcher -Force
Write-Host 'Copied.'

# Optional: mGBA's libretro core, for romhacks' title screens. Without it they show their base game's.
Step 'mGBA core'
$mgba = Join-Path $out 'tools\mgba'
if (Test-Path -LiteralPath (Join-Path $mgba 'mgba_libretro.dll')) { Write-Host 'Already there.' }
else {
    try {
        $unpacked = Fetch 'https://buildbot.libretro.com/nightly/windows/x86_64/latest/mgba_libretro.dll.zip' 'mgba'
        New-Item -ItemType Directory -Force $mgba | Out-Null
        Copy-Item -LiteralPath (Join-Path $unpacked 'mgba_libretro.dll') -Destination $mgba -Force
        Invoke-WebRequest 'https://raw.githubusercontent.com/mgba-emu/mgba/master/LICENSE' -OutFile (Join-Path $mgba 'LICENSE-mGBA.txt') -UseBasicParsing
        Write-Host 'Downloaded.'
    } catch { Write-Host "Skipped: $($_.Exception.Message) Romhacks show their base game's title screen." }
}

# Optional: our GBA player (engines\fulldex\gba-wide), built with CMake, Ninja and MinGW GCC with SDL2 (MSYS2 UCRT64).
# Without it GBA games play in Fulldex as before.
Step 'GBA player'
$gbaSource = Join-Path $repo 'engines\fulldex\gba-wide'
$gbaTools = 'cmake', 'ninja', 'g++' | Where-Object { -not (Get-Command $_ -ErrorAction SilentlyContinue) }
if (-not (Test-Path -LiteralPath (Join-Path $gbaSource 'CMakeLists.txt'))) { Write-Host 'Skipped: the Fulldex fork has no gba-wide folder at this pin.' }
elseif ($gbaTools) { Write-Host "Skipped: needs $($gbaTools -join ', ') (MSYS2 UCRT64 with SDL2). GBA games play in Fulldex." }
else {
    $gbaWork = Join-Path $work 'gbawide'
    & cmake -S $gbaSource -B $gbaWork -G Ninja -DCMAKE_BUILD_TYPE=Release | Out-Null
    if ($LASTEXITCODE -eq 0) { & cmake --build $gbaWork --parallel 4 | Out-Null }
    if ($LASTEXITCODE -ne 0) { Write-Host 'Skipped: it did not build. GBA games play in Fulldex.' }
    else {
        $gba = Join-Path $out 'engines\gbawide'
        New-Item -ItemType Directory -Force $gba | Out-Null
        Copy-Item -LiteralPath (Join-Path $gbaWork 'gbawide.exe') -Destination $gba -Force
        Copy-Item -LiteralPath (Join-Path $gbaSource 'README.md') -Destination (Join-Path $gba 'README.md') -Force
        Copy-Item -LiteralPath (Join-Path $repo 'engines\fulldex\LICENSE') -Destination (Join-Path $gba 'LICENSE.txt') -Force
        Invoke-WebRequest 'https://raw.githubusercontent.com/mgba-emu/mgba/master/LICENSE' -OutFile (Join-Path $gba 'LICENSE-mGBA.txt') -UseBasicParsing
        $rc = Join-Path $repo 'engines\fulldex\frontend\thirdparty\rcheevos\LICENSE'
        if (Test-Path -LiteralPath $rc) { Copy-Item -LiteralPath $rc -Destination (Join-Path $gba 'LICENSE-rcheevos.txt') -Force }
        $pd = Join-Path $gbaSource 'third_party\pokedaisy'
        if (Test-Path -LiteralPath (Join-Path $pd 'NOTICE')) {
            Copy-Item -LiteralPath (Join-Path $pd 'NOTICE') -Destination (Join-Path $gba 'NOTICE-PokeDaisy.txt') -Force
            Copy-Item -LiteralPath (Join-Path $pd 'LICENSE') -Destination (Join-Path $gba 'LICENSE-PokeDaisy.txt') -Force
        }
        Write-Host 'Built.'
    }
}

Step 'mon-bridge'
$bridgeProject = Join-Path $repo 'tools\mon-bridge\MonBridge.csproj'
if (-not (Get-Command dotnet -ErrorAction SilentlyContinue)) {
    throw 'mon-bridge needs the .NET 10 SDK: https://dotnet.microsoft.com/download/dotnet/10.0'
}
$bridgeOut = Join-Path $out 'tools\mon-bridge'
& dotnet publish $bridgeProject -c Release -m:4 -v q -o $bridgeOut "-p:BaseIntermediateOutputPath=$work/obj/" "-p:BaseOutputPath=$work/bin/"
if ($LASTEXITCODE -ne 0) { throw 'mon-bridge did not build.' }

# Optional: without it, moves that need a hand copy still work, with the simpler copy built into mon-bridge.
Step 'OpenHome converter'
$converter = Join-Path $out 'tools\openhome'
$tools = 'git', 'node', 'corepack', 'rustup' | Where-Object { -not (Get-Command $_ -ErrorAction SilentlyContinue) }
if ($tools) { Write-Host "Skipped: needs $($tools -join ', '). Moves use the simpler copy." }
else {
    $openHomeWork = Join-Path $work 'openhome'
    try { & (Join-Path $repo 'tools\openhome-bridge\build.ps1') -Out $converter -Work $openHomeWork }
    catch { Write-Host "Skipped: $($_.Exception.Message) Moves use the simpler copy." }
    Set-Location $repo
}

Step 'Checking the result\'
if (Test-Path -LiteralPath $work) { Remove-Item -LiteralPath $work -Recurse -Force }
foreach ($needed in 'NativeMonsLauncher.exe', 'games.json', 'love\lovec.exe', 'engines\gen1recomp\main.lua', 'engines\fulldex\Pokemon Fulldex.exe', 'engines\pokeplatinum\main.exe', 'engines\nativeplat\nativeplat.exe', 'tools\nds-patcher\hails_patcher.py', 'tools\mon-bridge\mon-bridge.exe') {
    if (-not (Test-Path -LiteralPath (Join-Path $out $needed))) { throw "Missing from the build: $needed" }
}
foreach ($removed in 'assets\launcher', 'assets\labels', 'src\import\LauncherView.lua', 'src\import\RomImporter.lua', 'src\mods\LauncherMods.lua') {
    if (Test-Path -LiteralPath (Join-Path $gen1 $removed)) { throw "Removed launcher file came back: $removed" }
}

Write-Host "`nDone. Start it here:`n$(Join-Path $out 'NativeMonsLauncher.exe')"

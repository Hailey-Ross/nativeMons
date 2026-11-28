# The licenses file packages ship beside the Rust launcher: every crate linked into the exe (normal dependencies of
# nativemons for one target, no build tools or proc-macros), each with its license files from the crate's source.
# crate-licenses.ps1 -Out <file> [-Features shipped] [-Target <triple>]. Reads `cargo metadata` only; runs no build.
param([Parameter(Mandatory = $true)][string]$Out, [string]$Features = "shipped", [string]$Target = "")
$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
if (-not $Target) { $Target = ((& rustc -vV) | Where-Object { $_ -like 'host: *' }).Substring(6).Trim() }
$json = (& cargo metadata --format-version 1 --locked --manifest-path (Join-Path $root 'crates\nativemons\Cargo.toml') --features $Features --filter-platform $Target) -join "`n"
if ($LASTEXITCODE -ne 0) { throw "cargo metadata failed" }
$m = $json | ConvertFrom-Json
$pk = @{}; foreach ($p in $m.packages) { $pk[$p.id] = $p }
$nodes = @{}; foreach ($n in $m.resolve.nodes) { $nodes[$n.id] = $n }
# The crates in the exe for that target: cargo tree evaluates each dependency's cfg (metadata leaves that to us).
$tree = @(& cargo tree --locked --manifest-path (Join-Path $root 'crates\nativemons\Cargo.toml') --features $Features --target $Target -e normal,no-proc-macro --prefix none -f "{p}")
if ($LASTEXITCODE -ne 0) { throw "cargo tree failed" }
$want = @{}; foreach ($l in $tree) { if ($l -match '^(\S+) v(\S+)') { $want["$($Matches[1]) $($Matches[2])"] = $true } }
$seen = @{}
foreach ($p in $m.packages) { if ($want.ContainsKey("$($p.name) $($p.version)")) { $seen[$p.id] = $true } }
# A vendored C library built by a build dependency (SDL 3 from sdl3-src) is linked in too.
foreach ($id in @($seen.Keys)) {
    foreach ($d in $nodes[$id].deps) { if ($pk[$d.pkg].name -like '*-src' -and @($d.dep_kinds | Where-Object { $_.kind -eq 'build' }).Count) { $seen[$d.pkg] = $true } }
}
$crates = @($seen.Keys | ForEach-Object { $pk[$_] } | Where-Object { $_.source -and -not @($_.targets | Where-Object { $_.kind -contains 'proc-macro' }).Count } | Sort-Object name, version)
# Vendored C libraries keep their license below the crate's root.
$extra = @{ 'sdl3-src' = @('SDL\LICENSE.txt') }
$names = '^(licen[cs]e|copying|notice|copyright|unlicense)([-_.].*)?$'
$sb = New-Object Text.StringBuilder
[void]$sb.AppendLine("Rust crates in the nativeMons launcher (the Rust build of NativeMonsLauncher.exe, $Target), with their licenses.")
[void]$sb.AppendLine("Made from launcher-rs/Cargo.lock by launcher-rs/tools/crate-licenses.ps1. $($crates.Count) crates.")
[void]$sb.AppendLine("")
[void]$sb.AppendLine("This software is based in part on the work of the Independent JPEG Group (libjpeg-turbo, by way of mozjpeg).")
[void]$sb.AppendLine("SDL 3 (Simple DirectMedia Layer) is statically linked, under the zlib license (sdl3-src below).")
[void]$sb.AppendLine("The fonts' license is fonts\OFL.txt (Noto Sans) and fonts\NotoSansSymbols2-OFL.txt (Noto Sans Symbols 2).")
[void]$sb.AppendLine("")
foreach ($c in $crates) { [void]$sb.AppendLine(("{0,-32} {1,-14} {2}" -f $c.name, $c.version, $(if ($c.license) { $c.license } else { "see its license file" }))) }
$texts = @{}
foreach ($c in $crates) {
    $dir = Split-Path $c.manifest_path -Parent
    [void]$sb.AppendLine("")
    [void]$sb.AppendLine("=" * 100)
    [void]$sb.AppendLine("$($c.name) $($c.version)  ($(if ($c.license) { $c.license } else { 'license file' }))$(if ($c.repository) { "  $($c.repository)" })")
    [void]$sb.AppendLine("=" * 100)
    $files = @(Get-ChildItem -LiteralPath $dir -File | Where-Object { $_.Name -match $names } | Sort-Object Name | ForEach-Object { $_.FullName })
    if ($c.license_file) { $files += (Join-Path $dir $c.license_file) }
    if ($extra.ContainsKey($c.name)) { $files += @($extra[$c.name] | ForEach-Object { Join-Path $dir $_ }) }
    $files = @($files | Where-Object { Test-Path -LiteralPath $_ } | Sort-Object -Unique)
    if (-not $files.Count) { [void]$sb.AppendLine("(The crate holds no license file; its license is $($c.license).)"); continue }
    foreach ($f in $files) {
        $t = [IO.File]::ReadAllText($f).Replace("`r`n", "`n").Trim()
        $key = [BitConverter]::ToString([Security.Cryptography.SHA256]::Create().ComputeHash([Text.Encoding]::UTF8.GetBytes($t)))
        [void]$sb.AppendLine("--- " + $f.Substring($dir.Length + 1).Replace('\', '/'))
        if ($texts.ContainsKey($key)) { [void]$sb.AppendLine("(The same text as $($texts[$key]) above.)"); continue }
        $texts[$key] = "$($c.name) $($c.version)"
        [void]$sb.AppendLine($t)
    }
}
$dirOut = Split-Path $Out -Parent
if ($dirOut) { New-Item -ItemType Directory -Force $dirOut | Out-Null }
[IO.File]::WriteAllText($Out, $sb.ToString().Replace("`r`n", "`n").Replace("`n", "`r`n"), (New-Object Text.UTF8Encoding $false))
Write-Host "$($crates.Count) crates, $($texts.Count) license texts: $Out"

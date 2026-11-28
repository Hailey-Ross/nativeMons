# A scratch golden root with Gen1Recomp: goldens-gen1\{csharp,rust}, each with copies of the app's engines\gen1recomp
# and love and mon-bridge (copied from the app, never changed there), and a local mod index (state\mod-index.json, so nothing is
# fetched). Run run-goldens.ps1 -Root <work>\goldens-gen1 against it. Headless only:
# LOVE runs with NATIVEMONS_CLI (no window). goldens-gen1.ps1 make | clean.
param([ValidateSet("make", "clean")][string]$Do = "make")
$ErrorActionPreference = "Stop"
. "$PSScriptRoot\work.ps1"
$root = "$Work\goldens-gen1"
if ($Do -eq "clean") { if (Test-Path $root) { Remove-Item -Recurse -Force $root }; return }
$app = "$App"
$base = "$Work\goldens\csharp"
$index = @'
{"schema_version": 1, "mods": [
 {"id": "fast_text", "title": "Fast Text", "author": "Sample Author", "summary": "Text prints at once.", "games": ["red", "blue"], "categories": ["QOL"], "license": "MIT", "repo": "https://example.invalid/fast_text",
  "latest": {"version": "1.2.0", "zip": {"url": "https://example.invalid/fast_text.zip", "size": 52000}}, "downloads": {"total": 950}},
 {"id": "kanto_colors", "title": "Kanto Colors", "author": "Sample Author", "summary": "A new palette for every town.", "games": ["red"], "categories": ["ART", "UI"], "permissions": ["filesystem"],
  "dependencies": ["fast_text@>=1.0"], "conflicts": ["mono_palette"],
  "latest": {"version": "0.9.1", "zip": {"url": "https://example.invalid/kanto_colors.zip", "size": 1840000}}, "downloads": {"total": 420}},
 {"id": "mono_palette", "title": "Mono Palette", "author": "Another Author", "summary": "The original greens.", "games": ["gen1"], "categories": ["ART"], "experimental": true,
  "conflicts": ["kanto_colors"], "latest": {"version": "2.0.0", "zip": {"url": "https://example.invalid/mono_palette.zip", "size": 9000}}, "downloads": {"total": 77}}
]}
'@
foreach ($side in "csharp", "rust") {
    $d = "$root\$side"
    New-Item -ItemType Directory -Force "$d\engines", "$d\state" | Out-Null
    robocopy "$app\engines\gen1recomp" "$d\engines\gen1recomp" /E /NFL /NDL /NJH /NJS /NP | Out-Null
    robocopy "$app\love" "$d\love" /E /NFL /NDL /NJH /NJS /NP | Out-Null
    New-Item -ItemType Directory -Force "$d\tools\mon-bridge" | Out-Null
    Copy-Item "$app\tools\mon-bridge\mon-bridge.exe" "$d\tools\mon-bridge\"
    [IO.File]::WriteAllText("$d\state\mod-index.json", $index)
    # Two sample mods in the engine copy: one needs a mod that isn't there, and they clash while both are on.
    foreach ($m in @(
        @{ id = "kanto_colors"; name = "Kanto Colors"; deps = '["fast_text@>=1.0"]'; clash = '["mono_palette"]' },
        @{ id = "mono_palette"; name = "Mono Palette"; deps = '[]'; clash = '[]' })) {
        $md = "$d\engines\gen1recomp\mods\$($m.id)"
        New-Item -ItemType Directory -Force $md | Out-Null
        [IO.File]::WriteAllText("$md\main.lua", "return {}`n")
        [IO.File]::WriteAllText("$md\manifest.json", "{`"id`": `"$($m.id)`", `"name`": `"$($m.name)`", `"version`": `"0.9.1`", `"entry`": `"main.lua`", `"category`": `"ART`", `"priority`": 100, `"dependencies`": $($m.deps), `"optional_dependencies`": [], `"conflicts`": $($m.clash), `"games`": [`"gen1`"], `"description`": `"A sample mod for the goldens.`"}")
    }
}
Copy-Item "$base\NativeMonsLauncher.exe", "$base\games.json" "$root\csharp" -Force
robocopy "$base\fonts" "$root\csharp\fonts" /E /NFL /NDL /NJH /NJS /NP | Out-Null
"made $root"

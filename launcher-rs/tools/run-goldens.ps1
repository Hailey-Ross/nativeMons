# Parity run, offscreen only: C# goldens (with -CSharp), Rust snapshots, then golden-diff per screen.
# Output under <work>\goldens\{csharp,rust} (tools\work.ps1). Never touches the user's app folder.
# The fixture (goldens\fixture\roms, copies of ROMs) is each side's ROM folder; placeholder engine files make
# Fulldex and the Platinum port count as installed. -NoRust: only the C# goldens. -Anim: frame sequences.
# -Screens and -Anim take one semicolon-separated string each (powershell -File passes no arrays).
# -Root: another golden root (goldens-gen1.ps1 makes one with Gen1Recomp); other sizes get a -WxH file suffix.
param([switch]$CSharp, [switch]$NoRust, [string]$Screens = "splash;credits;settings;main", [string]$Size = "600x1000",
      [string]$Anim = "", [string]$AnimSpan = "0 1.2 30", [string]$Root = "")
$ErrorActionPreference = "Stop"
. "$PSScriptRoot\work.ps1"
$screenList = @($Screens -split ';' | Where-Object { $_ }); $animList = @($Anim -split ';' | Where-Object { $_ })
if (-not $Root) { $Root = "$Work\goldens" }
$root = $Root
$target = "$TargetDir\debug"
$fixture = if ($env:NATIVEMONS_FIXTURE) { $env:NATIVEMONS_FIXTURE } else { "$Work\goldens\fixture\roms" }
$env:NATIVEMONS_TEST_T = "7.25"; $env:NATIVEMONS_TEST_SEED = "3"; $env:NATIVEMONS_TEST_BATTERY = "60"
$env:NATIVEMONS_TEST_FONTS = "bundled"; $env:NATIVEMONS_SNAPSHOT_TEXTMASK = "1"; $env:NATIVEMONS_TEST_QUIET = "1"
$env:NATIVEMONS_TEST_DEX = "sample"; $env:NATIVEMONS_TEST_NO_CAPTURE = "1"; $env:NATIVEMONS_TEST_CSHARP_ROWS = "1"
function Prepare-Side($dir) {
    New-Item -ItemType Directory -Force "$dir\state", "$dir\appdata", "$dir\out", "$dir\engines\fulldex", "$dir\engines\pokeplatinum" | Out-Null
    foreach ($f in "$dir\engines\fulldex\Pokemon Fulldex.exe", "$dir\engines\pokeplatinum\main.exe") { if (-not (Test-Path -LiteralPath $f)) { New-Item -ItemType File $f | Out-Null } }
    if (Test-Path $fixture) { [IO.File]::WriteAllText("$dir\state\rom-folders.txt", "$fixture`r`n") }
}
function Safe($s) { $n = $s -replace '[:,\\/ ]', '-'; if ($Size -ne '600x1000') { $n += "-$Size" }; $n }
$cs = "$root\csharp"
if ($CSharp) {
    Prepare-Side $cs
    $env:NATIVEMONS_STATE = "$cs\state"; $env:APPDATA = "$cs\appdata"
    foreach ($s in $screenList) { & "$cs\NativeMonsLauncher.exe" --snapshot "$cs\out\$(Safe $s).png" $s $Size | Out-Null }
    $span = $AnimSpan -split ' '
    foreach ($s in $animList) { $d = "$cs\anim\$(Safe $s)"; New-Item -ItemType Directory -Force $d | Out-Null; & "$cs\NativeMonsLauncher.exe" --snapshot-anim $d $s $Size $span[0] $span[1] $span[2] | Out-Null }
}
if ($NoRust) { return }
# The Rust side runs from its own launcher folder (an exe copy beside games.json), as the C# does.
$rs = "$root\rust"
Prepare-Side $rs
New-Item -ItemType Directory -Force "$rs\heat", "$rs\fonts" | Out-Null
Copy-Item "$target\nativemons.exe", "$cs\games.json" $rs -Force
# The Rust side's fonts: the bundled Noto set plus launcher-rs\fonts (the Noto Sans Symbols 2 fallback).
Copy-Item "$cs\fonts\*", "$PSScriptRoot\..\fonts\*" "$rs\fonts" -Force
$env:NATIVEMONS_STATE = "$rs\state"; $env:APPDATA = "$rs\appdata"; $env:NATIVEMONS_FONTS_DIR = "$rs\fonts"
# Screens the user decided may differ (deliberate-screens.txt) are compared as usual, and their line says so.
$deliberate = @{}
foreach ($l in Get-Content "$PSScriptRoot\deliberate-screens.txt" -ErrorAction SilentlyContinue) { if ($l -and -not $l.StartsWith('#')) { $f = $l -split "`t", 2; $deliberate[$f[0]] = $f[1] } }
foreach ($s in $screenList) {
    $n = Safe $s
    & "$rs\nativemons.exe" --snapshot "$rs\out\$n.png" $s $Size
    $line = & "$target\golden-diff.exe" "$cs\out\$n.png" "$rs\out\$n.png" $s "$rs\heat\$n.png"
    if ($deliberate.ContainsKey($s)) { $line = @($line | ForEach-Object { "$_ [deliberate difference, not a parity failure: $($deliberate[$s])]" }) }
    $line
}
$span = $AnimSpan -split ' '
foreach ($s in $animList) {
    $n = Safe $s; $d = "$rs\anim\$n"; New-Item -ItemType Directory -Force $d | Out-Null
    & "$rs\nativemons.exe" --snapshot-anim $d $s $Size $span[0] $span[1] $span[2]
    Get-ChildItem "$cs\anim\$n" -Filter "frame-*.png" | ForEach-Object { & "$target\golden-diff.exe" $_.FullName "$d\$($_.Name)" "$s/$($_.BaseName)" }
}

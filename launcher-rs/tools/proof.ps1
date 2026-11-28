# Byte proof for changes that must not move a pixel (speed work): the Rust launcher's every output on the parity set,
# hashed, so two builds can be compared file by file. Rust side only; the C# isn't needed.
#   proof.ps1 run -Exe <nativemons.exe> -Tag base      # snapshots, anims and every replay frame, hashed into <work>\proof-<tag>.txt
#   proof.ps1 compare -Tag base -Other after          # files that differ, are missing or extra
# Each run starts from the same fixture state: the golden roots' rust sides are copied from a pristine copy made by
# the first run (pristine-<root>), so dry runs (ROM copies, extraction) can't leave anything for the next. Needs the roots
# goldens, goldens-gen1 (with the Gen1Recomp slot) and goldens-boxes under <work> (tools\work.ps1), and their C# sides.
# -Roots: which golden roots (goldens-gen1 runs Gen1Recomp's command line: leave it out when no engine may run).
param([ValidateSet("run", "compare")][string]$Do = "run", [string]$Exe = "", [string]$Tag = "base", [string]$Other = "after", [switch]$Damage, [string]$Roots = "goldens;goldens-gen1;goldens-boxes")
$ErrorActionPreference = "Stop"
. "$PSScriptRoot\work.ps1"
$manifest = { param($t) "$Work\proof-$t.txt" }
if ($Do -eq "compare") {
    $a = @{}; foreach ($l in Get-Content (& $manifest $Tag)) { $p = $l -split "`t"; $a[$p[1]] = $p[0] }
    $b = @{}; foreach ($l in Get-Content (& $manifest $Other)) { $p = $l -split "`t"; $b[$p[1]] = $p[0] }
    $diff = @($a.Keys | Where-Object { $b.ContainsKey($_) -and $a[$_] -ne $b[$_] } | Sort-Object)
    $gone = @($a.Keys | Where-Object { -not $b.ContainsKey($_) } | Sort-Object); $new = @($b.Keys | Where-Object { -not $a.ContainsKey($_) } | Sort-Object)
    "{0} files in {1}, {2} in {3}: {4} differ, {5} only in {1}, {6} only in {3}" -f $a.Count, $Tag, $b.Count, $Other, $diff.Count, $gone.Count, $new.Count
    $diff + $gone + $new | Select-Object -First 30
    return
}
if (-not (Test-Path $Exe)) { throw "no exe at $Exe" }
# A target folder of its own for this exe, so the scripts copy it (and golden-diff) from there.
$t = "$Work\proof-exe-$Tag\debug"; New-Item -ItemType Directory -Force $t | Out-Null
Copy-Item $Exe "$t\nativemons.exe" -Force; Copy-Item "$TargetDir\debug\golden-diff.exe" $t -Force
$keep = $env:CARGO_TARGET_DIR; $env:CARGO_TARGET_DIR = Split-Path $t -Parent
if ($Damage) { $env:NATIVEMONS_TEST_DAMAGE = "1" } else { Remove-Item env:NATIVEMONS_TEST_DAMAGE -ErrorAction SilentlyContinue }
$rootNames = @($Roots -split '[;, ]' | Where-Object { $_ })
$has = { param($r) $rootNames -contains $r }
foreach ($r in $rootNames) {
    $side = "$Work\$r\rust"; $pristine = "$Work\pristine-$r"
    New-Item -ItemType Directory -Force $side | Out-Null
    if (-not (Test-Path $pristine)) { New-Item -ItemType Directory -Force $pristine | Out-Null; robocopy $side $pristine /E /XD "$side\out" "$side\replay" "$side\heat" "$side\anim" /NFL /NDL /NJH /NJS /NP | Out-Null }
    if (Test-Path $side) { Remove-Item -Recurse -Force $side }
    robocopy $pristine $side /E /NFL /NDL /NJH /NJS /NP | Out-Null
}
$g = "$PSScriptRoot\run-goldens.ps1"; $rp = "$PSScriptRoot\run-replay.ps1"
$main = "main;games;settings;gameoptions;game;folders;emu;credits;wallpapers;browse;import;setup-1;updatebanner;splash;main:sun;main:moon;main:omegaruby;main:ultramoon"
if (& $has "goldens") { & powershell -NoProfile -File $g -Root "$Work\goldens" -Screens $main -Anim "open-Games;poweroff" | Out-Null }
if (& $has "goldens") { & powershell -NoProfile -File $g -Root "$Work\goldens" -Screens "main" -Size 1600x1000 | Out-Null }
if (& $has "goldens-gen1") { & powershell -NoProfile -File $g -Root "$Work\goldens-gen1" -Screens "mods;dex;dexentry;saves:red;saves:crystal" | Out-Null }
if (& $has "goldens-boxes") { foreach ($s in "600x1000", "1600x1000") { & powershell -NoProfile -File $g -Root "$Work\goldens-boxes" -Screens "boxes;boxheld;boxchoose" -Size $s | Out-Null } }
if (& $has "goldens") { & powershell -NoProfile -File $rp -Frames -KeepFrames -Root "$Work\goldens" -Scripts "keys-games;pad-settings;pad-gameoptions;pad-repeat;mouse-folders;mouse-back;back-main;games-game;wheel-settings;controls;controls-sheet;setup-leave;setup-steps;rust-pages" | Out-Null }
if (& $has "goldens-boxes") { & powershell -NoProfile -File $rp -Frames -KeepFrames -Root "$Work\goldens-boxes" -Scripts "tablet-morph;rust-pages-3ds;rust-pages-pc" | Out-Null }
if (& $has "goldens-boxes") { & powershell -NoProfile -File $rp -Frames -KeepFrames -Root "$Work\goldens-boxes" -Scripts "box-drag;box-keys" -Size 1600x1000 | Out-Null }
if (& $has "goldens-gen1") { & powershell -NoProfile -File $rp -Frames -KeepFrames -Root "$Work\goldens-gen1" -Scripts "gen1-dex;gen1-dexlist;gen1-mods;gen1-saves" | Out-Null }
if (& $has "goldens") { & powershell -NoProfile -File $rp -Frames -KeepFrames -DryRun -Root "$Work\goldens" -Scripts "play-dry;play-dry-port" | Out-Null }
if (& $has "goldens-gen1") { & powershell -NoProfile -File $rp -Frames -KeepFrames -DryRun -Root "$Work\goldens-gen1" -Scripts "play-dry-gen1" | Out-Null }
if (& $has "goldens-boxes") { & powershell -NoProfile -File $rp -Frames -KeepFrames -DryRun -Root "$Work\goldens-boxes" -Scripts "play-dry-pc" | Out-Null }
$env:CARGO_TARGET_DIR = $keep
# Every output file, hashed, by its path under the root (the run's own logs of timings are left out).
$lines = foreach ($r in $rootNames) {
    $side = "$Work\$r\rust"
    foreach ($d in "out", "anim", "replay") {
        if (-not (Test-Path "$side\$d")) { continue }
        Get-ChildItem -Recurse -File "$side\$d" | Where-Object { $_.Name -ne "discord.txt" } | ForEach-Object { "{0}`t{1}" -f (Get-FileHash -Algorithm SHA256 $_.FullName).Hash, $_.FullName.Substring($Work.Length + 1) }
    }
}
$lines | Set-Content -Encoding utf8 (& $manifest $Tag)
"$Tag`: $(@($lines).Count) files hashed into $(& $manifest $Tag)"
if ($Damage) { foreach ($r in $rootNames) { Get-ChildItem -Recurse "$Work\$r\rust\replay" -Filter damage.txt -ErrorAction SilentlyContinue | ForEach-Object { "$($_.Directory.Name): $(Get-Content $_.FullName -TotalCount 1)" } } }
Remove-Item -Recurse -Force "$Work\proof-exe-$Tag"

# A scratch golden root for Move Pokemon: goldens-boxes\{csharp,rust}, each with mon-bridge (copied from the app)
# and copies of two saves (FireRed, Platinum) as Fulldex's saves. The originals are only read, hashed before and
# after. Run run-goldens.ps1 -Root <work>\goldens-boxes. goldens-boxes.ps1 make | clean.
param([ValidateSet("make", "clean")][string]$Do = "make")
$ErrorActionPreference = "Stop"
. "$PSScriptRoot\work.ps1"
$root = "$Work\goldens-boxes"
if ($Do -eq "clean") { if (Test-Path $root) { Remove-Item -Recurse -Force $root }; return }
$app = "$App"
$base = "$Work\goldens\csharp"
$saves = "firered.sav", "platinum.sav"
$lines = @()
foreach ($side in "csharp", "rust") {
    $d = "$root\$side"
    New-Item -ItemType Directory -Force "$d\tools\mon-bridge", "$d\engines\fulldex\saves" | Out-Null
    Copy-Item "$app\tools\mon-bridge\mon-bridge.exe" "$d\tools\mon-bridge\"
    foreach ($s in $saves) {
        $src = "$app\engines\fulldex\saves\$s"
        $before = (Get-FileHash -LiteralPath $src -Algorithm SHA256).Hash
        Copy-Item -LiteralPath $src "$d\engines\fulldex\saves\$s"
        $after = (Get-FileHash -LiteralPath $src -Algorithm SHA256).Hash
        $lines += "$side`t$s`t$before`t$after`t$(if ($before -eq $after) { 'unchanged' } else { 'CHANGED' })"
    }
}
Copy-Item "$base\NativeMonsLauncher.exe", "$base\games.json" "$root\csharp" -Force
robocopy "$base\fonts" "$root\csharp\fonts" /E /NFL /NDL /NJH /NJS /NP | Out-Null
$lines | Set-Content -Encoding utf8 "$root\originals.txt"
$lines

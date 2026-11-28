# The ROM fixture run-goldens.ps1 uses: copies of six of the user's ROMs (read only, hashed before and after).
# goldens-fixture.ps1 make | clean. Copies go to <work>\goldens\fixture\roms (tools\work.ps1); delete them after.
param([ValidateSet("make", "clean")][string]$Do = "make", [string]$Fixture = "")
$ErrorActionPreference = "Stop"
. "$PSScriptRoot\work.ps1"
if (-not $Fixture) { $Fixture = "$Work\goldens\fixture" }
$fx = $Fixture
if ($Do -eq "clean") { if (Test-Path $fx) { Remove-Item -Recurse -Force $fx }; return }
$o = [ordered]@{
    red      = (Rom-Path "red")
    crystal  = (Rom-Path "crystal")
    crystal2 = (Rom-Path "crystal2")
    firered  = (Rom-Path "firered")
    platinum = (Rom-Path "platinum")
    black    = (Rom-Path "black")
}
$lines = @()
foreach ($k in $o.Keys) {
    $before = (Get-FileHash -LiteralPath $o[$k] -Algorithm SHA256).Hash
    New-Item -ItemType Directory -Force "$fx\roms\$k" | Out-Null
    Copy-Item -LiteralPath $o[$k] "$fx\roms\$k"
    $after = (Get-FileHash -LiteralPath $o[$k] -Algorithm SHA256).Hash
    $lines += "$k`t$before`t$after`t$(if ($before -eq $after) { 'unchanged' } else { 'CHANGED' })"
}
$lines | Set-Content -Encoding utf8 "$fx\originals.txt"
$lines

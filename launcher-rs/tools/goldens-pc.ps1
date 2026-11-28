# A scratch root for the PC game installs and the save flows: goldens-pc\{csharp,rust} (mon-bridge from the app, a
# copy of the app's FireRed save as Fulldex's, two backups of it, one damaged) and goldens-pc\data: a fake Infinite
# Fusion (folder, zip, local git repo), a scratch roaming folder with one .rxdata, and saves to drop. The FireRed ROM
# and save are only read, hashed before and after. goldens-pc.ps1 make -CSharpExe <exe> | commit <subject> | clean.
param([ValidateSet("make", "commit", "clean")][string]$Do = "make", [string]$CSharpExe = "", [string]$Subject = "[6.8.3] The next release")
$ErrorActionPreference = "Stop"
. "$PSScriptRoot\work.ps1"
$root = "$Work\goldens-pc"; $data = "$root\data"
function Git-In([string]$dir) { & git -C $dir -c user.name=test -c user.email=test@example.invalid -c commit.gpgsign=false @args | Out-Null; if ($LASTEXITCODE) { throw "git $args failed" } }
if ($Do -eq "clean") { if (Test-Path $root) { Get-ChildItem -Recurse -Force $root | Where-Object { -not $_.PSIsContainer } | ForEach-Object { $_.IsReadOnly = $false }; Remove-Item -Recurse -Force $root }; return }
if ($Do -eq "commit") {
    # A new release on the fake repo's releases branch, for the update.
    Add-Content -Path "$data\if-repo\Data\release.txt" -Value $Subject
    Git-In "$data\if-repo" add -A; Git-In "$data\if-repo" commit -q -m $Subject; return
}
$launcher = "$PSScriptRoot\..\..\launcher"
New-Item -ItemType Directory -Force "$data\if-src\Data", "$data\if-src\Graphics", "$data\roaming\infinitefusion", "$data\drop", "$data\roms\firered", "$data\noexe" | Out-Null
[IO.File]::WriteAllText("$data\if-src\Game.exe", "not a real program")
[IO.File]::WriteAllText("$data\if-src\Data\Scripts.rxdata", "scripts")
[IO.File]::WriteAllText("$data\if-src\Graphics\readme.txt", "no pictures here")
[IO.File]::WriteAllText("$data\noexe\readme.txt", "no program")
[IO.File]::WriteAllText("$data\roaming\infinitefusion\File A.rxdata", "a save")
# The game's download: its files inside one folder, as its own zip has them.
Add-Type -AssemblyName System.IO.Compression.FileSystem
$wrap = "$data\zip-stage\Infinite Fusion"; New-Item -ItemType Directory -Force $wrap | Out-Null
robocopy "$data\if-src" $wrap /E /NFL /NDL /NJH /NJS /NP | Out-Null
foreach ($z in "$data\if.zip", "$data\noexe.zip") { if (Test-Path $z) { Remove-Item $z } }
[IO.Compression.ZipFile]::CreateFromDirectory("$data\zip-stage", "$data\if.zip")
[IO.Compression.ZipFile]::CreateFromDirectory("$data\noexe", "$data\noexe.zip")
Remove-Item -Recurse -Force "$data\zip-stage"
# The GitHub repository, local: one commit on releases.
if (-not (Test-Path "$data\if-repo\.git")) {
    robocopy "$data\if-src" "$data\if-repo" /E /NFL /NDL /NJH /NJS /NP | Out-Null
    Git-In "$data\if-repo" init -q -b releases; Git-In "$data\if-repo" add -A; Git-In "$data\if-repo" commit -q -m "[6.8.2] Safari Zone crash patch"
}
$lines = @()
function Copy-Hashed([string]$src, [string]$to, [string]$what) {
    $before = (Get-FileHash -LiteralPath $src -Algorithm SHA256).Hash
    Copy-Item -LiteralPath $src $to
    $after = (Get-FileHash -LiteralPath $src -Algorithm SHA256).Hash
    $script:lines += "$what`t$before`t$after`t$(if ($before -eq $after) { 'unchanged' } else { 'CHANGED' })"
}
Copy-Hashed (Rom-Path "firered") "$data\roms\firered\firered.gba" "firered rom"
$sav = "$App\engines\fulldex\saves\firered.sav"
Copy-Hashed $sav "$data\drop\Pokemon FireRed.sav" "firered.sav"
[IO.File]::WriteAllText("$data\drop\notes.txt", "not a save")
foreach ($side in "csharp", "rust") {
    $d = "$root\$side"
    New-Item -ItemType Directory -Force "$d\tools\mon-bridge", "$d\engines\fulldex\saves", "$d\state\save-backups\firered", "$d\fonts" | Out-Null
    Copy-Item "$App\tools\mon-bridge\*" "$d\tools\mon-bridge\"
    Copy-Item "$data\drop\Pokemon FireRed.sav" "$d\engines\fulldex\saves\firered.sav"
    Copy-Item "$data\drop\Pokemon FireRed.sav" "$d\state\save-backups\firered\20260101-120000-played.sav"
    # A backup that fails its check (bytes flipped in a copy), not marked, so it can be chosen.
    $b = [IO.File]::ReadAllBytes("$data\drop\Pokemon FireRed.sav"); for ($i = 0; $i -lt $b.Length; $i++) { $b[$i] = $b[$i] -bxor 0x5A }
    [IO.File]::WriteAllBytes("$d\state\save-backups\firered\20260102-120000-played.sav", $b)
}
if ($CSharpExe) { Copy-Item $CSharpExe "$root\csharp\NativeMonsLauncher.exe" -Force }
Copy-Item "$launcher\games.json" "$root\csharp" -Force
Copy-Item "$launcher\fonts\*" "$root\csharp\fonts" -Force
$lines | Set-Content -Encoding utf8 "$root\originals.txt"
$lines

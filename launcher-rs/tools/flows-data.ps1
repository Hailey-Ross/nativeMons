# Fixture data for the flows replays and window tests: fake PKHeX and OpenHome, the download stubs (each file named
# after its URL, as NATIVEMONS_TEST_FETCH reads them), and seeds. flows-data.ps1 [make | clean] [-Out <folder>].
# Prints the fake emulator's and Python's SHA-256 (NATIVEMONS_TEST_SHA256 for those flows). Nothing is fetched.
param([ValidateSet("make", "clean")][string]$Do = "make", [string]$Out = "")
$ErrorActionPreference = "Stop"
. "$PSScriptRoot\work.ps1"
if (-not $Out) { $Out = "$Work\flows-data" }
$d = $Out
if ($Do -eq "clean") { if (Test-Path $d) { Remove-Item -Recurse -Force $d }; return }
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.IO.Compression.FileSystem
New-Item -ItemType Directory -Force "$d\fetch", "$d\PKHeX", "$d\OpenHome", "$d\tmp" | Out-Null
Set-Content "$d\PKHeX\PKHeX.exe" "" -NoNewline; Set-Content "$d\OpenHome\OpenHome.exe" "" -NoNewline
function Name($url) { -join ($url.ToCharArray() | ForEach-Object { if (($_ -match '[A-Za-z0-9]') -or $_ -eq '.' -or $_ -eq '-') { $_ } else { '_' } }) }
function Put($url, [byte[]]$bytes) { [IO.File]::WriteAllBytes("$d\fetch\$(Name $url)", $bytes) }
function PutText($url, $text) { Put $url ([Text.Encoding]::UTF8.GetBytes($text)) }
function Zip($folder, $zip) { if (Test-Path $zip) { Remove-Item $zip }; [IO.Compression.ZipFile]::CreateFromDirectory($folder, $zip) }
function Pic($w, $h, $c1, $c2, $fmt) {
    $b = New-Object Drawing.Bitmap $w, $h
    $g = [Drawing.Graphics]::FromImage($b)
    $br = New-Object Drawing.Drawing2D.LinearGradientBrush (New-Object Drawing.Rectangle 0, 0, $w, $h), ([Drawing.Color]::FromArgb($c1)), ([Drawing.Color]::FromArgb($c2)), 35.0
    $g.FillRectangle($br, 0, 0, $w, $h)
    $g.FillEllipse([Drawing.Brushes]::White, [int]($w * 0.3), [int]($h * 0.3), [int]($w * 0.3), [int]($h * 0.25))
    $g.Dispose()
    $ms = New-Object IO.MemoryStream
    $b.Save($ms, $fmt); $b.Dispose()
    , $ms.ToArray()
}
$png = [Drawing.Imaging.ImageFormat]::Png; $jpg = [Drawing.Imaging.ImageFormat]::Jpeg
# A 3DS emulator release: one folder with azahar.exe (a text file: never started).
$e = "$d\tmp\emu"; New-Item -ItemType Directory -Force "$e\azahar" | Out-Null
Set-Content "$e\azahar\azahar.exe" "fake" -NoNewline; Set-Content "$e\azahar\readme.txt" "a fake emulator for the flows replays" -NoNewline
Zip $e "$d\emu.zip"
# The pinned tag from nm-core (engine sync moves it).
$plusTag = [regex]::Match((Get-Content -Raw "$PSScriptRoot\..\crates\nm-core\src\emu3ds.rs"), 'id: "azaharplus",[^\n]*?tag: Some\("([^"]+)"\)').Groups[1].Value
Copy-Item "$d\emu.zip" "$d\fetch\$(Name "https://github.com/Hailey-Ross/AzaharPlus/releases/download/$plusTag/azaharplus-nativemons-windows-x64.zip")"
# python.org's embeddable package, as far as the install reads it.
$p = "$d\tmp\py"; New-Item -ItemType Directory -Force $p | Out-Null
Set-Content "$p\python.exe" "fake" -NoNewline; Set-Content "$p\python314._pth" "python314.zip`r`n." -NoNewline
Zip $p "$d\python.zip"
Copy-Item "$d\python.zip" "$d\fetch\$(Name 'https://www.python.org/ftp/python/3.14.10/python-3.14.10-embed-amd64.zip')"
# Wallpaper search: two wallhaven pages (one thumbnail missing), openverse, and the pictures.
$hv = 'https://wallhaven.cc/api/v1/search?q=pokemon%20red&purity=100&categories=110&sorting=relevance&page='
PutText ($hv + "1") '{"meta":{"last_page":2},"data":[{"url":"https://wallhaven.cc/w/aaa111","resolution":"1920x1080","thumbs":{"large":"https://th.wallhaven.cc/lg/aa/aaa111.jpg"},"path":"https://w.wallhaven.cc/full/aa/wallhaven-aaa111.png"},{"url":"https://wallhaven.cc/w/bbb222","resolution":"1080x1920","thumbs":{"large":"https://th.wallhaven.cc/lg/bb/bbb222.jpg"},"path":"https://w.wallhaven.cc/full/bb/wallhaven-bbb222.jpg"},{"url":"https://wallhaven.cc/w/ccc333","resolution":"800x600","thumbs":{"large":"https://th.wallhaven.cc/lg/cc/ccc333.jpg"},"path":"https://w.wallhaven.cc/full/cc/wallhaven-ccc333.jpg"}]}'
PutText ($hv + "2") '{"meta":{"last_page":2},"data":[{"url":"https://wallhaven.cc/w/ddd444","resolution":"2560x1440","thumbs":{"large":"https://th.wallhaven.cc/lg/dd/ddd444.jpg"},"path":"https://w.wallhaven.cc/full/dd/wallhaven-ddd444.jpg?x=1"}]}'
Put "https://th.wallhaven.cc/lg/aa/aaa111.jpg" (Pic 300 200 0xFFCC3344 0xFF3344CC $png)
Put "https://th.wallhaven.cc/lg/bb/bbb222.jpg" (Pic 200 300 0xFF22AA55 0xFFEEDD22 $jpg)
Put "https://th.wallhaven.cc/lg/dd/ddd444.jpg" (Pic 320 180 0xFF884422 0xFF2288EE $jpg)
Put "https://w.wallhaven.cc/full/aa/wallhaven-aaa111.png" (Pic 1600 900 0xFFCC3344 0xFF3344CC $png)
Put "https://w.wallhaven.cc/full/dd/wallhaven-ddd444.jpg?x=1" (Pic 1200 1800 0xFF884422 0xFF2288EE $jpg)
$ov = 'https://api.openverse.org/v1/images/?q=pokemon%20red&mature=false&page_size=20&page=1'
PutText $ov '{"page_count":1,"results":[{"title":"Red cap","thumbnail":"https://api.openverse.org/v1/images/x1/thumb/","url":"https://upload.example.org/red-cap.png","foreign_landing_url":"https://example.org/red-cap","creator":"Someone","license":"by-sa","license_version":"4.0"},{"title":"Field","thumbnail":"https://api.openverse.org/v1/images/x2/thumb/","url":"https://upload.example.org/field","foreign_landing_url":"https://example.org/field","creator":null,"license":"cc0","license_version":"1.0"}]}'
Put "https://api.openverse.org/v1/images/x1/thumb/" (Pic 240 240 0xFFAA2222 0xFF222222 $png)
Put "https://upload.example.org/field" (Pic 900 1400 0xFF33AA33 0xFFAAAA33 $jpg)
# Updates: stable has a release whose package lacks the launcher.
$pk = "$d\tmp\pkg"; New-Item -ItemType Directory -Force "$pk\nativemons" | Out-Null; Set-Content "$pk\nativemons\games.json" "{}" -NoNewline
Zip $pk "$d\pkg.zip"
$sha = (Get-FileHash "$d\pkg.zip").Hash.ToLower()
$u = 'https://github.com/Hailey-Ross/nativeMons/releases/download/'
PutText ($u + "stable/release.json") ('{"channel":"stable","build":7,"version":"0.0.9","revision":"0123456789abcdef","package":"nativemons-0.0.9.zip","sha256":"' + $sha + '"}')
Copy-Item "$d\pkg.zip" "$d\fetch\$(Name ($u + 'stable/nativemons-0.0.9.zip'))"
# Sprites: one picture and its shiny.
Put "https://pogo.hails.app/api/pokemon-sprite/25.png" (Pic 96 96 0xFFFFDD00 0xFFFF8800 $png)
Put "https://pogo.hails.app/api/pokemon-sprite/shiny/25.png" (Pic 96 96 0xFFFFAA00 0xFFFF4400 $png)
# Seeds: the patcher present (for PatchPage); Red with a picture of its own (for Remove).
New-Item -ItemType Directory -Force "$d\seed-patch\tools\nds-patcher", "$d\seed-wall\state\wallpapers" | Out-Null
Set-Content "$d\seed-patch\tools\nds-patcher\hails_patcher.py" "# not run" -NoNewline
[IO.File]::WriteAllBytes("$d\seed-wall\state\wallpapers\red.jpg", (Pic 700 1100 0xFF5522AA 0xFF22AAEE $jpg)); Set-Content "$d\seed-wall\state\wallpapers\red.credit.txt" "From a test" -NoNewline
Remove-Item -Recurse -Force "$d\tmp"
"emu $((Get-FileHash "$d\emu.zip").Hash.ToLower())"
"python $((Get-FileHash "$d\python.zip").Hash.ToLower())"

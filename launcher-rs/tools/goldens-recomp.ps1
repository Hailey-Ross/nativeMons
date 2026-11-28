# A scratch golden root for 3DSrecomp+Zakuro: goldens-recomp\{csharp,rust}, each with copies of the app's Zakuro and
# nativemons-recomp (never started here but for `check` on the library), an empty stand-in zig.exe, synthetic X and Y
# dumps (X decrypted and fine, Y still marked encrypted) and, for X, a real library from a recompile (-Library) with
# a stamp and 30 findings, so X is built ("old") and Y isn't. goldens-recomp.ps1 make [-Library <dll>] | reset | clean.
# reset puts back what a dry run changes (stamps, header fix, asked lists). Run run-replay.ps1 -Root <this root>.
param([ValidateSet("make", "reset", "clean")][string]$Do = "make", [string]$Library = "")
$ErrorActionPreference = "Stop"
. "$PSScriptRoot\work.ps1"
$root = "$Work\goldens-recomp"
if ($Do -eq "clean") { if (Test-Path $root) { Remove-Item -Recurse -Force $root }; return }
if (Test-Path "$root\csharp-replay") { Remove-Item -Recurse -Force "$root\csharp-replay" }
$app = "$App"; $base = "$Work\goldens\csharp"; $in = "$root\inputs"
function New-Dump([string]$file, [uint64]$title, [bool]$fine) {
    $b = New-Object byte[] 0x1000
    [Text.Encoding]::ASCII.GetBytes("NCSD").CopyTo($b, 0x100); [BitConverter]::GetBytes($title).CopyTo($b, 0x108)
    [BitConverter]::GetBytes([uint32]1).CopyTo($b, 0x120); [BitConverter]::GetBytes([uint32]0x10).CopyTo($b, 0x124)
    [Text.Encoding]::ASCII.GetBytes("NCCH").CopyTo($b, 0x300); $b[0x38B] = 1; $b[0x38F] = $(if ($fine) { 0x04 } else { 0x21 })
    [BitConverter]::GetBytes([uint32]2).CopyTo($b, 0x3A0); [BitConverter]::GetBytes([uint32]1).CopyTo($b, 0x3A4)
    [BitConverter]::GetBytes([uint32]4).CopyTo($b, 0x3B0); [BitConverter]::GetBytes([uint32]1).CopyTo($b, 0x3B4)
    [Text.Encoding]::ASCII.GetBytes(".code").CopyTo($b, 0x600); [Text.Encoding]::ASCII.GetBytes("IVFC").CopyTo($b, 0xA00)
    [IO.File]::WriteAllBytes($file, $b)
    # A fixed time, so the dump's key (size and time) is the same after every reset.
    (Get-Item -LiteralPath $file).LastWriteTimeUtc = [DateTime]::new(2026, 1, 1, 0, 0, 0, [DateTimeKind]::Utc)
}
if ($Do -eq "make") {
    foreach ($side in "csharp", "rust") {
        $d = "$root\$side"
        New-Item -ItemType Directory -Force "$d\engines\zakuro", "$d\state" | Out-Null
        foreach ($f in "zakuro.exe", "nativemons-recomp.exe", "NATIVEMONS-BUILD.txt") { Copy-Item "$app\engines\zakuro\$f" "$d\engines\zakuro\" -Force }
        robocopy "$app\engines\zakuro\hints" "$d\engines\zakuro\hints" /E /NFL /NDL /NJH /NJS /NP | Out-Null
    }
    Copy-Item "$base\NativeMonsLauncher.exe", "$base\games.json" "$root\csharp" -Force
    robocopy "$base\fonts" "$root\csharp\fonts" /E /NFL /NDL /NJH /NJS /NP | Out-Null
    New-Item -ItemType Directory -Force "$root\csharp\out", $in | Out-Null
    if ($Library -ne "") { Copy-Item -LiteralPath $Library "$in\x.dll" -Force }
}
# Each side's state: its own dumps (the C# replay plays csharp-replay, a copy of the csharp side, so that side's list
# names the copy's), Zig's stand-in, X's library, stamp and findings; nothing a run left behind.
$key = "ctr-0004000000055d00-1000-" + [DateTime]::new(2026, 1, 1, 0, 0, 0, [DateTimeKind]::Utc).Ticks.ToString("x")
foreach ($side in "csharp", "rust") {
    $roms = "$root\$side\roms"; $played = if ($side -eq "csharp") { "$root\csharp-replay\roms" } else { $roms }
    New-Item -ItemType Directory -Force $roms | Out-Null
    New-Dump "$roms\x.3ds" 0x0004000000055D00 $true; New-Dump "$roms\y.3ds" 0x0004000000055E00 $false
    $st = "$root\$side\state"
    foreach ($x in "3ds", "recomp-asked.txt", "patch-asked.txt", "quick-start") { if (Test-Path "$st\$x") { Remove-Item -Recurse -Force "$st\$x" } }
    $r = "$st\3ds\recomp"; $z = "$st\3ds\zig-0.17.0\zig-x86_64-windows-0.17.0"
    New-Item -ItemType Directory -Force $r, $z | Out-Null
    New-Item -ItemType File "$z\zig.exe" | Out-Null
    [IO.File]::WriteAllText("$st\rom-files.txt", "$played\x.3ds`r`n$played\y.3ds`r`n")
    if (Test-Path "$in\x.dll") {
        Copy-Item "$in\x.dll" "$r\0004000000055D00.dll"
        [IO.File]::WriteAllText("$r\0004000000055D00.txt", "$key`r`n0`r`n`r`n")
        [IO.File]::WriteAllText("$r\0004000000055D00.hints", "# findings`n" + ((1..30 | ForEach-Object { "{0:X8}" -f (0x00100000 + $_ * 4) }) -join "`n") + "`n")
    }
}
"goldens-recomp $Do done (library: $(Test-Path "$in\x.dll"))"

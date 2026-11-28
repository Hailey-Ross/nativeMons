# Step 2 parity: runs each windowless CLI mode with the C# exe and the Rust exe on identical copies, then compares
# result files, written saves and extracted trees byte for byte. Originals are only read (hashed before and after).
# Output under <work>\parity (tools\work.ps1); -Clean deletes the copies afterwards.
# -Target: the exe for the "Rust" lane (default the Rust build; the C# exe there checks C# against itself).
param([string[]]$Modes = @("platinum", "nds", "roms"), [switch]$Clean, [string]$Target = "")
$ErrorActionPreference = "Stop"
. "$PSScriptRoot\work.ps1"
$Modes = @($Modes | ForEach-Object { $_ -split ',' })   # -File passes "a,b" as one string
$root = "$Work\parity"
if (-not $Target) { $Target = "$TargetDir\debug\nativemons.exe" }
$target = $Target
$csSource = "$Work\goldens\csharp"
$originals = @{
    platinumSave = "$TestData\projectpokemon\platinum.sav"
    platinumRom  = (Rom-Path "platinumRom")
    red          = (Rom-Path "red")
    yellow       = (Rom-Path "yellow")
    crystal      = (Rom-Path "crystal")
    crystal2     = (Rom-Path "crystal2")
    firered      = (Rom-Path "firered")
    emerald      = (Rom-Path "emerald")
    redChapter   = (Rom-Path "redChapter")
    imperium     = (Rom-Path "imperium")
    other        = (Rom-Path "other")
}
foreach ($v in $originals.Values) { if (-not (Test-Path -LiteralPath $v)) { throw "missing original $v" } }
$hashBefore = @{}; foreach ($k in $originals.Keys) { $hashBefore[$k] = (Get-FileHash -LiteralPath $originals[$k] -Algorithm SHA256).Hash }

# Two launcher folders with the same games.json, an empty state each and a scratch APPDATA each.
$cs = "$root\csharp"; $rs = "$root\rust"; $in = "$root\inputs"
New-Item -ItemType Directory -Force $cs, $rs, $in | Out-Null
Copy-Item "$csSource\NativeMonsLauncher.exe", "$csSource\games.json" $cs -Force
Copy-Item $target "$rs\nativemons.exe" -Force; Copy-Item "$csSource\games.json" $rs -Force
$exe = @{ cs = "$cs\NativeMonsLauncher.exe"; rs = "$rs\nativemons.exe" }

function Invoke-Side([string]$side, [string[]]$argv) {
    $env:NATIVEMONS_STATE = "$root\state-$side"; $env:APPDATA = "$root\appdata-$side"
    New-Item -ItemType Directory -Force $env:NATIVEMONS_STATE, $env:APPDATA | Out-Null
    $p = Start-Process -FilePath $exe[$side] -ArgumentList ($argv | ForEach-Object { '"' + $_ + '"' }) -Wait -PassThru -NoNewWindow
    return $p.ExitCode
}
function Compare-Files([string]$a, [string]$b) {
    if (-not (Test-Path -LiteralPath $a) -and -not (Test-Path -LiteralPath $b)) { return "both missing" }
    if (-not (Test-Path -LiteralPath $a)) { return "C# missing" }
    if (-not (Test-Path -LiteralPath $b)) { return "Rust missing" }
    $ha = (Get-FileHash -LiteralPath $a -Algorithm SHA256).Hash; $hb = (Get-FileHash -LiteralPath $b -Algorithm SHA256).Hash
    if ($ha -eq $hb) { return "identical" }
    return "differs (C# $((Get-Item -LiteralPath $a).Length) B, Rust $((Get-Item -LiteralPath $b).Length) B)"
}
function Compare-Trees([string]$a, [string]$b) {
    $fa = @{}; Get-ChildItem -LiteralPath $a -Recurse -File | ForEach-Object { $fa[$_.FullName.Substring($a.Length)] = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash }
    $fb = @{}; Get-ChildItem -LiteralPath $b -Recurse -File | ForEach-Object { $fb[$_.FullName.Substring($b.Length)] = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash }
    $bad = @($fa.Keys + $fb.Keys | Sort-Object -Unique | Where-Object { $fa[$_] -ne $fb[$_] })
    if ($bad.Count -eq 0) { return "identical ($($fa.Count) files)" }
    return "differs in $($bad.Count) of $($fa.Count) files: $($bad | Select-Object -First 5)"
}
$results = New-Object System.Collections.Generic.List[string]
function Add-Result([string]$case, [int]$ecs, [int]$ers, [string]$what) { $results.Add(("{0,-38} exit C# {1} Rust {2}  {3}" -f $case, $ecs, $ers, $what)) }

if ($Modes -contains "platinum") {
    Copy-Item -LiteralPath $originals.platinumSave "$in\platinum.sav" -Force
    Set-Content -LiteralPath "$in\garbage.sav" -Value "not a save" -Encoding ascii
    foreach ($side in "cs", "rs") {
        $o = "$root\out-$side"; New-Item -ItemType Directory -Force $o | Out-Null
        $e1 = Invoke-Side $side @("--platinum-convert", "to-port", "$in\platinum.sav", "$o\port.bin")
        $e2 = Invoke-Side $side @("--platinum-convert", "to-ds", "$o\port.bin", "$o\back.sav")
        $e3 = Invoke-Side $side @("--platinum-convert", "to-ds", "$in\garbage.sav", "$o\bad.sav")
        $e4 = Invoke-Side $side @("--platinum-convert", "to-ds", "$in\platinum.sav", "$o\notport.sav")
        Set-Variable -Name "pc_$side" -Value @($e1, $e2, $e3, $e4)
    }
    Add-Result "platinum-convert to-port" $pc_cs[0] $pc_rs[0] (Compare-Files "$root\out-cs\port.bin" "$root\out-rs\port.bin")
    Add-Result "platinum-convert to-ds (round trip)" $pc_cs[1] $pc_rs[1] (Compare-Files "$root\out-cs\back.sav" "$root\out-rs\back.sav")
    Add-Result "platinum-convert to-ds garbage (error)" $pc_cs[2] $pc_rs[2] (Compare-Files "$root\out-cs\bad.sav.error.txt" "$root\out-rs\bad.sav.error.txt")
    Add-Result "platinum-convert to-ds of a DS save" $pc_cs[3] $pc_rs[3] ((Compare-Files "$root\out-cs\notport.sav" "$root\out-rs\notport.sav") + " / error " + (Compare-Files "$root\out-cs\notport.sav.error.txt" "$root\out-rs\notport.sav.error.txt"))
    $rt = (Get-FileHash "$root\out-rs\back.sav").Hash -eq (Get-FileHash "$in\platinum.sav").Hash
    $results.Add("  round trip equals the input save: $rt")
}
if ($Modes -contains "nds") {
    Copy-Item -LiteralPath $originals.platinumRom "$in\platinum.nds" -Force
    foreach ($side in "cs", "rs") { Set-Variable -Name "nd_$side" -Value (Invoke-Side $side @("--extract-nds", "$in\platinum.nds", "$root\nds-$side")) }
    Add-Result "extract-nds Platinum" $nd_cs $nd_rs (Compare-Trees "$root\nds-cs" "$root\nds-rs")
}

if ($Modes -contains "roms") {
    # A ROM folder of copies (with a subfolder, a romhack and a non-Pokemon game) plus one ROM added on its own.
    $roms = "$in\roms"; New-Item -ItemType Directory -Force "$roms\gb", "$roms\gba", "$roms\nds", "$in\single" | Out-Null
    foreach ($k in "red", "yellow", "crystal", "other") { Copy-Item -LiteralPath $originals[$k] "$roms\gb\" -Force }
    foreach ($k in "firered", "emerald", "redChapter", "imperium") { Copy-Item -LiteralPath $originals[$k] "$roms\gba\" -Force }
    if (-not (Test-Path "$in\platinum.nds")) { Copy-Item -LiteralPath $originals.platinumRom "$in\platinum.nds" -Force }
    Copy-Item "$in\platinum.nds" "$roms\nds\Pokemon - Platinum Version (USA).nds" -Force
    Copy-Item -LiteralPath $originals.crystal2 "$in\single\" -Force
    foreach ($side in "cs", "rs") {
        $st = "$root\state-$side"; New-Item -ItemType Directory -Force $st | Out-Null
        Set-Content -LiteralPath "$st\rom-folders.txt" -Value $roms -Encoding utf8
        Set-Content -LiteralPath "$st\rom-files.txt" -Value "$in\single\Pokemon Crystal.gbc" -Encoding utf8
        $e1 = Invoke-Side $side @("--roms", "$root\roms1-$side.txt")
        $e2 = Invoke-Side $side @("--roms", "$root\roms2-$side.txt")
        Set-Variable -Name "rm_$side" -Value @($e1, $e2)
    }
    Add-Result "roms (first scan)" $rm_cs[0] $rm_rs[0] (Compare-Files "$root\roms1-cs.txt" "$root\roms1-rs.txt")
    Add-Result "roms (second scan, hash cache)" $rm_cs[1] $rm_rs[1] (Compare-Files "$root\roms2-cs.txt" "$root\roms2-rs.txt")
    Add-Result "roms: state rom-hashes.txt" 0 0 (Compare-Files "$root\state-cs\rom-hashes.txt" "$root\state-rs\rom-hashes.txt")
    $chosen = @(Get-ChildItem "$root\state-cs" -Filter "rom-*.txt" | Where-Object { $_.Name -notin "rom-folders.txt", "rom-files.txt", "rom-hashes.txt" })
    foreach ($c in $chosen) { Add-Result "roms: state $($c.Name)" 0 0 (Compare-Files $c.FullName "$root\state-rs\$($c.Name)") }
    $extra = @(Get-ChildItem "$root\state-rs" -Filter "rom-*.txt" | Where-Object { -not (Test-Path "$root\state-cs\$($_.Name)") })
    foreach ($c in $extra) { Add-Result "roms: state $($c.Name)" 0 0 "only Rust wrote it" }
}

if ($Modes -contains "saves") {
    # Each side runs the same steps in the same folder ($root\lane), which is then renamed, so paths match exactly.
    # Engines: copies of the app's LOVE, Gen1Recomp and mon-bridge (run headless); Fulldex is an empty stand-in file.
    $app = "$App"; $fx = "$root\fixture"
    if (-not (Test-Path "$fx\love\lovec.exe")) {
        New-Item -ItemType Directory -Force "$fx\engines", "$fx\tools" | Out-Null
        Copy-Item -Recurse -Force "$app\love" "$fx\love"; Copy-Item -Recurse -Force "$app\engines\gen1recomp" "$fx\engines\gen1recomp"
        Copy-Item -Recurse -Force "$app\tools\mon-bridge" "$fx\tools\mon-bridge"
    }
    $sv = @{ frSave = "$TestData\billspc\firered.sav"; frSave2 = "$TestData\more\firered-thegamershollow.sav"
             redSave = "$TestData\billspc\red.sav"; redFooter = "$TestData\more\red-mplewis.sav" }
    foreach ($k in $sv.Keys) { $originals[$k] = $sv[$k]; $hashBefore[$k] = (Get-FileHash -LiteralPath $sv[$k] -Algorithm SHA256).Hash }
    $in = "$root\inputs"; New-Item -ItemType Directory -Force "$in\roms" | Out-Null
    Copy-Item -LiteralPath $originals.firered "$in\roms\" -Force; Copy-Item -LiteralPath $originals.red "$in\roms\" -Force
    foreach ($k in $sv.Keys) { Copy-Item -LiteralPath $sv[$k] "$in\$k.sav" -Force }
    $bytes = [IO.File]::ReadAllBytes("$in\frSave.sav"); for ($i = 0x1000; $i -lt 0x1040; $i++) { $bytes[$i] = $bytes[$i] -bxor 0x5A }
    [IO.File]::WriteAllBytes("$in\frDamaged.sav", $bytes)
    & "$fx\tools\mon-bridge\mon-bridge.exe" repair "$in\frDamaged.sav" "$in\frRepaired.sav" | Out-Null
    Copy-Item "$in\frSave.sav" "$in\frSave3.sav" -Force
    $held = @(& "$fx\tools\mon-bridge\mon-bridge.exe" list "$in\frSave3.sav" | ForEach-Object { $f = $_ -split "`t"; if ($f.Count -ge 8 -and $f[0] -match '^\d+$') { [int]$f[0] * 30 + [int]$f[1] } })
    $free = (0..419 | Where-Object { $held -notcontains $_ } | Select-Object -Last 1)
    $moved = & "$fx\tools\mon-bridge\mon-bridge.exe" relocate "$in\frSave3.sav" ([int][Math]::Floor($held[0] / 30)) ($held[0] % 30) ([int][Math]::Floor($free / 30)) ($free % 30) "$in\relocate-backups"
    if ((Get-FileHash "$in\frSave3.sav").Hash -eq (Get-FileHash "$in\frSave.sav").Hash) { throw "the third FireRed save didn't change: $moved" }
    # A 3DS game: a synthetic dump for X in the ROM folder, and Y's save as its save (X and Y share the format).
    Copy-Item -LiteralPath "$TestData\projectpokemon\y.main" "$in\xy.main" -Force
    $originals["yMain"] = "$TestData\projectpokemon\y.main"; $hashBefore["yMain"] = (Get-FileHash -LiteralPath $originals.yMain).Hash
    Set-Content -LiteralPath "$in\garbage.main" -Value "not a 3DS save at all" -Encoding ascii
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    function New-Zip([string]$from, [string]$zip) { if (Test-Path $zip) { Remove-Item $zip }; [IO.Compression.ZipFile]::CreateFromDirectory($from, $zip) }
    New-Item -ItemType Directory -Force "$in\pcsrc\MyGame\data", "$in\pcfolder\sub" | Out-Null
    Copy-Item "$fx\tools\mon-bridge\mon-bridge.exe" "$in\pcsrc\MyGame\Game.exe" -Force; Set-Content -LiteralPath "$in\pcsrc\MyGame\data\readme.txt" -Value "data" -Encoding ascii
    New-Zip "$in\pcsrc" "$in\pcgame.zip"
    Copy-Item "$fx\tools\mon-bridge\mon-bridge.exe" "$in\pcfolder\Play Me.exe" -Force; Copy-Item "$fx\love\love.exe" "$in\pcfolder\sub\other.exe" -Force
    Copy-Item "$fx\love\love.exe" "$in\pcfolder\unins000.exe" -Force
    Set-Content -LiteralPath "$in\hints-empty.txt" -Value @("# nothing", "zz", "0x10") -Encoding ascii
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    function New-Zip([string]$from, [string]$zip) { if (Test-Path $zip) { Remove-Item $zip }; [IO.Compression.ZipFile]::CreateFromDirectory($from, $zip) }
    foreach ($d in "modsrc-bare", "modsrc-folder\testmod", "modsrc-none") { New-Item -ItemType Directory -Force "$in\$d" | Out-Null }
    Set-Content -LiteralPath "$in\modsrc-bare\manifest.json" -Value '{ "id": "baremod", "name": "Bare mod", "version": "1.0.0" }' -Encoding ascii
    Set-Content -LiteralPath "$in\modsrc-bare\main.lua" -Value 'return {}' -Encoding ascii
    Set-Content -LiteralPath "$in\modsrc-folder\testmod\manifest.json" -Value '{ "id": "testmod", "name": "Test mod", "version": "1.0.0", "conflicts": ["baremod"] }' -Encoding ascii
    Set-Content -LiteralPath "$in\modsrc-folder\testmod\main.lua" -Value 'return {}' -Encoding ascii
    Set-Content -LiteralPath "$in\modsrc-none\readme.txt" -Value 'no manifest' -Encoding ascii
    New-Zip "$in\modsrc-bare" "$in\mod-bare.zip"; New-Zip "$in\modsrc-folder" "$in\mod-folder.zip"; New-Zip "$in\modsrc-none" "$in\mod-none.zip"
    # Synthetic 3DS dumps (header, NCCH, ExeFS name, RomFS magic): decrypted but marked encrypted, still encrypted, fine.
    foreach ($v in "plain", "enc", "fine") {
        $b = New-Object byte[] 0x1000
        [Text.Encoding]::ASCII.GetBytes("NCSD").CopyTo($b, 0x100); [BitConverter]::GetBytes([uint64]0x0004000000055D00).CopyTo($b, 0x108)
        [BitConverter]::GetBytes([uint32]1).CopyTo($b, 0x120); [BitConverter]::GetBytes([uint32]0x10).CopyTo($b, 0x124)
        [Text.Encoding]::ASCII.GetBytes("NCCH").CopyTo($b, 0x300); $b[0x38B] = 1; $b[0x38F] = $(if ($v -eq "fine") { 0x04 } else { 0x21 })
        [BitConverter]::GetBytes([uint32]2).CopyTo($b, 0x3A0); [BitConverter]::GetBytes([uint32]1).CopyTo($b, 0x3A4)
        [BitConverter]::GetBytes([uint32]4).CopyTo($b, 0x3B0); [BitConverter]::GetBytes([uint32]1).CopyTo($b, 0x3B4)
        [Text.Encoding]::ASCII.GetBytes(".code").CopyTo($b, 0x600); [Text.Encoding]::ASCII.GetBytes($(if ($v -eq "enc") { "Q7#z" } else { "IVFC" })).CopyTo($b, 0xA00)
        [IO.File]::WriteAllBytes("$in\ctr-$v.3ds", $b)
    }
    New-Item -ItemType Directory -Force "$in\addfolder", "$in\zipsrc" | Out-Null
    Copy-Item -LiteralPath $originals.yellow "$in\addfolder\" -Force; Copy-Item -LiteralPath $originals.crystal "$in\zipsrc\" -Force
    Set-Content -LiteralPath "$in\zipsrc\notes.txt" -Value "not a rom" -Encoding ascii
    New-Zip "$in\zipsrc" "$in\roms-zip.zip"; Copy-Item -LiteralPath $originals.other "$in\other.gb" -Force
    # A fake release on disk: what --update downloads when NATIVEMONS_UPDATE_URL points at it.
    $pkg = "$root\fakeupd-src\nativeMons"
    foreach ($d in "engines\gen1recomp\src", "engines\fulldex\saves", "engines\nativeplat", "tools\mon-bridge", "state") { New-Item -ItemType Directory -Force "$pkg\$d" | Out-Null }
    foreach ($f in "NativeMonsLauncher.exe", "engines\gen1recomp\main.lua", "engines\gen1recomp\src\a.lua", "engines\fulldex\Pokemon Fulldex.exe", "engines\fulldex\saves\kept.sav", "engines\nativeplat\nativeplat.exe", "tools\mon-bridge\mon-bridge.exe", "state\kept.txt") { Set-Content -LiteralPath "$pkg\$f" -Value "stub $f" -Encoding ascii }
    Copy-Item "$csSource\games.json" "$pkg\games.json"
    New-Item -ItemType Directory -Force "$root\fakeupd\bleeding-edge" | Out-Null
    New-Zip "$root\fakeupd-src" "$root\fakeupd\bleeding-edge\pkg.zip"
    $sha = (Get-FileHash "$root\fakeupd\bleeding-edge\pkg.zip" -Algorithm SHA256).Hash.ToLowerInvariant()
    Set-Content -LiteralPath "$root\fakeupd\bleeding-edge\release.json" -Encoding ascii -Value ('{"channel":"bleeding-edge","revision":"abc123","version":"0.0.9","package":"pkg.zip","sha256":"' + $sha + '","build":999}')
    $lane = "$root\lane"; $master = "$lane\fulldex\saves\firered.sav"
    # Steps: name, then the arguments; "!" steps run PowerShell first (file moves the same on both sides).
    $steps = @(
        @("roms0", "--roms", "<out>"),
        @("prep-fr", "--saves", "firered", "<out>", "prepare"), @("prep-red", "--saves", "red", "<out>", "prepare"),
        @("roms1", "--roms", "<out>"),
        @("sync-none", "--sync", "firered", "<out>"),
        @("!", { Copy-Item "$in\frSave.sav" $master -Force }),
        @("sync-fulldex", "--sync", "firered", "<out>"), @("sync-level", "--sync", "firered", "<out>"),
        @("list1", "--saves", "firered", "<out>", "list"), @("new", "--saves", "firered", "<out>", "new", "Second"),
        @("!", { Copy-Item "$in\frSave2.sav" $master -Force }),
        @("sync-new", "--sync", "firered", "<out>"), @("use1", "--saves", "firered", "<out>", "use", "1"),
        @("copy1", "--saves", "firered", "<out>", "copy", "1"), @("caught", "--saves", "firered", "<out>", "caught"),
        @("delete2", "--saves", "firered", "<out>", "delete", "2"), @("delete-active", "--saves", "firered", "<out>", "delete", "1"),
        # Gen1Recomp changed: its slot gets the other save, so only its side moved.
        @("import-fr", "--saves", "firered", "<out>", "import", "$in\frSave2.sav"), @("sync-gen1", "--sync", "firered", "<out>"),
        # Both changed (a repaired copy is a third save): a conflict, then Fulldex's wins.
        @("!", { Copy-Item "$in\frSave.sav" $master -Force }),
        @("import-fr2", "--saves", "firered", "<out>", "import", "$in\frSave3.sav"),
        @("sync-conflict", "--sync", "firered", "<out>"), @("sync-prefer", "--sync", "firered", "<out>", "fulldex"),
        @("!", { Copy-Item "$in\frDamaged.sav" $master -Force }),
        @("sync-damaged", "--sync", "firered", "<out>"),
        # Gen1Recomp's import doesn't always change what it exports, so the record is moved instead (same on both sides).
        @("!", { $f = "$lane\state\save-sync\firered.txt"; [IO.File]::WriteAllText($f, ([IO.File]::ReadAllText($f) -replace 'gen1=[0-9A-F]*', 'gen1=00')) }),
        @("sync-gen1-rec", "--sync", "firered", "<out>"),
        @("!", { $f = "$lane\state\save-sync\firered.txt"; [IO.File]::WriteAllText($f, (([IO.File]::ReadAllText($f) -replace 'gen1=[0-9A-F]*', 'gen1=00') -replace 'master=[0-9A-F]*', 'master=00')); Copy-Item "$in\frSave2.sav" $master -Force }),
        @("sync-conflict-rec", "--sync", "firered", "<out>"), @("sync-prefer-gen1", "--sync", "firered", "<out>", "gen1"),
        @("export-plain", "--export", "firered", "plain", "$lane\out\plain"), @("export-gba-cp", "--export", "firered", "checkpoint", "$lane\out\cp"),
        @("!", { New-Item -ItemType Directory -Force "$lane\state\save-sets\firered" | Out-Null; Set-Content -LiteralPath "$lane\state\save-sets\firered\switching.txt" -Value "1" -NoNewline }),
        @("recover", "--saves", "firered", "<out>", "recover"),
        @("import-red", "--saves", "red", "<out>", "import", "$in\redSave.sav"),
        @("import-footer", "--saves", "red", "<out>", "import", "$in\redFooter.sav"),
        @("import-trim", "--saves", "red", "<out>", "import-trim", "$in\redFooter.sav"),
        @("export-red-cp", "--export", "red", "checkpoint", "$lane\out\redcp"),
        @("!", { if (-not (Test-Path "$in\adopt.lua")) { Copy-Item (Get-ChildItem "$lane\state\gen1recomp" -Recurse -Filter "slot*.lua" | Where-Object { $_.DirectoryName -like "*\saves\red" } | Select-Object -First 1).FullName "$in\adopt.lua" } }),
        @("adopt", "--saves", "red", "<out>", "adopt", "$in\adopt.lua", "Adopted"),
        @("caught-red", "--saves", "red", "<out>", "caught"), @("list-red", "--saves", "red", "<out>", "list"),
        @("sync-red", "--sync", "red", "<out>"), @("export-none", "--export", "nosuch", "plain", "$lane\out\none"),
        @("mod-clashes0", "--mod-clashes", "firered", "<out>"), @("mod-deps0", "--mod-deps", "firered", "<out>"), @("mod-nosuch", "--mod-clashes", "nosuch", "<out>"),
        @("mod-bare", "--install-mod", "$in\mod-bare.zip", "<out>"), @("mod-folder", "--install-mod", "$in\mod-folder.zip", "<out>"),
        @("mod-again", "--install-mod", "$in\mod-folder.zip", "<out>"), @("mod-wrong", "--install-mod", "$in\mod-folder.zip", "<out>", "othermod"),
        @("mod-none", "--install-mod", "$in\mod-none.zip", "<out>"), @("mod-missing", "--install-mod", "$in\nothing.zip", "<out>"),
        @("mod-clashes1", "--mod-clashes", "firered", "<out>"), @("mod-deps1", "--mod-deps", "firered", "<out>"),
        # The community mod index (read only): one not in it, one for other games, and the smallest that fits FireRed.
        @("index-none", "--install-index-mod", "firered", "no-such-mod-here", "<out>"),
        @("!", { $ix = Get-Content -Raw -Encoding UTF8 "$lane\state\mod-index.json" | ConvertFrom-Json; $fit = { param($m, $v) (-not $m.games) -or @($m.games | Where-Object { $_ -eq $v -or $_ -eq 'gen3' }).Count }
                 $script:modOther = @($ix.mods | Where-Object { $_.games -and -not (& $fit $_ 'firered') -and $_.latest.zip.url -like 'https://*' })[0].id
                 $script:modFit = @($ix.mods | Where-Object { (& $fit $_ 'firered') -and $_.latest.zip.url -like 'https://*' -and $_.latest.zip.size -gt 0 } | Sort-Object { [long]$_.latest.zip.size })[0].id }),
        @("index-other", "--install-index-mod", "firered", "<mod-other>", "<out>"), @("index-fit", "--install-index-mod", "firered", "<mod-fit>", "<out>"),
        @("mod-clashes2", "--mod-clashes", "firered", "<out>"), @("mod-deps2", "--mod-deps", "firered", "<out>"),
        @("!", { foreach ($v in "plain", "enc", "fine") { Copy-Item "$in\ctr-$v.3ds" "$lane\out\ctr-$v.3ds" -Force } }),
        @("hdr-check", "--3ds-header", "<out>", "$lane\out\ctr-plain.3ds", "check"), @("hdr-fix", "--3ds-header", "<out>", "$lane\out\ctr-plain.3ds", "fix"),
        @("hdr-check2", "--3ds-header", "<out>", "$lane\out\ctr-plain.3ds", "check"), @("hdr-fix2", "--3ds-header", "<out>", "$lane\out\ctr-plain.3ds", "fix"),
        @("hdr-undo", "--3ds-header", "<out>", "$lane\out\ctr-plain.3ds", "undo"), @("hdr-undo2", "--3ds-header", "<out>", "$lane\out\ctr-plain.3ds", "undo"),
        @("hdr-enc", "--3ds-header", "<out>", "$lane\out\ctr-enc.3ds", "check"), @("hdr-enc-fix", "--3ds-header", "<out>", "$lane\out\ctr-enc.3ds", "fix"),
        @("hdr-fine", "--3ds-header", "<out>", "$lane\out\ctr-fine.3ds", "check"), @("hdr-notctr", "--3ds-header", "<out>", "$in\frSave.sav", "check"),
        @("add-roms", "--add-roms", "<out>", "$in\addfolder", "$in\roms-zip.zip", "$in\other.gb", "$in\roms\$(Split-Path -Leaf $originals.red)", "$in\nowhere.gb"),
        @("roms2", "--roms", "<out>"),
        @("!", { Copy-Item "$in\ctr-plain.3ds" "$lane\out\x-dump.3ds" -Force; Add-Content -LiteralPath "$lane\state\rom-files.txt" -Value "$lane\out\x-dump.3ds"; Set-Content -LiteralPath "$lane\state\3ds-games.txt" -Value "on" -NoNewline }),
        @("roms3ds", "--roms", "<out>"), @("x-list", "--saves", "x", "<out>", "list"),
        @("cfg-azahar", "--3ds-config", "<out>", "azahar", "x"), @("cfg-plus", "--3ds-config", "<out>", "azaharplus", "x"), @("cfg-bad", "--3ds-config", "<out>", "azahar", "red"),
        @("!", { New-Item -ItemType Directory -Force "$lane\state\3ds\saves\x\data" | Out-Null; Copy-Item "$in\xy.main" "$lane\state\3ds\saves\x\data\main" -Force; Set-Content -LiteralPath "$lane\state\game-options.txt" -Value "vol=55" }),
        @("cfg-vol", "--3ds-config", "<out>", "azahar", "x"),
        @("emu-remove-none", "--remove-emu", "<out>", "azahar"), @("emu-unknown", "--install-emu", "<out>", "nosuch"),
        @("emu-install", "--install-emu", "<out>", "azahar"),
        @("x-prepare", "--3ds-save", "<out>", "x", "azahar", "prepare"),
        @("!", { $s3 = "$lane\state\3ds\emu\azahar\sdmc\Nintendo 3DS\00000000000000000000000000000000\00000000000000000000000000000000\title\00040000\00055d00\data\00000001"; if (Test-Path "$s3\main") { $b = [IO.File]::ReadAllBytes("$s3\main"); [IO.File]::WriteAllBytes("$s3\main", $b) } }),
        @("x-collect", "--3ds-save", "<out>", "x", "azahar", "collect"),
        @("x-prepare2", "--3ds-save", "<out>", "x", "azahar", "prepare"),
        @("!", { $s3 = "$lane\state\3ds\emu\azahar\sdmc\Nintendo 3DS\00000000000000000000000000000000\00000000000000000000000000000000\title\00040000\00055d00\data\00000001"; Copy-Item "$in\garbage.main" "$s3\main" -Force }),
        @("x-collect-bad", "--3ds-save", "<out>", "x", "azahar", "collect"),
        @("x-zakuro", "--3ds-save", "<out>", "x", "zakuro", "prepare"), @("x-zakuro-collect", "--3ds-save", "<out>", "x", "zakuro", "collect"),
        @("x-list2", "--saves", "x", "<out>", "list"), @("x-new", "--saves", "x", "<out>", "new", "Second"), @("x-use1", "--saves", "x", "<out>", "use", "1"),
        @("emu-remove", "--remove-emu", "<out>", "azahar"),
        @("hints", "--recomp-hints", "<out>"),
        @("pc-zip", "--add-pc", "<out>", "zip", "$in\pcgame.zip"), @("pc-zip-again", "--add-pc", "<out>", "zip", "$in\pcgame.zip"),
        @("pc-choose", "--add-pc", "<out>", "folder", "$in\pcfolder"), @("pc-pick", "--add-pc", "<out>", "folder", "$in\pcfolder", "1"),
        @("pc-none", "--add-pc", "<out>", "folder", "$in\zipsrc"), @("pc-missing", "--add-pc", "<out>", "zip", "$in\nothing.zip"),
        @("pc-noexe-zip", "--add-pc", "<out>", "zip", "$in\mod-none.zip"), @("roms-pc", "--roms", "<out>"),
        @("submit-empty", "--submit-hints", "<out>", "0004000000055D00-0123456789ABCDEF", "$in\hints-empty.txt"),
        @("submit-missing", "--submit-hints", "<out>", "0004000000055D00-0123456789ABCDEF", "$in\no-hints.txt"),
        # The updater last (it replaces the launcher's files): GitHub read only with local=, then a local fake release.
        @("!", { Set-Content -LiteralPath "$lane\launcher\version.txt" -Value "local=parity" -Encoding ascii }),
        @("update-check", "--update", "<out>"),
        @("!", { Remove-Item -LiteralPath "$lane\launcher\version.txt"; $env:NATIVEMONS_UPDATE_URL = "file:///" + ($root -replace '\\', '/') + "/fakeupd/" }),
        @("update-apply", "--update", "<out>"),
        @("!", { Remove-Item Env:\NATIVEMONS_UPDATE_URL })
    )
    $exit = @{}
    foreach ($side in "cs", "rs") {
        if (Test-Path $lane) { Remove-Item -Recurse -Force -Confirm:$false $lane }
        New-Item -ItemType Directory -Force "$lane\launcher", "$lane\state", "$lane\appdata\LOVE\nativemons\saves", "$lane\fulldex\saves", "$lane\results", "$lane\out" | Out-Null
        Set-Content -LiteralPath "$lane\appdata\LOVE\nativemons\saves\old-note.txt" -Value "from an older install" -Encoding ascii
        New-Item -ItemType File "$lane\fulldex\Pokemon Fulldex.exe" | Out-Null
        if ($side -eq "cs") { Copy-Item "$csSource\NativeMonsLauncher.exe" "$lane\launcher\" } else { Copy-Item $target "$lane\launcher\nativemons.exe" }
        Copy-Item "$csSource\games.json" "$lane\launcher\"
        Set-Content -LiteralPath "$lane\state\nativemons.cfg" -Encoding ascii -Value @("love=$fx\love\lovec.exe", "gen1recomp=$fx\engines\gen1recomp", "monbridge=$fx\tools\mon-bridge\mon-bridge.exe", "fulldex=$lane\fulldex\Pokemon Fulldex.exe", "emulators=$lane\emulators", "zakuro=$lane\zakuro\zakuro.exe")
        Set-Content -LiteralPath "$lane\state\rom-folders.txt" -Value "$in\roms" -Encoding utf8
        $exe = if ($side -eq "cs") { "$lane\launcher\NativeMonsLauncher.exe" } else { "$lane\launcher\nativemons.exe" }
        $env:NATIVEMONS_STATE = "$lane\state"; $env:APPDATA = "$lane\appdata"
        foreach ($s in $steps) {
            if ($s[0] -eq "!") { & $s[1]; continue }
            $argv = @($s[1..($s.Count - 1)] | ForEach-Object { (($_ -replace '<out>', "$lane\results\$($s[0]).txt") -replace '<mod-other>', "$script:modOther") -replace '<mod-fit>', "$script:modFit" })
            $p = Start-Process -FilePath $exe -ArgumentList ($argv | ForEach-Object { '"' + $_ + '"' }) -Wait -PassThru -NoNewWindow
            $exit["$side|$($s[0])"] = $p.ExitCode
        }
        if (Test-Path "$root\done-$side") { Remove-Item -Recurse -Force -Confirm:$false "$root\done-$side" }
        Rename-Item $lane "done-$side"
    }
    # Results, then every file both lanes wrote (exes aside); time stamps in names and text are masked.
    function Get-Norm([string]$s) { ($s -replace '\d{8}-\d{6}(-\d{3})?', '<stamp>') -replace '\d{4}-\d{2}-\d{2} \d{2}:\d{2}', '<time>' }
    # Gen1Recomp's own clock in its files (savedAt, slot ids) and its extract workers' order; the rest must match.
    function Get-EngineNorm([string]$s) { ((Get-Norm $s) -replace 'savedAt = \d+', 'savedAt = <t>') -replace '"[0-9a-f]{32}"', '"<id>"' }
    foreach ($s in $steps) {
        if ($s[0] -eq "!") { continue }
        $a = "$root\done-cs\results\$($s[0]).txt"; $b = "$root\done-rs\results\$($s[0]).txt"
        $what = Compare-Files $a $b
        if ($what -like "differs*") { $what += ": C# [" + ((Get-Content -Raw $a) -replace "`r?`n", " / ") + "] Rust [" + ((Get-Content -Raw $b) -replace "`r?`n", " / ") + "]" }
        elseif ($what -eq "identical") { $what += ": " + ((Get-Content -Raw $a) -replace "`r?`n", " / ").Substring(0, [Math]::Min(90, (Get-Content -Raw $a).Length)) }
        foreach ($extra in ".caught") { if (Test-Path "$a$extra") { $what += " | caught " + (Compare-Files "$a$extra" "$b$extra") } }
        Add-Result "saves $($s[0])" $exit["cs|$($s[0])"] $exit["rs|$($s[0])"] $what
    }
    $tree = @{}
    foreach ($side in "cs", "rs") {
        $base = "$root\done-$side"
        Get-ChildItem -LiteralPath $base -Recurse -File | Where-Object { $_.FullName -notlike "$base\launcher\*" -and $_.FullName -notlike "$base\results\*" } | ForEach-Object {
            $rel = Get-Norm $_.FullName.Substring($base.Length)
            $h = if ($_.Extension -in ".txt", ".cfg", ".lua", ".bak") { $t = Get-EngineNorm ([IO.File]::ReadAllText($_.FullName)); if ($_.Name -eq "manifest.lua") { $t = (($t -split "`n") | Sort-Object) -join "`n" }; [BitConverter]::ToString([Security.Cryptography.SHA256]::Create().ComputeHash([Text.Encoding]::UTF8.GetBytes($t))) } else { (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash }
            $key = "$side|$rel"; $n = 1; while ($tree.ContainsKey($key)) { $n++; $key = "$side|$rel#$n" }
            $tree[$key] = $h
        }
    }
    foreach ($side in "cs", "rs") {
        $base = "$root\done-$side\launcher"
        Get-ChildItem -LiteralPath $base -Recurse -File | Where-Object { $_.Name -notin "NativeMonsLauncher.exe.old", "nativemons.exe" } | ForEach-Object {
            $tree["$side|\launcher" + $_.FullName.Substring($base.Length)] = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash
        }
    }
    $rels = @($tree.Keys | ForEach-Object { $_.Substring(3) } | Sort-Object -Unique)
    $bad = @($rels | Where-Object { $tree["cs|$_"] -ne $tree["rs|$_"] })
    Add-Result "saves: state, saves, exports (files)" 0 0 $(if ($bad.Count -eq 0) { "identical ($($rels.Count) files, stamps masked)" } else { "differs in $($bad.Count) of $($rels.Count)" })
    foreach ($x in $bad) { $results.Add("    differs: $x (C# $(if ($tree["cs|$x"]) { 'has' } else { 'none' }), Rust $(if ($tree["rs|$x"]) { 'has' } else { 'none' }))") }
}

if ($Modes -contains "recomp") {
    # --recomp in one lane per side (same path, renamed after): not installed, then copies of the app's Zakuro and build
    # tool with an empty stand-in zig.exe, so a build of a synthetic X dump fails at once and nothing is compiled.
    $lane = "$root\lane"; $in = "$root\inputs"; New-Item -ItemType Directory -Force $in | Out-Null
    $b = New-Object byte[] 0x1000
    [Text.Encoding]::ASCII.GetBytes("NCSD").CopyTo($b, 0x100); [BitConverter]::GetBytes([uint64]0x0004000000055D00).CopyTo($b, 0x108)
    [BitConverter]::GetBytes([uint32]1).CopyTo($b, 0x120); [BitConverter]::GetBytes([uint32]0x10).CopyTo($b, 0x124)
    [Text.Encoding]::ASCII.GetBytes("NCCH").CopyTo($b, 0x300); $b[0x38B] = 1; $b[0x38F] = 0x21
    [BitConverter]::GetBytes([uint32]2).CopyTo($b, 0x3A0); [BitConverter]::GetBytes([uint32]1).CopyTo($b, 0x3A4)
    [BitConverter]::GetBytes([uint32]4).CopyTo($b, 0x3B0); [BitConverter]::GetBytes([uint32]1).CopyTo($b, 0x3B4)
    [Text.Encoding]::ASCII.GetBytes(".code").CopyTo($b, 0x600); [Text.Encoding]::ASCII.GetBytes("IVFC").CopyTo($b, 0xA00)
    $zig = "$lane\state\3ds\zig-0.17.0\zig-x86_64-windows-0.17.0"
    $steps = @(
        @("recomp-none", "--recomp", "<out>", "x", "state"),
        @("!", { New-Item -ItemType Directory -Force "$lane\zakuro", $zig | Out-Null; foreach ($f in "zakuro.exe", "nativemons-recomp.exe", "NATIVEMONS-BUILD.txt") { Copy-Item "$App\engines\zakuro\$f" "$lane\zakuro\" -Force } }),
        @("recomp-state", "--recomp", "<out>", "x", "state"), @("recomp-nosuch", "--recomp", "<out>", "nosuch", "state"), @("recomp-red", "--recomp", "<out>", "red", "state"),
        @("!", { New-Item -ItemType File -Force "$zig\zig.exe" | Out-Null }),
        @("recomp-build", "--recomp", "<out>", "x", "build"), @("recomp-state2", "--recomp", "<out>", "x", "state")
    )
    $exit = @{}
    foreach ($side in "cs", "rs") {
        if (Test-Path $lane) { Remove-Item -Recurse -Force -Confirm:$false $lane }
        New-Item -ItemType Directory -Force "$lane\launcher", "$lane\state\3ds", "$lane\appdata", "$lane\results", "$lane\roms" | Out-Null
        [IO.File]::WriteAllBytes("$lane\roms\x-dump.3ds", $b)
        if ($side -eq "cs") { Copy-Item "$csSource\NativeMonsLauncher.exe" "$lane\launcher\" } else { Copy-Item $target "$lane\launcher\nativemons.exe" }
        Copy-Item "$csSource\games.json" "$lane\launcher\"
        Set-Content -LiteralPath "$lane\state\nativemons.cfg" -Encoding ascii -Value @("zakuro=$lane\zakuro\zakuro.exe", "emulators=$lane\emulators")
        Set-Content -LiteralPath "$lane\state\rom-files.txt" -Value "$lane\roms\x-dump.3ds" -Encoding utf8
        $exe = if ($side -eq "cs") { "$lane\launcher\NativeMonsLauncher.exe" } else { "$lane\launcher\nativemons.exe" }
        $env:NATIVEMONS_STATE = "$lane\state"; $env:APPDATA = "$lane\appdata"
        foreach ($s in $steps) {
            if ($s[0] -eq "!") { & $s[1]; continue }
            $argv = @($s[1..($s.Count - 1)] | ForEach-Object { $_ -replace '<out>', "$lane\results\$($s[0]).txt" })
            $p = Start-Process -FilePath $exe -ArgumentList ($argv | ForEach-Object { '"' + $_ + '"' }) -Wait -PassThru -NoNewWindow
            $exit["$side|$($s[0])"] = $p.ExitCode
        }
        if (Test-Path "$root\recomp-$side") { Remove-Item -Recurse -Force -Confirm:$false "$root\recomp-$side" }
        Rename-Item $lane "recomp-$side"
    }
    foreach ($s in $steps) {
        if ($s[0] -eq "!") { continue }
        $a = "$root\recomp-cs\results\$($s[0]).txt"; $c = "$root\recomp-rs\results\$($s[0]).txt"
        $what = Compare-Files $a $c
        $what += ": " + $(if ($what -eq "identical") { ((Get-Content -Raw $a) -replace "`r?`n", " / ") } else { "C# [" + ((Get-Content -Raw $a) -replace "`r?`n", " / ") + "] Rust [" + ((Get-Content -Raw $c) -replace "`r?`n", " / ") + "]" })
        Add-Result "recomp $($s[0])" $exit["cs|$($s[0])"] $exit["rs|$($s[0])"] $what
    }
    $tb = Compare-Trees "$root\recomp-cs\state" "$root\recomp-rs\state"
    Add-Result "recomp: state files" 0 0 $tb
}

$hashAfter = @{}; foreach ($k in $originals.Keys) { $hashAfter[$k] = (Get-FileHash -LiteralPath $originals[$k] -Algorithm SHA256).Hash }
$results.Add("")
foreach ($k in $originals.Keys) { $results.Add(("original {0}: before {1} after {2} {3}" -f $k, $hashBefore[$k], $hashAfter[$k], ($(if ($hashBefore[$k] -eq $hashAfter[$k]) { "unchanged" } else { "CHANGED" })))) }
$results | Set-Content -LiteralPath "$root\parity-result.txt" -Encoding utf8
$results
if ($Clean) { Remove-Item -Recurse -Force -Confirm:$false $root }

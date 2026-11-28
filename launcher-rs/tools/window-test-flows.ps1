# Window checks for the flows ported after the first window tests (needs the user's OK: a window on monitor 3 for
# about 3 minutes). Seven short runs in a scratch copy of the boxes fixture (<work>\window-test-flows, deleted after
# unless -Keep): the file and folder pickers on Open in PKHeX and Import saves, AzaharPlus's install animation, fan art
# thumbnails arriving, a title-screen capture running behind the menu ("Just a moment" on Play), and PlaySnaps' grab
# of a test window. Downloads come from the flows-data stubs (NATIVEMONS_TEST_LIVE_STUBS), captures from fixture shots
# (no engine starts), the "emulator" and the game's engine are files that are never run. No real input is sent,
# nothing is recorded, no sound. Results: summary.txt in the scratch folder, printed.
# Needs: goldens-boxes.ps1, goldens-fixture.ps1 and flows-data.ps1 made, and the release window build (the -run script
# does it). -Headless: the same runs without a window (nativemons --live-headless), to check a change before the real one.
param([string]$Exe = "", [switch]$Keep, [string]$Only = "", [string]$Data = "", [switch]$Headless)
$ErrorActionPreference = "Stop"
. "$PSScriptRoot\work.ps1"
if (-not $Exe) { $Exe = "$TargetDir\release\nativemons.exe" }
if (-not $Data) { $Data = "$Work\flows-data" }
$root = "$Work\window-test-flows"
$boxes = "$Work\goldens-boxes\rust"
if (-not (Test-Path $Exe)) { throw "no window build at $Exe" }
if (-not (Test-Path "$boxes\engines\fulldex\saves\firered.sav")) { throw "make the boxes fixture first (goldens-boxes.ps1 make)" }
if (-not (Test-Path "$Data\emu.zip")) { throw "make the flows data first (flows-data.ps1 make)" }
if (Test-Path $root) { Remove-Item -Recurse -Force $root }
$side = "$root\side"
robocopy $boxes $side /E /XD "$boxes\out" "$boxes\replay" "$boxes\heat" "$boxes\perf" /NFL /NDL /NJH /NJS /NP | Out-Null
New-Item -ItemType Directory -Force "$side\state", "$side\appdata", "$side\fonts", "$root\logs", "$root\shots" | Out-Null
Copy-Item $Exe "$side\nativemons.exe" -Force
Copy-Item "$Work\goldens-boxes\csharp\games.json" $side -Force
Copy-Item "$Work\goldens\csharp\fonts\*", "$PSScriptRoot\..\fonts\*" "$side\fonts" -Force
# State as on a set-up install (so no guided setup): a ROM folder, the wallpaper chosen, FireRed as the last game.
$fixture = "$Work\goldens\fixture\roms"
if (-not (Test-Path $fixture)) { throw "make the ROM fixture first (goldens-fixture.ps1 make)" }
[IO.File]::WriteAllText("$side\state\rom-folders.txt", "$fixture`r`n")
[IO.File]::WriteAllText("$side\state\last-game.txt", "firered")
New-Item -ItemType Directory -Force "$side\engines\fulldex", "$side\engines\pokeplatinum" | Out-Null
foreach ($f in "$side\engines\fulldex\Pokemon Fulldex.exe", "$side\engines\pokeplatinum\main.exe") { if (-not (Test-Path -LiteralPath $f)) { New-Item -ItemType File $f | Out-Null } }
[IO.File]::WriteAllText("$side\state\wallpaper.txt", "drawn")
$emuSha = (Get-FileHash "$Data\emu.zip").Hash.ToLower()
# Fixture shots for the capture run: a lit band with both screens, the right one moving (as art-test's).
Add-Type -AssemblyName System.Drawing
for ($i = 0; $i -lt 48; $i++) {
    $b = New-Object Drawing.Bitmap 960, 720, ([Drawing.Imaging.PixelFormat]::Format24bppRgb)
    $g = [Drawing.Graphics]::FromImage($b); $g.Clear([Drawing.Color]::Black)
    $g.FillRectangle((New-Object Drawing.SolidBrush ([Drawing.Color]::FromArgb(30, 60, 160))), 0, 180, 480, 360)
    $g.FillEllipse([Drawing.Brushes]::Gold, 120, 260, 240, 120)
    $g.FillRectangle((New-Object Drawing.SolidBrush ([Drawing.Color]::FromArgb(40 + $i * 3, 140, 90))), 480, 180, 480, 360)
    $g.FillEllipse([Drawing.Brushes]::White, 520 + $i * 6, 300, 60, 60)
    $g.Dispose(); $b.Save(("$root\shots\shot-{0:D4}.bmp" -f ($i * 6)), [Drawing.Imaging.ImageFormat]::Bmp); $b.Dispose()
}
Add-Type -Namespace W -Name U -MemberDefinition '[DllImport("user32.dll")] public static extern bool PostMessageW(System.IntPtr h, uint m, System.IntPtr w, System.IntPtr l);'
Add-Type -Namespace W -Name E -MemberDefinition @"
public delegate bool EnumProc(System.IntPtr h, System.IntPtr l);
[DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, System.IntPtr l);
[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(System.IntPtr h, out uint pid);
[DllImport("user32.dll")] public static extern bool IsWindowVisible(System.IntPtr h);
"@
# A visible window of the launcher's process other than its own (a picker), or zero.
function Find-Other($main) {
    $mainPid = [uint32]0; [void][W.E]::GetWindowThreadProcessId($main, [ref]$mainPid); $script:found = [IntPtr]::Zero
    [void][W.E]::EnumWindows({ param($h, $l) $p = [uint32]0; [void][W.E]::GetWindowThreadProcessId($h, [ref]$p); if ($p -eq $mainPid -and $h -ne $main -and [W.E]::IsWindowVisible($h)) { $script:found = $h; return $false }; return $true }, [IntPtr]::Zero)
    $script:found
}
# Waits up to $s seconds for a picker, notes it in the log and closes it (a cancel).
function Close-Picker($h, $log, [int]$s) {
    $t0 = Get-Date
    while (((Get-Date) - $t0).TotalSeconds -lt $s) {
        $o = Find-Other $h
        if ($o -ne [IntPtr]::Zero) { Start-Sleep -Milliseconds 1500; Add-Content $log "harness: picker window up, closed"; [void][W.U]::PostMessageW($o, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero); return }
        Start-Sleep -Milliseconds 250
    }
    Add-Content $log "harness: no picker window within $s s"
}
$stubs = "NATIVEMONS_TEST_LIVE_STUBS", "NATIVEMONS_TEST_FLOWS", "NATIVEMONS_TEST_FETCH", "NATIVEMONS_TEST_SHA256", "NATIVEMONS_TEST_ART_SHOTS", "NATIVEMONS_TEST_ART_PLAN", "NATIVEMONS_TEST_ART_DELAY", "NATIVEMONS_TEST_NO_CAPTURE"
function Start-Run($name, [string]$inputText, [string]$open, [hashtable]$vars, [int]$limitS, [scriptblock]$during) {
    if ($Only -and -not (@($Only -split ';') -contains $name)) { return }
    $log = "$root\logs\$name.log"
    [IO.File]::WriteAllText("$side\state\light-mode.txt", "off")
    $env:NATIVEMONS_STATE = "$side\state"; $env:APPDATA = "$side\appdata"; $env:NATIVEMONS_FONTS_DIR = "$side\fonts"
    $env:NATIVEMONS_TEST_SCREEN = "DISPLAY3"; $env:NATIVEMONS_TEST_LOG = $log
    foreach ($v in ($stubs + @("NATIVEMONS_TEST_T", "NATIVEMONS_TEST_INPUT", "NATIVEMONS_TEST_OPEN", "NATIVEMONS_TEST_FRAMES"))) { Remove-Item "env:$v" -ErrorAction SilentlyContinue }
    $env:NATIVEMONS_TEST_NO_CAPTURE = "1"
    foreach ($k in $vars.Keys) { if ($vars[$k] -eq $null) { Remove-Item "env:$k" -ErrorAction SilentlyContinue } else { Set-Item "env:$k" $vars[$k] } }
    if ($inputText) { [IO.File]::WriteAllText("$root\logs\$name.input.txt", $inputText); $env:NATIVEMONS_TEST_INPUT = "$root\logs\$name.input.txt" }
    if ($open) { $env:NATIVEMONS_TEST_OPEN = $open }
    # nativemons.exe is a console program: CreateNoWindow keeps its console from showing (the launcher window still does).
    $psi = New-Object Diagnostics.ProcessStartInfo "$side\nativemons.exe"
    $psi.Arguments = if ($Headless) { "--live-headless $($limitS - 3)" } else { "--launcher" }
    $psi.UseShellExecute = $false; $psi.CreateNoWindow = $true
    $p = [Diagnostics.Process]::Start($psi)
    $hwnd = [IntPtr]::Zero
    $t0 = Get-Date
    while (-not $Headless -and ((Get-Date) - $t0).TotalSeconds -lt 10 -and $hwnd -eq [IntPtr]::Zero) {
        Start-Sleep -Milliseconds 200
        $l = if (Test-Path $log) { Select-String -Path $log -Pattern '^\d+ hwnd (\d+)' | Select-Object -First 1 } else { $null }
        if ($l) { $hwnd = [IntPtr][long]$l.Matches[0].Groups[1].Value }
    }
    if ($during -and $hwnd -ne [IntPtr]::Zero) { & $during $hwnd $log }
    $ended = $p.WaitForExit($limitS * 1000)
    if (-not $ended) { Stop-Process -Id $p.Id -Force; Add-Content $log "harness: stopped after $limitS s" }
    foreach ($v in ($stubs + @("NATIVEMONS_TEST_INPUT", "NATIVEMONS_TEST_OPEN"))) { Remove-Item "env:$v" -ErrorAction SilentlyContinue }
    [pscustomobject]@{ name = $name; ended = $ended; log = $log; seconds = [int]((Get-Date) - $t0).TotalSeconds }
}

$runs = @()
$flows = @{ NATIVEMONS_TEST_LIVE_STUBS = "1"; NATIVEMONS_TEST_FLOWS = "1"; NATIVEMONS_TEST_FETCH = "$Data\fetch" }
# 1. Open in PKHeX on FireRed's page: PKHeX isn't known yet, so its file picker opens; the harness cancels it.
# Input by page and item names: "waitpage" holds the later steps until that page is on top (see live.rs).
$runs += Start-Run "picker-pkhex" "0 waitpage Pokemon FireRed`n1500 clicktext Open in PKHeX`n9000 close`n" "game:firered" @{} 25 { param($h, $log) Close-Picker $h $log 6 }
# 2. Settings, Import saves, Add a save folder: the folder picker opens; canceled.
$runs += Start-Run "picker-import" "0 waitpage Settings`n1500 clicktext Import saves`n1600 waitpage Import saves`n3500 clicktext Add a save folder`n11000 close`n" "settings" @{} 25 { param($h, $log) Start-Sleep 3; Close-Picker $h $log 6 }
# 3. Emulators: 3DS games on, AzaharPlus, Install: the download (a stub), check and unpack under the spinning page.
$v = $flows.Clone(); $v.NATIVEMONS_TEST_SHA256 = $emuSha
$runs += Start-Run "emu-install" "0 waitpage Emulators`n1000 clicktext 3DS games`n1600 clicktext AzaharPlus`n1700 waitpage AzaharPlus`n2300 clicktext Install`n12000 close`n" "emu" $v 30 $null
# 4. Wallpapers: Red's own picture, fan art search: the results page, its thumbnails arriving one by one.
$runs += Start-Run "wall-thumbs" "0 waitpage Wallpapers`n1000 clicktext Your own pictures`n1100 waitpage Your own pictures`n1500 clicktext Pokemon Red`n1600 waitpage Pokemon Red`n2000 clicktext Find a wallpaper`n2100 waitpage Find a wallpaper`n2500 clicktext Fan art: Red`n10000 close`n" "wallpapers" $flows.Clone() 25 $null
# 5. A title-screen capture behind the menu (fixture shots, held 8 s; the start's own capture first holds the splash
#    8 s): CaptureMissing starts ~3 s after the menu shows, Play at 4 s waits on "Just a moment" until it stops, then
#    carries on (the engine is an empty file: "Could not start").
$v = @{ NATIVEMONS_TEST_NO_CAPTURE = $null; NATIVEMONS_TEST_ART_SHOTS = "$root\shots"; NATIVEMONS_TEST_ART_PLAN = "$root\logs\capture-plan.txt"; NATIVEMONS_TEST_ART_DELAY = "8000" }
$runs += Start-Run "capture-behind" "0 waitpage nativeMons`n4000 clicktext Play`n14000 close`n" "" $v 35 $null
# 6. PlaySnaps' grab: a test window (a lit 5:3 picture on black, 1000x900, in front on monitor 3) stands in for the
#    emulator; nativemons --play-snap-grab takes its client area and judges it. The window closes itself after 8 s.
if (-not $Headless -and (-not $Only -or (@($Only -split ';') -contains "playsnaps-grab"))) {
    $form = @'
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
$f = New-Object Windows.Forms.Form; $f.Text = "PlaySnaps test window"; $f.StartPosition = "Manual"; $f.FormBorderStyle = "FixedSingle"
$f.Location = New-Object Drawing.Point 2020, 60; $f.ClientSize = New-Object Drawing.Size 1000, 900; $f.BackColor = [Drawing.Color]::Black; $f.TopMost = $true
$b = New-Object Drawing.Bitmap 600, 360; $g = [Drawing.Graphics]::FromImage($b); $r = New-Object Random 5
for ($y = 0; $y -lt 360; $y += 12) { for ($x = 0; $x -lt 600; $x += 12) { $c = 40 + $r.Next(200); $g.FillRectangle((New-Object Drawing.SolidBrush ([Drawing.Color]::FromArgb($c, $c, [int]($c * 0.8)))), $x, $y, 12, 12) } }
$g.Dispose(); $pb = New-Object Windows.Forms.PictureBox; $pb.Image = $b; $pb.SetBounds(200, 40, 600, 360); $f.Controls.Add($pb)
$t = New-Object Windows.Forms.Timer; $t.Interval = 8000; $t.Add_Tick({ $f.Close() }); $t.Start()
$f.Add_Shown({ $f.Activate() }); [void]$f.ShowDialog()
'@
    [IO.File]::WriteAllText("$root\logs\snap-window.ps1", $form)
    $w = Start-Process powershell -ArgumentList "-NoProfile", "-ExecutionPolicy", "Bypass", "-WindowStyle", "Hidden", "-File", "`"$root\logs\snap-window.ps1`"" -PassThru
    Start-Sleep 3
    $grab = & "$side\nativemons.exe" --play-snap-grab $w.Id "$root\logs\snap" 2>&1
    if (-not $w.WaitForExit(15000)) { Stop-Process -Id $w.Id -Force }
    $runs += [pscustomobject]@{ name = "playsnaps-grab"; ended = $true; log = $null; seconds = 0; text = "$grab" }
}

# Summary from the logs: pages in order (with the job or capture running), pickers, frames, how each run ended.
$out = @()
foreach ($r in $runs) {
    if (-not $r.log) { $out += "$($r.name): $($r.text)"; continue }
    $lines = @(Get-Content $r.log -ErrorAction SilentlyContinue)
    $frames = @($lines | Where-Object { $_ -match '^(\d+) frame ([0-9.]+) ms' } | ForEach-Object { [pscustomobject]@{ t = [int]$Matches[1]; ms = [double]$Matches[2] } })
    $fps = if ($frames.Count -gt 1) { "{0} frames, {1:0.00} ms a frame (max {2:0.00})" -f $frames.Count, (($frames | Measure-Object ms -Average).Average), (($frames | Measure-Object ms -Maximum).Maximum) } else { "no frames" }
    $pages = (@($lines | Where-Object { $_ -match ' page ' } | ForEach-Object { ($_ -split ' ', 2)[0] + "ms " + ($_ -split ' page ', 2)[1] }) -join ' > ')
    $extra = @($lines | Where-Object { $_ -match ' (dialog |harness: |scripted: |scripted Page|effect Launch)' } | ForEach-Object { ($_ -split ' ', 2)[1] }) -join '; '
    $capt = @($lines | Where-Object { $_ -match '\(capturing\)' }).Count
    $exit = $lines | Where-Object { $_ -match ' exit ' } | Select-Object -Last 1
    $slow = @($lines | Where-Object { $_ -match ' slow frame ' } | ForEach-Object { ($_ -split ' slow frame ', 2)[1] })
    $out += "$($r.name): ended by itself $($r.ended) after $($r.seconds) s | $fps | pages $pages | $extra | frames while capturing $capt | $exit"
    if ($slow.Count) { $out += "  over 16 ms: $($slow.Count): " + (($slow | Select-Object -First 6) -join '; ') }
}
if (Test-Path "$root\logs\capture-plan.txt") { $out += "capture plan: " + ((Get-Content "$root\logs\capture-plan.txt" | Select-Object -First 3) -join ' / ') }
$out | Set-Content -Encoding utf8 "$root\summary.txt"
$out
if (-not $Keep) { Copy-Item "$root\summary.txt" "$env:TEMP\nativemons-window-test-flows-summary.txt" -Force; Remove-Item -Recurse -Force $root; "scratch folder removed; summary kept at $env:TEMP\nativemons-window-test-flows-summary.txt" }

# Scripted input sequences (tools\replays\*.txt), offscreen: each snap is diffed against the C# golden of that screen.
# Uses the same sides and env as run-goldens.ps1 (run that first so <root>\csharp\out holds the goldens).
# -Scripts: semicolon list of script names (without .txt); -Root: the golden root the scripts were written for.
# -CSharp: the C# launcher plays the same script (--replay, a test-only hook) in <root>\csharp-replay, a copy of the C#
# side; every snap is diffed against the C# one, and replay.log's pages and selections are compared step by step.
# -Frames: both save every frame drawn (frames\f-<ms>-<n>.png), diffed pair by pair, deleted after unless -KeepFrames.
param([string]$Scripts = "keys-games;pad-settings;pad-gameoptions;pad-repeat;mouse-folders;mouse-back;back-main;games-game;wheel-settings;controls;controls-sheet", [string]$Size = "600x1000",
      [string]$Root = "", [switch]$CSharp, [switch]$Frames, [switch]$KeepFrames, [switch]$DryRun,
      [string]$CSharpExe = "",
      [string]$Fixture = "")
$ErrorActionPreference = "Stop"
. "$PSScriptRoot\work.ps1"
$target = "$TargetDir\debug"
if (-not $Root) { $Root = "$Work\goldens" }; if (-not $CSharpExe) { $CSharpExe = "$Work\csharp-replay\after\NativeMonsLauncher.exe" }
if (-not $Fixture) { $Fixture = if ($env:NATIVEMONS_FIXTURE) { $env:NATIVEMONS_FIXTURE } else { "$Work\goldens\fixture\roms" } }
$fixture = $Fixture
$env:NATIVEMONS_TEST_T = "7.25"; $env:NATIVEMONS_TEST_SEED = "3"; $env:NATIVEMONS_TEST_BATTERY = "60"
$env:NATIVEMONS_TEST_FONTS = "bundled"; $env:NATIVEMONS_SNAPSHOT_TEXTMASK = "1"; $env:NATIVEMONS_TEST_QUIET = "1"
$env:NATIVEMONS_TEST_DEX = "sample"; $env:NATIVEMONS_TEST_NO_CAPTURE = "1"; $env:NATIVEMONS_TEST_CSHARP_ROWS = "1"
# -DryRun: Play and the other starts are noted (effect Launch), never started (NATIVEMONS_TEST_DRYRUN=1, both launchers).
if ($DryRun) { $env:NATIVEMONS_TEST_DRYRUN = "1" } else { Remove-Item env:NATIVEMONS_TEST_DRYRUN -ErrorAction SilentlyContinue }
if ($Frames) { $env:NATIVEMONS_REPLAY_FRAMES = "1" } else { Remove-Item env:NATIVEMONS_REPLAY_FRAMES -ErrorAction SilentlyContinue }
$rs = "$Root\rust"; $cs = "$Root\csharp"; $cr = "$Root\csharp-replay"
New-Item -ItemType Directory -Force "$rs\state", "$rs\appdata", "$rs\replay", "$rs\fonts", "$rs\engines\fulldex", "$rs\engines\pokeplatinum" | Out-Null
foreach ($f in "$rs\engines\fulldex\Pokemon Fulldex.exe", "$rs\engines\pokeplatinum\main.exe") { if (-not (Test-Path -LiteralPath $f)) { New-Item -ItemType File $f | Out-Null } }
if (Test-Path $fixture) { [IO.File]::WriteAllText("$rs\state\rom-folders.txt", "$fixture`r`n") }
Copy-Item "$target\nativemons.exe", "$cs\games.json" $rs -Force
Copy-Item "$cs\fonts\*", "$PSScriptRoot\..\fonts\*" "$rs\fonts" -Force
if ($CSharp) {
    # A copy of the C# side (its engines and state), so the goldens' own state is never changed by a script.
    robocopy $cs $cr /E /XD "$cs\out" "$cs\anim" /NFL /NDL /NJH /NJS /NP | Out-Null
    New-Item -ItemType Directory -Force "$cr\replay", "$cr\state", "$cr\appdata", "$cr\engines\fulldex", "$cr\engines\pokeplatinum" | Out-Null
    foreach ($f in "$cr\engines\fulldex\Pokemon Fulldex.exe", "$cr\engines\pokeplatinum\main.exe") { if (-not (Test-Path -LiteralPath $f)) { New-Item -ItemType File $f | Out-Null } }
    if (Test-Path $fixture) { [IO.File]::WriteAllText("$cr\state\rom-folders.txt", "$fixture`r`n") }
    Copy-Item $CSharpExe "$cr\NativeMonsLauncher.exe" -Force
}
# Each script starts from the same state: options and folds it changes are put back.
function Reset-State($dir) { Remove-Item -Recurse -Force -ErrorAction SilentlyContinue "$dir\state\quick-start"; foreach ($f in "games-folded.txt", "game-options.txt", "quick-start.txt", "wallpaper.txt", "3ds-games.txt", "light-mode.txt", "engine-platinum.txt", "last-game.txt", "layout.txt") { Remove-Item -Force -ErrorAction SilentlyContinue "$dir\state\$f" } }
# replay.log without the Rust-only effects (cursor and window outline), for the step-by-step comparison.
# Each side's own folder reads as <side> (a dry run's Launch lines name programs and folders in it, escaped as Rust Debug).
function Log-Steps($file, $side) {
    $esc = $side.Replace('\', '\\')
    # OpenFolder: the C# logs its path as is, the Rust Debug-escaped.
    @(Get-Content $file | Where-Object { $_ -notmatch 'effect (HandCursor|SetRegion)' -and $_ -notmatch '^frames drawn' } | ForEach-Object { $l = $_.Replace($esc, '<side>').Replace($side, '<side>'); if ($l -match 'effect OpenFolder') { $l.Replace('\\', '\') } else { $l } })
}
function Diff-Pair($golden, $ours, $name) {
    $line = & "$target\golden-diff.exe" $golden $ours $name
    $line
}
$summary = @()
foreach ($s in @($Scripts -split ';' | Where-Object { $_ })) {
    $out = "$rs\replay\$s"
    $deliberate = @(Get-Content "$PSScriptRoot\replays\deliberate.txt" -ErrorAction SilentlyContinue | Where-Object { $_ -and -not $_.StartsWith('#') } | ForEach-Object { $f = $_ -split "`t"; if ($f[0] -eq $s) { [pscustomobject]@{ Step = $f[1]; From = [int]$f[2]; To = [int]$f[3] } } })
    $inWindow = { param($n) if ($n -match '^f-(\d+)-') { $ms = [int]$Matches[1]; foreach ($d in $deliberate) { if ($ms -ge $d.From -and $ms -le $d.To) { return $true } } }; $false }
    $left = @(foreach ($d in $deliberate) { $p = $d.Step -split '-'; [int]$p[0]..[int]$p[-1] })
    if (Test-Path $out) { Remove-Item -Recurse -Force $out }
    Reset-State $rs
    Write-Output "=== $s"
    $env:NATIVEMONS_STATE = "$rs\state"; $env:APPDATA = "$rs\appdata"; $env:NATIVEMONS_FONTS_DIR = "$rs\fonts"
    # Discord's frames (the replay's discord command) go to a file per side, never to Discord.
    New-Item -ItemType Directory -Force $out | Out-Null; $env:NATIVEMONS_TEST_DISCORD = "$out\discord.txt"
    $ErrorActionPreference = "Continue"
    $said = & "$rs\nativemons.exe" --replay "$PSScriptRoot\replays\$s.txt" $out $Size 2>&1
    $ErrorActionPreference = "Stop"
    if (-not (Test-Path "$out\replay.log")) { Write-Output "rust: no replay.log: $($said | Select-Object -Last 2)"; $summary += "$s`: Rust replay failed"; continue }
    Write-Output ("rust: " + (Get-Content "$out\replay.log" -Tail 1))
    if ($CSharp) {
        $cout = "$cr\replay\$s"
        if (Test-Path $cout) { Remove-Item -Recurse -Force $cout }
        Reset-State $cr
        $env:NATIVEMONS_STATE = "$cr\state"; $env:APPDATA = "$cr\appdata"; Remove-Item env:NATIVEMONS_FONTS_DIR -ErrorAction SilentlyContinue
        New-Item -ItemType Directory -Force $cout | Out-Null; $env:NATIVEMONS_TEST_DISCORD = "$cout\discord.txt"
        $p = Start-Process -FilePath "$cr\NativeMonsLauncher.exe" -ArgumentList "--replay", "`"$PSScriptRoot\replays\$s.txt`"", "`"$cout`"", $Size -Wait -PassThru -WindowStyle Hidden
        if (-not (Test-Path "$cout\replay.log")) { Write-Output "c#: no replay.log (exit $($p.ExitCode)): $(Get-Content "$cout\replay-error.txt" -TotalCount 1 -ErrorAction SilentlyContinue)"; $summary += "$s`: C# replay failed"; continue }
        Write-Output ("c#:   " + (Get-Content "$cout\replay.log" -Tail 1))
        $a = Log-Steps "$out\replay.log" $rs; $b = Log-Steps "$cout\replay.log" $cr
        # A deliberate difference's step (replays\deliberate.txt) is left out.
        # A step is "6" or a range "6-27"; the indented effect lines under a left-out step go with it.
        $drop = { param($lines) $skip = $false; @(foreach ($l in $lines) { if ($l -match '^(\d+): ') { $skip = $left -contains [int]$Matches[1] } elseif (-not $l.StartsWith('  ')) { $skip = $false }; if (-not $skip) { $l } }) }
        $a = & $drop $a; $b = & $drop $b
        $steps = @(Compare-Object $a $b -SyncWindow 0)
        if ($steps.Count -eq 0) { Write-Output "log: every step's page, selection and effects match" }
        else { Write-Output "log: $($steps.Count) lines differ"; $steps | Select-Object -First 12 | ForEach-Object { Write-Output ("  " + $_.SideIndicator + " " + $_.InputObject) } }
        $fr = (Get-Content "$out\replay.log" | Select-String '^frames drawn').Line; $fc = (Get-Content "$cout\replay.log" | Select-String '^frames drawn').Line
        Write-Output "frames: rust $fr, c# $fc"
        # Discord's payloads with the nonce, process id and start time (different every run) read as <n>.
        if ((Test-Path "$out\discord.txt") -or (Test-Path "$cout\discord.txt")) {
            $norm = { param($f) @(Get-Content -Encoding utf8 $f -ErrorAction SilentlyContinue | ForEach-Object { $_ -replace '"nonce":"[0-9a-f-]+"', '"nonce":"<n>"' -replace '"pid":[0-9]+', '"pid":<n>' -replace '"start":[0-9]+', '"start":<n>' }) }
            $da = & $norm "$out\discord.txt"; $db = & $norm "$cout\discord.txt"
            $dd = @(Compare-Object $da $db -SyncWindow 0)
            Write-Output ("discord: {0} frames rust, {1} c#, {2}" -f $da.Count, $db.Count, $(if ($dd.Count -eq 0) { "identical" } else { "$($dd.Count) lines differ" }))
            $dd | Select-Object -First 6 | ForEach-Object { Write-Output ("  " + $_.SideIndicator + " " + $_.InputObject) }
        }
    }
    Get-ChildItem $out -Filter "*.png" | Where-Object { $_.Name -notmatch '-early\.png$' } | ForEach-Object {
        $n = $_.BaseName; $golden = "$cs\out\$n.png"
        if ($Size -ne '600x1000') { $golden = "$cs\out\$n-$Size.png" }
        if (Test-Path $golden) { $r = Diff-Pair $golden $_.FullName "$s/$n vs golden"; $r; $summary += $r } else { Write-Output "$s/$n`: no C# golden" }
        if ($CSharp) {
            $c = "$cr\replay\$s\$n.png"
            $snapStep = (Get-Content "$out\replay.log" | Select-String "^(\d+): snap $n( |$)" | Select-Object -First 1)
            if ($snapStep -and $left -contains [int]$snapStep.Matches[0].Groups[1].Value) { Write-Output "$s/$n`: a deliberate difference (replays\deliberate.txt), not compared"; return }
            if (Test-Path $c) { $r = Diff-Pair $c $_.FullName "$s/$n vs C# replay"; $r; $summary += $r } else { Write-Output "$s/$n`: no C# replay snap" }
        }
    }
    if ($CSharp -and $Frames) {
        $rf = @(Get-ChildItem "$out\frames" -Filter "*.png" -ErrorAction SilentlyContinue | ForEach-Object { $_.Name })
        $cf = @(Get-ChildItem "$cr\replay\$s\frames" -Filter "*.png" -ErrorAction SilentlyContinue | ForEach-Object { $_.Name })
        $kept = @(@($rf) + @($cf) | Where-Object { & $inWindow $_ } | Sort-Object -Unique)
        $rf = @($rf | Where-Object { -not (& $inWindow $_) }); $cf = @($cf | Where-Object { -not (& $inWindow $_) })
        if ($kept.Count) { Write-Output "frames of a deliberate difference (replays\deliberate.txt), not compared: $($kept.Count)" }
        $both = @($rf | Where-Object { $cf -contains $_ })
        $only = @(Compare-Object $rf $cf)
        $pass = 0; $fail = @(); $minIou = 100.0
        # Residuals accepted by hand for one frame each (tools\replays\residuals.txt), added to the Rust frame's accept list.
        $res = @(Get-Content "$PSScriptRoot\replays\residuals.txt" -ErrorAction SilentlyContinue | Where-Object { $_ -and -not $_.StartsWith('#') })
        foreach ($l in $res) { $f = $l -split "`t"; if ($f[0] -like "$s/*") { $png = $f[0].Substring($s.Length + 1); if (Test-Path "$out\frames\$png") { Add-Content -Path ("$out\frames\" + [IO.Path]::ChangeExtension($png, ".accept.txt")) -Value ($f[1] + "`t" + $f[2]) } } }
        foreach ($n in $both) {
            $r = Diff-Pair "$cr\replay\$s\frames\$n" "$out\frames\$n" "$s/$n"
            if ($r -match '\| IoU ([0-9.]+)%') { $minIou = [Math]::Min($minIou, [double]$Matches[1]) }
            if ($r -match ': PASS') { $pass++ } else { $fail += $r }
        }
        Write-Output ("frames diffed: {0}, PASS {1}, FAIL {2}, lowest raw IoU {3:0.0000}%; drawn by one side only: {4}" -f $both.Count, $pass, $fail.Count, $minIou, $only.Count)
        $only | Select-Object -First 8 | ForEach-Object { Write-Output ("  only " + $(if ($_.SideIndicator -eq '<=') { "rust" } else { "c#" }) + ": " + $_.InputObject) }
        $fail | Select-Object -First 8 | ForEach-Object { Write-Output ("  " + $_) }
        $summary += ("{0}/frames: {1} of {2} PASS, lowest IoU {3:0.0000}%, one side only {4}" -f $s, $pass, $both.Count, $minIou, $only.Count)
        if (-not $KeepFrames) { Remove-Item -Recurse -Force -ErrorAction SilentlyContinue "$out\frames", "$cr\replay\$s\frames" }
    }
}
Write-Output "=== summary"
$summary | ForEach-Object { ($_ -split ' \| ')[0] + $(if ($_ -match '\| IoU ([0-9.]+)% \(([0-9.]+)%') { " IoU $($Matches[1])% ($($Matches[2])%)" } else { "" }) }

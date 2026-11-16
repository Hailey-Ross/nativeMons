# Merges a player's shared Zakuro findings (an issue from the launcher's "Share Zakuro findings") into the definitions
# branch of the Zakuro fork, nativemons-hints (hints\<title id>-<code hash>.hints and index.txt), after checking them.
# Launchers download changed lists from that branch, so -Push puts them in front of every player within hours.
#   merge.ps1 -Issue 123 -Branch <checkout of the nativemons-hints branch> [-Push] [-Close]
param([Parameter(Mandatory)][int]$Issue, [Parameter(Mandatory)][string]$Branch, [switch]$Push, [switch]$Close)
$ErrorActionPreference = 'Stop'
$repo = 'Hailey-Ross/nativeMons'
# The titles 3DSrecomp+Zakuro builds (Recomp.Titles in the launcher).
$titles = '0004000000055D00', '0004000000055E00', '000400000011C400', '000400000011C500', '0004000000164800', '0004000000175E00', '00040000001B5000', '00040000001B5100'

$i = gh issue view $Issue -R $repo --json title,body,labels | ConvertFrom-Json
if (-not ($i.labels.name -contains 'zakuro-hints')) { throw "Issue $Issue isn't labelled zakuro-hints." }
$code = [regex]::Match($i.body, '### Code version\s+([0-9A-Fa-f]{16}-[0-9A-Fa-f]{16})').Groups[1].Value.ToUpperInvariant()
if (-not $code) { throw 'No code version (<title id>-<code hash>) in the issue.' }
if ($titles -notcontains $code.Substring(0, 16)) { throw "Title $($code.Substring(0, 16)) isn't one 3DSrecomp+Zakuro builds." }
$url = [regex]::Match($i.body, 'https://github\.com/user-attachments/files/\d+/[^\s\)\]]+').Value
$tmp = Join-Path ([IO.Path]::GetTempPath()) "zakuro-hints-$Issue.txt"
if ($url) { Invoke-WebRequest -UseBasicParsing $url -OutFile $tmp }
else {
    # No file attached: the lines pasted into the form's file box (in a code block or not).
    $box = [regex]::Match($i.body, '### The \.hints file\s+(.*?)(
### |\z)', 'Singleline').Groups[1].Value
    $box = ($box -replace '```[a-z]*', '').Trim()
    if (-not $box) { throw 'No attached file and no pasted list in the issue.' }
    [IO.File]::WriteAllText($tmp, $box)
}
if ((Get-Item $tmp).Length -gt 2MB) { Remove-Item $tmp; throw 'The file is over 2 MB: not a hint list.' }
$bad = 0; $got = New-Object System.Collections.Generic.List[uint32]
foreach ($line in Get-Content $tmp) {
    $l = $line.Trim()
    if ($l -eq '' -or $l.StartsWith('#')) { continue }
    if ($l -notmatch '^(0x)?([0-9A-Fa-f]{1,8})$') { $bad++; continue }
    $a = [Convert]::ToUInt32($Matches[2], 16)
    # The code of a 3DS title starts at 0x00100000; hints point into it (3dsrecomp keeps only those inside the code).
    if ($a -lt 0x00100000 -or $a -ge 0x10000000) { $bad++; continue }
    $got.Add($a)
}
Remove-Item $tmp
if ($bad -gt 0) { throw "$bad lines aren't addresses in a game's code: nothing merged." }
if ($got.Count -eq 0) { throw 'The file has no addresses.' }

if ((git -C $Branch rev-parse --abbrev-ref HEAD) -ne 'nativemons-hints') { throw "$Branch isn't on the nativemons-hints branch." }
git -C $Branch pull -q --ff-only
$dir = Join-Path $Branch 'hints'
New-Item -ItemType Directory -Force $dir | Out-Null
$file = Join-Path $dir "$code.hints"
$have = New-Object System.Collections.Generic.SortedSet[uint32]
if (Test-Path $file) { foreach ($l in Get-Content $file) { if ($l -match '^[0-9A-F]{8}$') { [void]$have.Add([Convert]::ToUInt32($l, 16)) } } }
$before = $have.Count
foreach ($a in $got) { [void]$have.Add($a) }
$added = $have.Count - $before
# LF only: index.txt checksums the exact bytes the launcher downloads.
[IO.File]::WriteAllText($file, (($have | ForEach-Object { $_.ToString('X8') + "`n" }) -join ''))
$index = Get-ChildItem $dir -Filter *.hints | Sort-Object Name | ForEach-Object { "$($_.Name) $((Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant())`n" }
[IO.File]::WriteAllText((Join-Path $Branch 'index.txt'), ($index -join ''))
"Issue ${Issue}: $($got.Count) addresses for $code, $added new; $file now holds $($have.Count)."
if ($Push -and $added -gt 0) {
    git -C $Branch add hints index.txt
    git -C $Branch commit -q -m "Findings from nativeMons issue $Issue for $code"
    git -C $Branch push -q origin nativemons-hints
    'Pushed: launchers pick it up at their next check.'
}
if ($Close) {
    gh issue comment $Issue -R $repo --body "Thank you! $added new places from this list are in the shared list for $code, and reach everyone's launcher at its next check, which rebuilds the game quietly to cover them."
    gh issue close $Issue -R $repo
}

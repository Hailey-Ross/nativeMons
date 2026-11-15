# Merges a player's shared Zakuro findings (an issue from the launcher's "Share Zakuro findings") into the shared lists
# of the Zakuro fork (nativemons-hints\<title id>-<code hash>.hints), after checking them. Commit and push the fork
# yourself afterwards: its release ships the lists to every player.
#   merge.ps1 -Issue 123 -Fork <zakuro checkout> [-Close]
param([Parameter(Mandatory)][int]$Issue, [Parameter(Mandatory)][string]$Fork, [switch]$Close)
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
if (-not $url) { throw 'No attached file in the issue.' }

$tmp = Join-Path ([IO.Path]::GetTempPath()) "zakuro-hints-$Issue.txt"
Invoke-WebRequest -UseBasicParsing $url -OutFile $tmp
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

$dir = Join-Path $Fork 'nativemons-hints'
New-Item -ItemType Directory -Force $dir | Out-Null
$file = Join-Path $dir "$code.hints"
$have = New-Object System.Collections.Generic.SortedSet[uint32]
if (Test-Path $file) { foreach ($l in Get-Content $file) { if ($l -match '^[0-9A-F]{8}$') { [void]$have.Add([Convert]::ToUInt32($l, 16)) } } }
$before = $have.Count
foreach ($a in $got) { [void]$have.Add($a) }
$added = $have.Count - $before
Set-Content -LiteralPath $file -Value ($have | ForEach-Object { $_.ToString('X8') }) -Encoding ascii
"Issue ${Issue}: $($got.Count) addresses for $code, $added new; $file now holds $($have.Count)."
if ($Close) {
    gh issue comment $Issue -R $repo --body "Thank you! $added new places from this list are in the shared list for $code, and reach everyone's builds with the next 3DSrecomp+Zakuro update."
    gh issue close $Issue -R $repo
}

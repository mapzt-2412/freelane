param([Parameter(Mandatory=$true)][string]$AgentPath)
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot
$script=Join-Path $repo 'scripts/install-windows-agent.ps1'
# Refuse to touch an existing real installation; only this test's fresh directory is cleaned.
$directory=Join-Path $env:LOCALAPPDATA 'FreeLane'
$shortcut=Join-Path ([Environment]::GetFolderPath('Startup')) 'FreeLane Guest Agent.lnk'
if ((Test-Path -LiteralPath $directory) -or (Test-Path -LiteralPath $shortcut)) {throw 'Acceptance requires a fresh runner without an agent installation'}
$root=Join-Path ([IO.Path]::GetTempPath()) ('freelane-lifecycle-'+[Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $root | Out-Null
$first=Join-Path $root 'first.exe';Copy-Item -LiteralPath $AgentPath -Destination $first
$hash=(Get-FileHash -LiteralPath $first -Algorithm SHA256).Hash
function Assert($condition,$name) {if (!$condition) {throw "FAILED: $name"}}
function Refused([scriptblock]$call,$name) {$failed=$false;try {& $call | Out-Null} catch {$failed=$true};Assert $failed $name}
try {
 Refused {& $script -Action Install -AgentPath $first -ExpectedSha256 ('0'*64)} 'tampered checksum refused'
 Assert (!(Test-Path -LiteralPath (Join-Path $directory 'freelane-guest-agent.exe'))) 'invalid source never published'
 & $script -Action Install -AgentPath $first -ExpectedSha256 $hash
 $dest=Join-Path $directory 'freelane-guest-agent.exe'
 Assert ((Get-FileHash -LiteralPath $dest).Hash -eq $hash) 'verified install'
 Assert (Test-Path -LiteralPath $shortcut) 'startup shortcut'
 Refused {& $script -Action Install -AgentPath $first -ExpectedSha256 $hash} 'duplicate preserved'
 $second=Join-Path $root 'second.exe';[IO.File]::WriteAllBytes($second,([IO.File]::ReadAllBytes($first)+[byte[]](1,2,3)))
 $secondHash=(Get-FileHash -LiteralPath $second).Hash
 & $script -Action Update -AgentPath $second -ExpectedSha256 $secondHash
 Assert ((Get-FileHash -LiteralPath $dest).Hash -eq $secondHash) 'update published'
 & $script -Action Rollback
 Assert ((Get-FileHash -LiteralPath $dest).Hash -eq $hash) 'rollback restored verified old binary'
 $manifest=Join-Path $directory 'agent-install.json';$saved=[IO.File]::ReadAllText($manifest)
 $bad=$saved | ConvertFrom-Json;$bad.backup='../outside.exe';$bad | ConvertTo-Json | Set-Content -LiteralPath $manifest
 Refused {& $script -Action Rollback} 'backup traversal refused'
 [IO.File]::WriteAllText($manifest,$saved)
 [IO.File]::WriteAllBytes($dest,([IO.File]::ReadAllBytes($first)+[byte[]](9)))
 Refused {& $script -Action Update -AgentPath $second -ExpectedSha256 $secondHash} 'changed installation preserved on update'
 Refused {& $script -Action Uninstall} 'changed installation preserved on uninstall'
 [IO.File]::WriteAllBytes($dest,[IO.File]::ReadAllBytes($first))
 $unrelated=Join-Path $directory 'user-file.txt';[IO.File]::WriteAllText($unrelated,'preserve')
 & $script -Action Uninstall
 Assert (!(Test-Path -LiteralPath $dest)) 'uninstalled executable'
 Assert (!(Test-Path -LiteralPath $shortcut)) 'uninstalled shortcut'
 Assert ([IO.File]::ReadAllText($unrelated) -eq 'preserve') 'unrelated file preserved'
 Write-Output 'PASS: verified install/update/rollback/uninstall and preservation checks'
} finally {
 # Runner fixture only; production installation directories are never recursively removed by the helper.
 if (Test-Path -LiteralPath $shortcut) {Remove-Item -LiteralPath $shortcut}
 if (Test-Path -LiteralPath $directory) {Remove-Item -LiteralPath $directory -Recurse}
 Remove-Item -LiteralPath $root -Recurse
}

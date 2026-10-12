# Run as the normal interactive Windows user. No service, elevation or host changes.
param([Parameter(Mandatory=$true)][string]$AgentPath)
$ErrorActionPreference = 'Stop'
$source = (Resolve-Path -LiteralPath $AgentPath).Path
if ([IO.Path]::GetExtension($source) -ine '.exe') { throw 'Supply the built ARM64 guest-agent .exe' }
$directory = Join-Path $env:LOCALAPPDATA 'FreeLane'
$destination = Join-Path $directory 'freelane-guest-agent.exe'
$shortcut = Join-Path ([Environment]::GetFolderPath('Startup')) 'FreeLane Guest Agent.lnk'
if ((Test-Path -LiteralPath $destination) -or (Test-Path -LiteralPath $shortcut)) {
    throw 'Existing installation found. Stop the owned agent and inspect/back up its files before updating.'
}
New-Item -ItemType Directory -Path $directory -Force | Out-Null
Copy-Item -LiteralPath $source -Destination $destination
$shell = New-Object -ComObject WScript.Shell
$link = $shell.CreateShortcut($shortcut)
$link.TargetPath = $destination
$link.WorkingDirectory = $directory
$link.WindowStyle = 7
$link.Description = 'FreeLane interactive user guest agent (VM application windows)'
$link.Save()
Write-Output 'Installed a per-user Startup shortcut. Sign in again to start the agent; no elevated service was installed.'

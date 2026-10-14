# Per-user lifecycle only: no elevation, service, driver or unowned process changes.
[CmdletBinding()]
param(
 [ValidateSet('Install','Update','Rollback','Uninstall')][string]$Action='Install',
 [string]$AgentPath,
 [string]$ExpectedSha256
)
$ErrorActionPreference='Stop'
$directory=Join-Path $env:LOCALAPPDATA 'FreeLane'
$destination=Join-Path $directory 'freelane-guest-agent.exe'
$manifestPath=Join-Path $directory 'agent-install.json'
$shortcut=Join-Path ([Environment]::GetFolderPath('Startup')) 'FreeLane Guest Agent.lnk'
function Hash([string]$path) {return (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()}
function PlainFile([string]$path) {
 $item=Get-Item -LiteralPath $path -Force
 if ($item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {throw 'Agent files must be ordinary files'}
}
function Arm64([string]$path) {
 $stream=[IO.File]::OpenRead($path)
 try {
  if ($stream.Length -lt 64) {throw 'Invalid ARM64 executable'}
  $reader=New-Object IO.BinaryReader($stream)
  if ($reader.ReadUInt16() -ne 0x5a4d) {throw 'Invalid ARM64 executable'}
  $stream.Position=0x3c;$offset=$reader.ReadUInt32()
  if ($offset -gt ($stream.Length-6)) {throw 'Invalid ARM64 executable'}
  $stream.Position=$offset
  if ($reader.ReadUInt32() -ne 0x4550 -or $reader.ReadUInt16() -ne 0xaa64) {throw 'Supply the Windows ARM64 agent executable'}
 } finally {$stream.Dispose()}
}
function ReadManifest {
 PlainFile $manifestPath
 if ((Get-Item -LiteralPath $manifestPath).Length -gt 4096) {throw 'Invalid installation manifest'}
 $value=[IO.File]::ReadAllText($manifestPath) | ConvertFrom-Json
 if ($value.schema -ne 1 -or $value.sha256 -notmatch '^[0-9a-f]{64}$') {throw 'Invalid installation manifest'}
 if ($value.backup -and ($value.backup -notmatch '^agent-backup-[0-9a-f]{32}\.exe$' -or $value.backup_sha256 -notmatch '^[0-9a-f]{64}$')) {throw 'Invalid backup metadata'}
 return $value
}
function ManagedInstallation {
 $value=ReadManifest
 PlainFile $destination
 if ((Hash $destination) -ne $value.sha256) {throw 'Installed agent changed; preserve and inspect it before updating or uninstalling'}
 PlainFile $shortcut
 $link=(New-Object -ComObject WScript.Shell).CreateShortcut($shortcut)
 if ($link.TargetPath -ine $destination -or $link.Arguments) {throw 'Startup shortcut changed; preserve and inspect it'}
 # An active executable can be locked by Windows. Never terminate a process by name.
 foreach ($process in @(Get-Process -Name 'freelane-guest-agent' -ErrorAction SilentlyContinue)) {
  try {$path=$process.Path} catch {throw 'Cannot inspect running agent; stop it before continuing'}
  if (!$path -or $path -ieq $destination) {throw 'Stop the installed agent in your user session before continuing'}
 }
 return $value
}
if ($Action -in @('Install','Update')) {
 if (!$AgentPath -or $ExpectedSha256 -notmatch '^[0-9a-fA-F]{64}$') {throw 'Install/update requires AgentPath and the trusted package ExpectedSha256'}
 $source=(Resolve-Path -LiteralPath $AgentPath).Path
 PlainFile $source
 if ([IO.Path]::GetExtension($source) -ine '.exe') {throw 'Supply the ARM64 guest-agent .exe'}
 Arm64 $source
 if ((Hash $source) -ne $ExpectedSha256.ToLowerInvariant()) {throw 'Agent checksum verification failed; installation refused'}
}
if (!(Test-Path -LiteralPath $directory)) {
 if ($Action -ne 'Install') {throw 'No managed agent installation found'}
 New-Item -ItemType Directory -Path $directory | Out-Null
}
$item=Get-Item -LiteralPath $directory -Force
if (!$item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {throw 'Installation directory must be an ordinary directory'}
$lockPath=Join-Path $directory 'agent-install.lock'
if (Test-Path -LiteralPath $lockPath) {PlainFile $lockPath}
$lock=[IO.File]::Open($lockPath,[IO.FileMode]::OpenOrCreate,[IO.FileAccess]::ReadWrite,[IO.FileShare]::None)
$stage=$null;$stageManifest=$null;$stageShortcut=$null;$backup=$null;$manifestBackup=$null;$published=$false;$oldManifest=$null;$installedShortcut=$false
try {
 if ($Action -eq 'Install') {
  foreach ($path in @($destination,$shortcut,$manifestPath)) {if (Test-Path -LiteralPath $path) {throw 'Existing installation or shortcut found; no files replaced'}}
 } else {$oldManifest=ManagedInstallation}
 if ($Action -eq 'Uninstall') {
  # Preserve backup binaries and unrelated files; never recursively remove LocalAppData.
  Remove-Item -LiteralPath $shortcut
  Remove-Item -LiteralPath $destination
  Remove-Item -LiteralPath $manifestPath
  Write-Output 'Uninstalled managed agent and Startup shortcut. Backups and other user files are preserved.'
  return
 }
 if ($Action -eq 'Rollback') {
  if (!$oldManifest.backup) {throw 'No verified backup available'}
  $source=Join-Path $directory $oldManifest.backup
  PlainFile $source
  $ExpectedSha256=$oldManifest.backup_sha256
  if ((Hash $source) -ne $ExpectedSha256) {throw 'Backup checksum verification failed; rollback refused'}
  Arm64 $source
 }
 $expected=$ExpectedSha256.ToLowerInvariant()
 $id=[Guid]::NewGuid().ToString('N')
 $stage=Join-Path $directory ('agent-stage-'+$id+'.exe')
 [IO.File]::Copy($source,$stage,$false)
 PlainFile $stage;Arm64 $stage
 if ((Hash $stage) -ne $expected) {throw 'Staged checksum verification failed; installation refused'}
 $newManifest=@{schema=1;sha256=$expected;backup=$null;backup_sha256=$null}
 if ($oldManifest) {
  # Recheck immediately before replacement; File.Replace retains the actual old bytes.
  $checked=ManagedInstallation
  if ($checked.sha256 -ne $oldManifest.sha256) {throw 'Installation changed during update'}
  $backupName='agent-backup-'+$id+'.exe';$backup=Join-Path $directory $backupName
  [IO.File]::Replace($stage,$destination,$backup)
  $published=$true
  if ((Hash $backup) -ne $oldManifest.sha256) {throw 'Installation changed during replacement; restoring preserved bytes'}
  $newManifest.backup=$backupName;$newManifest.backup_sha256=$oldManifest.sha256
 } else {
  $stageShortcut=Join-Path ([IO.Path]::GetDirectoryName($shortcut)) ('FreeLane-stage-'+$id+'.lnk')
  $link=(New-Object -ComObject WScript.Shell).CreateShortcut($stageShortcut)
  $link.TargetPath=$destination;$link.WorkingDirectory=$directory;$link.WindowStyle=7
  $link.Description='FreeLane interactive user guest agent';$link.Save()
  [IO.File]::Move($stage,$destination);$published=$true
  [IO.File]::Move($stageShortcut,$shortcut);$installedShortcut=$true
 }
 $stageManifest=Join-Path $directory ('manifest-stage-'+$id+'.json')
 [IO.File]::WriteAllText($stageManifest,($newManifest | ConvertTo-Json -Compress),[Text.UTF8Encoding]::new($false))
 if ($oldManifest) {
  $manifestBackup=Join-Path $directory ('manifest-backup-'+$id+'.json')
  [IO.File]::Replace($stageManifest,$manifestPath,$manifestBackup)
 } else {[IO.File]::Move($stageManifest,$manifestPath)}
 $published=$false
 Write-Output "Agent $Action complete. Sign in again to start the per-user agent. No service or elevation was used."
} catch {
 $failure=$_
 if ($published) {
  if ((Test-Path -LiteralPath $destination) -and (Hash $destination) -eq $expected) {
   if ($backup -and (Test-Path -LiteralPath $backup)) {
    $replaced=Join-Path $directory ('agent-recovery-'+[Guid]::NewGuid().ToString('N')+'.exe')
    [IO.File]::Replace($backup,$destination,$replaced)
    if ((Hash $replaced) -eq $expected) {Remove-Item -LiteralPath $replaced}
   } else {Remove-Item -LiteralPath $destination}
   if ($installedShortcut) {Remove-Item -LiteralPath $shortcut}
  } else {throw 'Agent destination changed during failure recovery; files retained for inspection'}
 }
 throw $failure
} finally {
 foreach ($path in @($stage,$stageManifest,$stageShortcut,$manifestBackup)) {if ($path -and (Test-Path -LiteralPath $path)) {Remove-Item -LiteralPath $path}}
 $lock.Dispose()
}

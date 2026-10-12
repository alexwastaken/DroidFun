$ErrorActionPreference = 'Stop'
$bundle = Join-Path $env:CARGO_TARGET_DIR "$env:DESKBOT_TARGET/release/bundle/nsis"
$installers = @(Get-ChildItem $bundle -Filter '*.exe')
if ($installers.Count -ne 1) { throw 'Expected one Windows installer' }
$installDir = Join-Path $env:RUNNER_TEMP 'Deskbot-installed'
$installer = Start-Process -FilePath $installers[0].FullName -ArgumentList "/S /D=$installDir" -Wait -PassThru
if ($installer.ExitCode -ne 0) { throw "Installer failed: $($installer.ExitCode)" }
$binary = Join-Path $installDir 'deskbot.exe'
if (!(Test-Path $binary)) { throw 'Installer did not install the application' }
# The bundler restores the unpatched build after embedding NSIS bundle metadata.
# Use the installer payload as the installation oracle.
$payload = Join-Path $env:RUNNER_TEMP 'Deskbot-nsis-payload'
$sevenZip = Join-Path $env:ProgramFiles '7-Zip/7z.exe'
& $sevenZip x -y "-o$payload" $installers[0].FullName
if ($LASTEXITCODE -ne 0) { throw 'Could not extract installer payload for verification' }
$packaged = @(Get-ChildItem $payload -Recurse -Filter 'deskbot.exe')
if ($packaged.Count -ne 1) { throw 'Expected one application executable in installer' }
if ((Get-FileHash $binary).Hash -ne (Get-FileHash $packaged[0].FullName).Hash) { throw 'Installed binary differs from installer payload' }
$bytes = [System.IO.File]::ReadAllBytes($binary)
$peOffset = [BitConverter]::ToUInt32($bytes, 0x3c)
$machine = [BitConverter]::ToUInt16($bytes, $peOffset + 4)
$architecture = switch ($machine) { 0x8664 { 'x64' }; 0xaa64 { 'ARM64' }; default { throw 'Unexpected installed executable architecture' } }
if (!$env:DESKBOT_PLATFORM.EndsWith($architecture)) { throw 'Installed executable has wrong architecture' }
$app = Start-Process -FilePath $binary -PassThru
try {
  Start-Sleep -Seconds 15
  $app.Refresh()
  if ($app.HasExited) { throw "Installed application exited: $($app.ExitCode)" }
  if ($app.MainWindowHandle -eq 0) { throw 'Deskbot did not create a native application window' }
  $result = @{result='passed'; installedBinaryMatchesPackage=$true; installedBinaryArchitecture=$architecture; nativeWindowCreated=$true; aliveSeconds=15; installerExitCode=$installer.ExitCode; environment='Fresh GitHub Windows runner; no robot or account credentials'}
  $result | ConvertTo-Json | Set-Content -Encoding utf8 (Join-Path $env:DESKBOT_ARTIFACT_DIR 'native-smoke.json')
} finally {
  if (!$app.HasExited) { Stop-Process -Id $app.Id -Force }
}

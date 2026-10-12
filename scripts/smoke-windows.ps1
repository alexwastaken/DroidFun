$ErrorActionPreference = 'Stop'
$bundle = Join-Path $env:CARGO_TARGET_DIR "$env:DESKBOT_TARGET/release/bundle/nsis"
$installers = @(Get-ChildItem $bundle -Filter '*.exe')
if ($installers.Count -ne 1) { throw 'Expected one Windows installer' }
$installDir = Join-Path $env:RUNNER_TEMP 'Deskbot-installed'
$installer = Start-Process -FilePath $installers[0].FullName -ArgumentList "/S /D=$installDir" -Wait -PassThru
if ($installer.ExitCode -ne 0) { throw "Installer failed: $($installer.ExitCode)" }
$binary = Join-Path $installDir 'deskbot.exe'
if (!(Test-Path $binary)) { throw 'Installer did not install the application' }
$built = Join-Path $env:CARGO_TARGET_DIR "$env:DESKBOT_TARGET/release/deskbot.exe"
if ((Get-FileHash $binary).Hash -ne (Get-FileHash $built).Hash) { throw 'Installed binary differs from checked build' }
$app = Start-Process -FilePath $binary -PassThru
try {
  Start-Sleep -Seconds 15
  $app.Refresh()
  if ($app.HasExited) { throw "Installed application exited: $($app.ExitCode)" }
  if ($app.MainWindowHandle -eq 0) { throw 'Deskbot did not create a native application window' }
  $result = @{result='passed'; installedBinaryMatchesBuild=$true; nativeWindowCreated=$true; aliveSeconds=15; installerExitCode=$installer.ExitCode; environment='Fresh GitHub Windows runner; no robot or account credentials'}
  $result | ConvertTo-Json | Set-Content -Encoding utf8 (Join-Path $env:DESKBOT_ARTIFACT_DIR 'native-smoke.json')
} finally {
  if (!$app.HasExited) { Stop-Process -Id $app.Id -Force }
}

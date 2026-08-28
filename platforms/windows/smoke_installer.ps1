param(
  [Parameter(Mandatory = $true)]
  [string]$InstallerPath
)

$ErrorActionPreference = "Stop"
$installer = (Resolve-Path $InstallerPath).Path
$logDir = Join-Path $env:RUNNER_TEMP "gonnyu-installer-smoke"
$installLog = Join-Path $logDir "install.log"
$uninstallLog = Join-Path $logDir "uninstall.log"
$installedDll = Join-Path $env:ProgramFiles "GonnyuGeneralIME\GannyuTextService.dll"

New-Item -ItemType Directory -Force -Path $logDir | Out-Null

function Invoke-Burn([string[]]$Arguments) {
  $process = Start-Process -FilePath $installer -ArgumentList $Arguments -Wait -PassThru
  if ($process.ExitCode -notin 0, 3010) {
    throw "Installer exited with $($process.ExitCode)"
  }
}

Invoke-Burn @("/quiet", "/norestart", "/log", $installLog)
if (-not (Test-Path -LiteralPath $installedDll -PathType Leaf)) {
  throw "Installed text service is missing: $installedDll"
}

Invoke-Burn @("/uninstall", "/quiet", "/norestart", "/log", $uninstallLog)
if (Test-Path -LiteralPath $installedDll -PathType Leaf) {
  throw "Text service remains after uninstall: $installedDll"
}

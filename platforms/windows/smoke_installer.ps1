param(
  [Parameter(Mandatory = $true)]
  [string]$InstallerPath
)

$ErrorActionPreference = "Stop"
$installer = (Resolve-Path $InstallerPath).Path
$logDir = Join-Path $env:RUNNER_TEMP "gonnyu-installer-smoke"
$installLog = Join-Path $logDir "install.log"
$uninstallLog = Join-Path $logDir "uninstall.log"
$installedDllX64 = Join-Path $env:ProgramFiles "GonnyuGeneralIME\x64\GannyuTextService.dll"
$installedDllX86 = Join-Path $env:ProgramFiles "GonnyuGeneralIME\x86\GannyuTextService.dll"
$installedTutorial = Join-Path $env:ProgramFiles "GonnyuGeneralIME\tutorial.html"
$clsidKey = "HKCR\CLSID\{7A6B9C3E-4A1F-4D58-8B2E-9A1C7D3A2F11}\InprocServer32"

New-Item -ItemType Directory -Force -Path $logDir | Out-Null

function Invoke-Burn([string[]]$Arguments) {
  Write-Host "Burn: $($Arguments -join ' ')"
  $process = Start-Process -FilePath $installer -ArgumentList $Arguments -Wait -PassThru
  Write-Host "Burn exit code: $($process.ExitCode)"
  if ($process.ExitCode -notin 0, 3010) {
    throw "Installer exited with $($process.ExitCode)"
  }
}

function Get-PeMachine([string]$Path) {
  $stream = [System.IO.File]::OpenRead($Path)
  try {
    $reader = [System.IO.BinaryReader]::new($stream)
    $stream.Position = 0x3c
    $peOffset = $reader.ReadInt32()
    $stream.Position = $peOffset + 4
    return $reader.ReadUInt16()
  } finally {
    $stream.Dispose()
  }
}

function Assert-ComRegistration([string]$View, [string]$ExpectedDll) {
  $output = & reg.exe query $clsidKey "/reg:$View" 2>&1
  Write-Host "$View-bit COM query:`n$($output -join "`n")"
  if ($LASTEXITCODE -ne 0) {
    throw "Missing $View-bit COM registration: $clsidKey"
  }
  if (($output -join "`n").IndexOf($ExpectedDll, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
    throw "$View-bit COM registration does not point to $ExpectedDll"
  }
}

function Assert-ComRegistrationAbsent([string]$View) {
  & reg.exe query $clsidKey "/reg:$View" *> $null
  if ($LASTEXITCODE -eq 0) {
    throw "$View-bit COM registration remains after uninstall: $clsidKey"
  }
}

Invoke-Burn @("/quiet", "/norestart", "/log", $installLog)
Write-Host "x64 DLL: $installedDllX64 exists=$(Test-Path -LiteralPath $installedDllX64 -PathType Leaf)"
Write-Host "x86 DLL: $installedDllX86 exists=$(Test-Path -LiteralPath $installedDllX86 -PathType Leaf)"
Write-Host "Tutorial: $installedTutorial exists=$(Test-Path -LiteralPath $installedTutorial -PathType Leaf)"
if (-not (Test-Path -LiteralPath $installedDllX64 -PathType Leaf)) {
  throw "Installed x64 text service is missing: $installedDllX64"
}
if (-not (Test-Path -LiteralPath $installedDllX86 -PathType Leaf)) {
  throw "Installed x86 text service is missing: $installedDllX86"
}
if (-not (Test-Path -LiteralPath $installedTutorial -PathType Leaf)) {
  throw "Installed tutorial is missing: $installedTutorial"
}
if ((Get-PeMachine $installedDllX64) -ne 0x8664) {
  throw "Installed x64 text service has the wrong PE machine type"
}
if ((Get-PeMachine $installedDllX86) -ne 0x014c) {
  throw "Installed x86 text service has the wrong PE machine type"
}
Assert-ComRegistration "64" $installedDllX64
Assert-ComRegistration "32" $installedDllX86

Invoke-Burn @("/uninstall", "/quiet", "/norestart", "/log", $uninstallLog)
Write-Host "After uninstall x64 DLL exists=$(Test-Path -LiteralPath $installedDllX64 -PathType Leaf)"
Write-Host "After uninstall x86 DLL exists=$(Test-Path -LiteralPath $installedDllX86 -PathType Leaf)"
Write-Host "After uninstall tutorial exists=$(Test-Path -LiteralPath $installedTutorial -PathType Leaf)"
foreach ($view in @("64", "32")) {
  $remaining = & reg.exe query $clsidKey "/reg:$view" 2>&1
  Write-Host "After uninstall $view-bit COM query exit=${LASTEXITCODE}:`n$($remaining -join "`n")"
}
if (Test-Path -LiteralPath $installedDllX64 -PathType Leaf) {
  throw "x64 text service remains after uninstall: $installedDllX64"
}
if (Test-Path -LiteralPath $installedDllX86 -PathType Leaf) {
  throw "x86 text service remains after uninstall: $installedDllX86"
}
if (Test-Path -LiteralPath $installedTutorial -PathType Leaf) {
  throw "Tutorial remains after uninstall: $installedTutorial"
}
Assert-ComRegistrationAbsent "64"
Assert-ComRegistrationAbsent "32"

@echo off
setlocal

if "%~1"=="" (
  echo Missing installer path.
  exit /b 1
)

set "SCRIPT_DIR=%~dp0"
set "REPO_ROOT=%SCRIPT_DIR%..\.."
set "INSTALLER_PATH=%~1"
set "LOG_SCOPE=%~2"

if not defined LOG_SCOPE set "LOG_SCOPE=manual-install"

set "LOG_DIR=%REPO_ROOT%\build\windows\logs\%LOG_SCOPE%"

if not exist "%INSTALLER_PATH%" (
  echo Missing %INSTALLER_PATH%
  exit /b 1
)

if not exist "%LOG_DIR%" mkdir "%LOG_DIR%"

for /f %%i in ('powershell -NoProfile -Command "(Get-Date).ToString('yyyyMMdd-HHmmss')"') do set "STAMP=%%i"
if not defined STAMP (
  echo Failed to create log timestamp.
  exit /b 1
)

set "BURN_LOG=%LOG_DIR%\GonnyuGeneralIME-%STAMP%.log"

echo Running installer:
echo   %INSTALLER_PATH%
echo Bundle log:
echo   %BURN_LOG%

start /wait "" "%INSTALLER_PATH%" /log "%BURN_LOG%"
set "INSTALL_EXIT=%ERRORLEVEL%"
if not "%INSTALL_EXIT%"=="0" goto :install_failed

echo.
echo Installer finished.
echo Bundle log: %BURN_LOG%
echo MSI log: %LOG_DIR%\GonnyuGeneralIME-%STAMP%_000_GonnyuGeneralIME_Setup.msi.log
exit /b 0

:install_failed
echo.
echo Installer failed with exit code %INSTALL_EXIT%.
echo Bundle log: %BURN_LOG%
echo MSI log: %LOG_DIR%\GonnyuGeneralIME-%STAMP%_000_GonnyuGeneralIME_Setup.msi.log
exit /b %INSTALL_EXIT%

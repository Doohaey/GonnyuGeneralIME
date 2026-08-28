@echo off
setlocal

set "SCRIPT_DIR=%~dp0"
for %%F in ("%SCRIPT_DIR%..\..\build\downloaded-windows-installer-latest\GonnyuGeneralIME-*-windows-installer.exe") do set "EXE_PATH=%%~fF"

if not exist "%EXE_PATH%" (
  echo Missing %EXE_PATH%
  exit /b 1
)

call "%SCRIPT_DIR%run_installer_with_log.bat" "%EXE_PATH%" "downloaded-installer"
exit /b %ERRORLEVEL%

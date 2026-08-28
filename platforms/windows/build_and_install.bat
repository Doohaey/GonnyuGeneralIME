@echo off
setlocal
set "SCRIPT_DIR=%~dp0"
set "REPO_ROOT=%SCRIPT_DIR%..\.."
for /f "tokens=3" %%i in ('findstr /r /c:"^version[ ]*=" "%REPO_ROOT%\Cargo.toml"') do set "GONNYU_VERSION=%%~i"
set "EXE_PATH=%REPO_ROOT%\build\windows\installer\GonnyuGeneralIME-%GONNYU_VERSION%-windows-installer.exe"

call "%SCRIPT_DIR%build_installer.bat" || goto :err

if not exist "%EXE_PATH%" (
  echo Missing %EXE_PATH%
  goto :err
)

call "%SCRIPT_DIR%run_installer_with_log.bat" "%EXE_PATH%" "local-build-install"
if errorlevel 1 goto :err

echo.
echo Build and install OK. Open Settings ^> Time and language ^> Language and region ^> Chinese ^> Keyboard ^> Add ^> Gannyu.
goto :eof

:err
set "EXIT_CODE=%ERRORLEVEL%"
if "%EXIT_CODE%"=="" set "EXIT_CODE=1"
echo Build or install failed.
exit /b %EXIT_CODE%

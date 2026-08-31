@echo off
setlocal

set "SCRIPT_DIR=%~dp0"
set "REPO_ROOT=%SCRIPT_DIR%..\.."
set "BUILD_ROOT=%REPO_ROOT%\build\windows"
set "CMAKE_BUILD_DIR=%BUILD_ROOT%\cmake"
set "INSTALLER_DIR=%BUILD_ROOT%\installer"
set "DLL_PATH=%CMAKE_BUILD_DIR%\GannyuTextService.dll"
set "CMAKE_BUILD_TYPE=Release"
for /f "tokens=3" %%i in ('findstr /r /c:"^version[ ]*=" "%REPO_ROOT%\Cargo.toml"') do set "GONNYU_VERSION=%%~i"
if not defined GONNYU_VERSION (
  echo Missing workspace package version.
  exit /b 1
)
if not defined GANNYU_RESOURCE_KEY (
  echo Missing GANNYU_RESOURCE_KEY for release build.
  exit /b 1
)
set "MSI_PRODUCT_VERSION=%GONNYU_VERSION%"
set "BUNDLE_PRODUCT_VERSION=%GONNYU_VERSION%.0"
set "MSI_PATH=%INSTALLER_DIR%\GonnyuGeneralIME-%GONNYU_VERSION%-windows-installer.msi"
set "EXE_PATH=%INSTALLER_DIR%\GonnyuGeneralIME-%GONNYU_VERSION%-windows-installer.exe"
set "RUSTFLAGS=--remap-path-prefix=%REPO_ROOT%=. -C target-feature=+crt-static %RUSTFLAGS%"
set "RESOURCE_BUILD_ROOT=%TEMP%\GonnyuGeneralIME-resources-%RANDOM%-%RANDOM%"
set "GONNYU_RESOURCE_DIR=%RESOURCE_BUILD_ROOT%\bundle"

if exist "%USERPROFILE%\.cargo\bin" set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
if exist "%USERPROFILE%\.dotnet\tools" set "PATH=%USERPROFILE%\.dotnet\tools;%PATH%"

where cargo >nul 2>nul || (
  echo Missing cargo. Install the Rust toolchain first.
  exit /b 1
)

where cmake >nul 2>nul || (
  echo Missing cmake. Install CMake first.
  exit /b 1
)

where wix >nul 2>nul || (
  echo Missing wix. Run: dotnet tool install --global wix --version 5.0.2
  exit /b 1
)

set "WIX_VERSION="
for /f %%i in ('wix --version') do if not defined WIX_VERSION set "WIX_VERSION=%%i"
if "%WIX_VERSION:~0,2%"=="7." (
  echo Installed wix %WIX_VERSION% requires an extra EULA. Install wix 5.0.2 instead.
  exit /b 1
)

wix extension add WixToolset.BootstrapperApplications.wixext/5.0.2 >nul 2>nul

call :ensure_vs || exit /b 1

pushd "%REPO_ROOT%"
mkdir "%RESOURCE_BUILD_ROOT%" || goto :err
cargo run -p gonnyu-resource-build --release -- "%REPO_ROOT%\resources" "%GONNYU_RESOURCE_DIR%" || goto :err
cargo build --release -p gannyu-input-ffi || goto :err
popd

if not exist "%CMAKE_BUILD_DIR%" mkdir "%CMAKE_BUILD_DIR%"
if not exist "%INSTALLER_DIR%" mkdir "%INSTALLER_DIR%"

cmake -S "%SCRIPT_DIR%GannyuTextService" -B "%CMAKE_BUILD_DIR%" -G "NMake Makefiles" -DCMAKE_BUILD_TYPE=%CMAKE_BUILD_TYPE% || goto :err
cmake --build "%CMAKE_BUILD_DIR%" --config Release || goto :err

if not exist "%DLL_PATH%" if exist "%CMAKE_BUILD_DIR%\Release\GannyuTextService.dll" set "DLL_PATH=%CMAKE_BUILD_DIR%\Release\GannyuTextService.dll"

if not exist "%DLL_PATH%" (
  echo Missing %DLL_PATH%
  goto :err
)

call :assert_release_dll "%DLL_PATH%" || goto :err

rem Sanitize the DLL: strip symbol/debug sections and scrub panic-location
rem strings that leak source paths and the Rust module structure. The .def file
rem already restricts the PE export table to the COM entry points. If
rem llvm-objcopy is available (e.g. from the Android NDK or LLVM), also strip
rem the COFF symbol table; otherwise fall back to panic-string scrubbing only.
set "LLVM_OBJCOPY="
for /f "delims=" %%i in ('where llvm-objcopy 2^>nul') do if not defined LLVM_OBJCOPY set "LLVM_OBJCOPY=%%i"
if defined LLVM_OBJCOPY (
    cargo run -p gannyu-sanitize-binary --release -- "%DLL_PATH%" --strip-tool "%LLVM_OBJCOPY%"
) else (
    cargo run -p gannyu-sanitize-binary --release -- "%DLL_PATH%" --no-strip
)
if errorlevel 1 goto :err

set "BA_WIXEXT=%REPO_ROOT%\.wix\extensions\WixToolset.BootstrapperApplications.wixext\5.0.2\wixext5\WixToolset.BootstrapperApplications.wixext.dll"
if not exist "%BA_WIXEXT%" set "BA_WIXEXT=%REPO_ROOT%\.wix\extensions\WixToolset.Bal.wixext\5.0.2\wixext5\WixToolset.BootstrapperApplications.wixext.dll"
if not exist "%BA_WIXEXT%" (
  echo Missing bootstrapper extension DLL: %BA_WIXEXT%
  goto :err
)

wix build "%SCRIPT_DIR%Installer.wxs" -arch x64 -d "DllPath=%DLL_PATH%" -d "TutorialPath=%REPO_ROOT%\resources\tutorial\tutorial.html" -d "GonnyuProductVersion=%MSI_PRODUCT_VERSION%" -o "%MSI_PATH%" || goto :err
wix build "%SCRIPT_DIR%InstallerBundle.wxs" -arch x64 -ext "%BA_WIXEXT%" -d "MsiPath=%MSI_PATH%" -d "GonnyuBundleVersion=%BUNDLE_PRODUCT_VERSION%" -o "%EXE_PATH%" || goto :err

echo.
echo Installer ready: %EXE_PATH%
if exist "%RESOURCE_BUILD_ROOT%" rmdir /s /q "%RESOURCE_BUILD_ROOT%"
exit /b 0

:assert_release_dll
set "CHECK_DLL=%~1"
powershell -NoProfile -ExecutionPolicy Bypass -Command ^
  "$dll = $env:CHECK_DLL;" ^
  "$bad = @('MSVCP140D.dll', 'VCRUNTIME140D.dll', 'VCRUNTIME140_1D.dll', 'ucrtbased.dll', 'MSVCP140.dll', 'VCRUNTIME140.dll', 'VCRUNTIME140_1.dll');" ^
  "$text = [System.Text.Encoding]::ASCII.GetString([System.IO.File]::ReadAllBytes($dll));" ^
  "$hits = $bad | Where-Object { $text.Contains($_) };" ^
  "if ($hits) { Write-Host ('External VC runtime dependency found in ' + $dll + ': ' + ($hits -join ', ')); exit 1 }"
if errorlevel 1 exit /b 1
exit /b 0

:ensure_vs
if defined VCINSTALLDIR exit /b 0
set "VSWHERE=%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe"
if not exist "%VSWHERE%" (
  echo Missing Visual Studio 2022 Build Tools or the Desktop C++ workload.
  exit /b 1
)
set "VSROOT="
for /f "usebackq delims=" %%i in (`"%VSWHERE%" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do if not defined VSROOT set "VSROOT=%%i"
if not defined VSROOT (
  echo Missing Visual Studio 2022 Build Tools or the Desktop C++ workload.
  exit /b 1
)
call "%VSROOT%\Common7\Tools\VsDevCmd.bat" -arch=x64 >nul
if errorlevel 1 (
  echo Failed to load the Visual Studio build environment.
  exit /b 1
)
exit /b 0

:err
if exist "%RESOURCE_BUILD_ROOT%" rmdir /s /q "%RESOURCE_BUILD_ROOT%"
echo Build failed.
exit /b 1

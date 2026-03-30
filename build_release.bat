@echo off
setlocal enabledelayedexpansion

set VERSION=v0.2.2
set RELEASE_DIR=releases
set BUNDLE_NAME=ac_pro_engineer_%VERSION%
set BUNDLE_DIR=%RELEASE_DIR%\%BUNDLE_NAME%
set WIN_DIR=%BUNDLE_DIR%\Windows
set LIN_DIR=%BUNDLE_DIR%\Linux

echo ==========================================
echo AC Pro Engineer All-in-One Builder %VERSION%
echo ==========================================

echo.
echo [1/5] Cleaning old builds...
if exist "%RELEASE_DIR%" rmdir /s /q "%RELEASE_DIR%"
mkdir "%WIN_DIR%"
mkdir "%LIN_DIR%"

echo.
echo [2/5] Building Windows TUI...
cargo build -p ac_tui --release
if %errorlevel% neq 0 (
    echo ERROR: Windows build failed!
    exit /b %errorlevel%
)

echo.
echo [3/5] Building shm-bridge (Windows)...
cargo build -p shm-bridge --release
if %errorlevel% neq 0 (
    echo ERROR: shm-bridge build failed!
    exit /b %errorlevel%
)

echo.
echo [4/5] Building Linux TUI (zigbuild)...
cargo zigbuild -p ac_tui --target x86_64-unknown-linux-gnu --release
if %errorlevel% neq 0 (
    echo ERROR: Linux build failed!
    exit /b %errorlevel%
)

echo.
echo [5/5] Packaging files...

if exist "target\release\ac_pro_engineer.exe" (
    copy /Y "target\release\ac_pro_engineer.exe" "%WIN_DIR%\" >nul
    echo   - Windows binary copied.
) else (
    echo ERROR: Windows binary not found!
    exit /b 1
)

if exist "target\x86_64-unknown-linux-gnu\release\ac_pro_engineer" (
    copy /Y "target\x86_64-unknown-linux-gnu\release\ac_pro_engineer" "%LIN_DIR%\" >nul
    echo   - Linux binary copied.
) else (
    echo ERROR: Linux binary not found!
    exit /b 1
)

if exist "target\release\shm-bridge.exe" (
    copy /Y "target\release\shm-bridge.exe" "%LIN_DIR%\" >nul
    echo   - shm-bridge.exe copied to Linux folder.
) else (
    echo ERROR: shm-bridge.exe not found!
    exit /b 1
)

if exist "README.txt" (
    copy /Y "README.txt" "%BUNDLE_DIR%\" >nul
    copy /Y "README.txt" "%WIN_DIR%\" >nul
    copy /Y "README.txt" "%LIN_DIR%\" >nul
    echo   - README.txt copied to all folders.
) else (
    echo WARNING: README.txt not found in the root directory!
)

echo.
echo Zipping All-in-One release...
pushd "%RELEASE_DIR%"
tar -a -c -f "%BUNDLE_NAME%.zip" "%BUNDLE_NAME%"
popd

echo.
echo DONE! Release archive is ready in the '%RELEASE_DIR%' folder.
echo ==========================================
pause
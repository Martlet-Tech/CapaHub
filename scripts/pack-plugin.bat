@echo off
setlocal enabledelayedexpansion

set SCRIPTS_DIR=%~dp0
set PROJECT_DIR=%SCRIPTS_DIR%..
pushd "%PROJECT_DIR%"

set PLUGIN_NAME=%1
if "%PLUGIN_NAME%"=="" set PLUGIN_NAME=template

echo Building plugin: %PLUGIN_NAME%
cargo build --release -p %PLUGIN_NAME%
if %ERRORLEVEL% neq 0 (
    echo Build failed!
    popd
    exit /b 1
)

set TEMP_DIR=%TEMP%\CapaHub\%PLUGIN_NAME%
if exist "%TEMP_DIR%" rmdir /S /Q "%TEMP_DIR%"
mkdir "%TEMP_DIR%\assets"

copy /Y "target\release\%PLUGIN_NAME%.dll" "%TEMP_DIR%\%PLUGIN_NAME%.dll" >nul
copy /Y "plugins\%PLUGIN_NAME%\plugin.toml" "%TEMP_DIR%\plugin.toml" >nul

if not exist "plugins\%PLUGIN_NAME%\index.html" (
    echo ^<html^>^<body style="font-family:sans-serif;margin:40px"^>^<h1^>%PLUGIN_NAME%^</h1^>^<p^>Plugin v0.1.0^</p^>^</body^>^</html^> > "%TEMP_DIR%\index.html"
) else (
    copy /Y "plugins\%PLUGIN_NAME%\index.html" "%TEMP_DIR%\index.html" >nul
)
if exist "plugins\%PLUGIN_NAME%\assets\icon.png" (
    copy /Y "plugins\%PLUGIN_NAME%\assets\icon.png" "%TEMP_DIR%\assets\icon.png" >nul
)

set DAP_FILE=%TEMP%\CapaHub\%PLUGIN_NAME%-0.1.0.dap

:: Generate PowerShell script
set PS_FILE=%TEMP%\capapack.ps1
echo $src = '%TEMP_DIR%' > "%PS_FILE%"
echo $dst = '%DAP_FILE%' >> "%PS_FILE%"
echo Add-Type -AssemblyName System.IO.Compression.FileSystem >> "%PS_FILE%"
echo if (Test-Path $dst) { Remove-Item $dst -Force } >> "%PS_FILE%"
echo [System.IO.Compression.ZipFile]::CreateFromDirectory($src, $dst) >> "%PS_FILE%"
echo Write-Host ('Created: ' + $dst) >> "%PS_FILE%"

powershell -ExecutionPolicy Bypass -File "%PS_FILE%"
del "%PS_FILE%" >nul 2>&1

echo.
echo Test steps:
echo   1. rmdir /S /Q "%LOCALAPPDATA%\CapaHub\plugins"
echo   2. scripts\run.bat
echo   3. Tray ^> Plugin Manager ^> Install DAP
echo   4. Select: %DAP_FILE%

popd

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

set PLUGIN_DIR=%LOCALAPPDATA%\CapaHub\plugins\%PLUGIN_NAME%
if not exist "%PLUGIN_DIR%" mkdir "%PLUGIN_DIR%"

copy /Y "target\release\%PLUGIN_NAME%.dll" "%PLUGIN_DIR%\%PLUGIN_NAME%.dll"

if exist "plugins\%PLUGIN_NAME%\plugin.toml" (
    copy /Y "plugins\%PLUGIN_NAME%\plugin.toml" "%PLUGIN_DIR%\plugin.toml"
)

echo Deployed %PLUGIN_NAME% to %PLUGIN_DIR%
popd

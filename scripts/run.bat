@echo off
set SCRIPTS_DIR=%~dp0
set PROJECT_DIR=%SCRIPTS_DIR%..
pushd "%PROJECT_DIR%"
cargo run -p desktop
popd

@echo off
setlocal

set "SCRIPT_DIR=%~dp0"
pushd "%SCRIPT_DIR%" || exit /b 1

call npm run tauri:build
set "EXIT_CODE=%ERRORLEVEL%"

popd
exit /b %EXIT_CODE%

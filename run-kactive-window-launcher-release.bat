@echo off
setlocal

set "SCRIPT_DIR=%~dp0"
set "APP_EXE=%SCRIPT_DIR%src-tauri\target\release\app.exe"

pushd "%SCRIPT_DIR%" || exit /b 1

if not exist "%APP_EXE%" (
  call build-kactive-window-launcher-release.bat
  if errorlevel 1 (
    set "EXIT_CODE=%ERRORLEVEL%"
    popd
    exit /b %EXIT_CODE%
  )
)

if not exist "%APP_EXE%" (
  echo Release executable was not found: "%APP_EXE%"
  popd
  exit /b 1
)

start "" "%APP_EXE%"
set "EXIT_CODE=%ERRORLEVEL%"

popd
exit /b %EXIT_CODE%

@echo off
setlocal

REM Build the Plinth installer. Requires the normal system toolchain to be
REM on PATH: the .NET SDK (Fable), Rust (Tauri backend), and Node.
REM Nothing here reaches into another application's private storage.

cd /d "%~dp0"

set "MISSING="

where dotnet >nul 2>&1 || set "MISSING=%MISSING% dotnet"
where cargo  >nul 2>&1 || set "MISSING=%MISSING% cargo"
where npm    >nul 2>&1 || set "MISSING=%MISSING% npm"

if defined MISSING (
  echo.
  echo Missing from PATH:%MISSING%
  echo.
  echo Install as normal system tools, then open a NEW terminal:
  echo   .NET SDK 8   https://dotnet.microsoft.com/download/dotnet/8.0
  echo   Rust         https://rustup.rs
  echo   Node LTS     https://nodejs.org
  echo.
  exit /b 1
)

for /f "delims=" %%v in ('dotnet --version') do echo Using .NET %%v
for /f "delims=" %%v in ('cargo --version') do echo Using %%v

call npm run tauri build

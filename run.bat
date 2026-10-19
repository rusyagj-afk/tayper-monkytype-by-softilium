@echo off
REM softilium — запуск одной командой на Windows
REM Использование:
REM   run.bat --wpm 200 --lang ru
REM   run.bat                        (интерактивный режим)

set "DIR=%~dp0"
set "BIN=%DIR%target\release\softilium.exe"

if not exist "%BIN%" (
    echo [softilium] Компиляция...
    cargo build --release --manifest-path="%DIR%Cargo.toml"
)

"%BIN%" %*

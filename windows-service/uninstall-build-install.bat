@echo off

cd /d "%~dp0\.."

call windows-service\uninstall.bat

echo Building....
cargo build --release --features windows-service
if %errorlevel% neq 0 exit /b %errorlevel%

call windows-service\install.bat

pause
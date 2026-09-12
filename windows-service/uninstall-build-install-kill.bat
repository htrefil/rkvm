@echo off

cd /d "%~dp0\.."

call windows-service\uninstall-build-install.bat

call windows-service\uninstall.bat

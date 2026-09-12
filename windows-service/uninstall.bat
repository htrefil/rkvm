@echo off

set SERVICE_NAME=RkvmService
set BASE_PATH=C:\ProgramData\rkvm
set SERVICE_PATH=%BASE_PATH%\rkvm-service.exe

cd /d "%~dp0\.."

echo Stopping service...
sc stop "%SERVICE_NAME%"
timeout /t 2 /nobreak > nul

echo Deleting service...
sc delete "%SERVICE_NAME%"

timeout /t 2 /nobreak > nul
taskkill /IM rkvm-service.exe /IM rkvm-client.exe /F /T

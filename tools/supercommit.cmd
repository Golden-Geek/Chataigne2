@echo off
setlocal

powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0supercommit.ps1" %*
exit /b %errorlevel%

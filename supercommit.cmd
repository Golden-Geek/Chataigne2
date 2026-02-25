@echo off
setlocal

powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0tools\supercommit.ps1" %*
exit /b %errorlevel%

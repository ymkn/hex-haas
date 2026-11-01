@echo off
pwsh.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\start-haas.ps1"
if errorlevel 1 pause

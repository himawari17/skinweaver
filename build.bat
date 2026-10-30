@echo off
rem Double-click: builds the release exe into dist\ and keeps the window open.
cd /d "%~dp0"
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0build.ps1"
pause

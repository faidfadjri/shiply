@echo off
setlocal
cd /d "%~dp0"
title Shiply Installer

powershell.exe -NoProfile -ExecutionPolicy Bypass -Command ^
    "$ScriptDir = '%~dp0'; " ^
    "$candidates = @('Shiply-Setup.exe', 'setup.exe', 'shiply-windows-x86_64.exe', 'shiply.exe'); " ^
    "$found = $candidates | ForEach-Object { Join-Path $ScriptDir $_ } | Where-Object { Test-Path $_ } | Select-Object -First 1; " ^
    "if ($found) { " ^
    "    Unblock-File $found -ErrorAction SilentlyContinue; " ^
    "    Start-Process -FilePath $found -ArgumentList '--setup'; " ^
    "} else { " ^
    "    Write-Host 'Downloading Shiply Installer...' -ForegroundColor Cyan; " ^
    "    $tmp = Join-Path $env:TEMP 'Shiply-Setup.exe'; " ^
    "    Invoke-WebRequest -Uri 'https://github.com/faidfadjri/shiply/releases/latest/download/Shiply-Setup.exe' -OutFile $tmp -UseBasicParsing; " ^
    "    Unblock-File $tmp -ErrorAction SilentlyContinue; " ^
    "    Start-Process -FilePath $tmp -ArgumentList '--setup'; " ^
    "}"

# Shiply Windows Desktop Installer Script
# Installs Shiply to %LOCALAPPDATA%\Programs\Shiply\shiply.exe and creates
# Desktop and Start Menu shortcuts with logo.ico.

$ErrorActionPreference = 'Stop'
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$RepoRoot = Split-Path -Parent $ScriptDir

$SetupExe = Join-Path $ScriptDir "setup.exe"
$ShiplyExe = Join-Path $ScriptDir "shiply.exe"

if (Test-Path $SetupExe) {
    Write-Host "Running Shiply Windows Installer ($SetupExe --install)..." -ForegroundColor Cyan
    & $SetupExe --install
} elseif (Test-Path $ShiplyExe) {
    Write-Host "Running Shiply Windows Installer ($ShiplyExe --install)..." -ForegroundColor Cyan
    & $ShiplyExe --install
} else {
    Write-Host "Building and installing Shiply from source..." -ForegroundColor Cyan
    Push-Location $RepoRoot
    try {
        & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $RepoRoot "install.ps1")
    } finally {
        Pop-Location
    }
}

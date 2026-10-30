# Shiply Windows Desktop Installer Script
# Installs Shiply to %LOCALAPPDATA%\Programs\Shiply\shiply.exe and creates
# Desktop and Start Menu shortcuts with logo.ico.

$ErrorActionPreference = 'Stop'
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$RepoRoot = Split-Path -Parent $ScriptDir
$GithubRepo = "faidfadjri/shiply"

$ShiplySetupExe = Join-Path $ScriptDir "Shiply-Setup.exe"
$SetupExe = Join-Path $ScriptDir "setup.exe"
$ShiplyExe = Join-Path $ScriptDir "shiply.exe"

if (Test-Path $ShiplySetupExe) {
    Unblock-File $ShiplySetupExe -ErrorAction SilentlyContinue
    Write-Host "Running local Shiply Windows Installer ($ShiplySetupExe --install)..." -ForegroundColor Cyan
    & $ShiplySetupExe --install
} elseif (Test-Path $SetupExe) {
    Unblock-File $SetupExe -ErrorAction SilentlyContinue
    Write-Host "Running local Shiply Windows Installer ($SetupExe --install)..." -ForegroundColor Cyan
    & $SetupExe --install
} elseif (Test-Path $ShiplyExe) {
    Unblock-File $ShiplyExe -ErrorAction SilentlyContinue
    Write-Host "Running local Shiply Windows Installer ($ShiplyExe --install)..." -ForegroundColor Cyan
    & $ShiplyExe --install
} elseif ((Get-Command "cargo" -ErrorAction SilentlyContinue) -and (Test-Path (Join-Path $RepoRoot "Cargo.toml"))) {
    Write-Host "Building and installing Shiply from source with Cargo..." -ForegroundColor Cyan
    Push-Location $RepoRoot
    try {
        & cargo build --release
        if ($LASTEXITCODE -ne 0) {
            throw "Cargo build failed with exit code $LASTEXITCODE."
        }
        $BuiltExe = Join-Path $RepoRoot "target\release\shiply.exe"
        Unblock-File $BuiltExe -ErrorAction SilentlyContinue
        Write-Host "Running built installer ($BuiltExe --install)..." -ForegroundColor Cyan
        & $BuiltExe --install
    } finally {
        Pop-Location
    }
} else {
    Write-Host "Downloading latest Shiply Installer from GitHub Releases..." -ForegroundColor Cyan
    $DownloadUrl = "https://github.com/$GithubRepo/releases/latest/download/Shiply-Setup.exe"
    $TempInstaller = Join-Path $env:TEMP "Shiply-Setup-$([System.Guid]::NewGuid().ToString('N')).exe"
    try {
        Write-Host "Downloading $DownloadUrl..." -ForegroundColor Gray
        [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
        Invoke-WebRequest -Uri $DownloadUrl -OutFile $TempInstaller -UseBasicParsing
        Unblock-File $TempInstaller -ErrorAction SilentlyContinue
        Write-Host "Running downloaded installer ($TempInstaller --install)..." -ForegroundColor Cyan
        & $TempInstaller --install
    } finally {
        if (Test-Path $TempInstaller) {
            Remove-Item $TempInstaller -Force -ErrorAction SilentlyContinue
        }
    }
}

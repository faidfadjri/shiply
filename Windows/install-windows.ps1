# Shiply Windows Desktop Installer Script
# Installs Shiply to %LOCALAPPDATA%\Programs\Shiply\shiply.exe and creates
# Desktop and Start Menu shortcuts with logo.ico.

$ErrorActionPreference = 'Stop'
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$RepoRoot = Split-Path -Parent $ScriptDir
$GithubRepo = "faidfadjri/shiply"

$SetupExe = Join-Path $ScriptDir "setup.exe"
$ShiplyExe = Join-Path $ScriptDir "shiply.exe"

if (Test-Path $SetupExe) {
    Write-Host "Running local Shiply Windows Installer ($SetupExe --install)..." -ForegroundColor Cyan
    & $SetupExe --install
} elseif (Test-Path $ShiplyExe) {
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
        Write-Host "Running built installer ($BuiltExe --install)..." -ForegroundColor Cyan
        & $BuiltExe --install
    } finally {
        Pop-Location
    }
} else {
    Write-Host "Cargo not found; downloading latest Shiply binary from GitHub Releases..." -ForegroundColor Cyan
    $DownloadUrl = "https://github.com/$GithubRepo/releases/latest/download/shiply-windows-x86_64.exe"
    $TempInstaller = Join-Path $env:TEMP "shiply-installer-$([System.Guid]::NewGuid().ToString('N')).exe"
    try {
        Write-Host "Downloading $DownloadUrl..." -ForegroundColor Gray
        [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
        Invoke-WebRequest -Uri $DownloadUrl -OutFile $TempInstaller -UseBasicParsing
        Write-Host "Running downloaded installer ($TempInstaller --install)..." -ForegroundColor Cyan
        & $TempInstaller --install
    } finally {
        if (Test-Path $TempInstaller) {
            Remove-Item $TempInstaller -Force -ErrorAction SilentlyContinue
        }
    }
}

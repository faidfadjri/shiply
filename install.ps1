# Shiply Universal Windows Installer Script
# Builds Shiply from source (or downloads the latest release from GitHub if Cargo is absent)
# and installs/updates Shiply in %LOCALAPPDATA%\Programs\Shiply\shiply.exe with Desktop/Start Menu shortcuts.

$ErrorActionPreference = 'Stop'
$GithubRepo = "faidfadjri/shiply"

if ($PSScriptRoot) {
    Set-Location -LiteralPath $PSScriptRoot
}

if ((Get-Command "cargo" -ErrorAction SilentlyContinue) -and (Test-Path "Cargo.toml")) {
    Write-Host "[1/2] Building Shiply release binary (cargo build --release)..." -ForegroundColor Cyan
    cargo build --release
    if ($LASTEXITCODE -ne 0) {
        throw "Cargo release build failed (exit code $LASTEXITCODE)."
    }

    $InstallerExe = "./target/release/shiply.exe"
    Write-Host "[2/2] Running self-installer ($InstallerExe --install)..." -ForegroundColor Cyan
    & $InstallerExe --install
    if ($LASTEXITCODE -ne 0) {
        throw "Installation failed (exit code $LASTEXITCODE)."
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

Write-Host "Successfully installed/updated Shiply!" -ForegroundColor Green
Write-Host "  Installed path: $env:LOCALAPPDATA\Programs\Shiply\shiply.exe" -ForegroundColor Gray

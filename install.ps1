# Builds Shiply in release mode, places setup.exe and shiply.exe in ./Windows/, ./dist/, and ./setup.exe,
# syncs logo.ico into ./Windows/, ./Linux/, and ./Mac/, and installs or updates Shiply in
# %LOCALAPPDATA%\Programs\Shiply\shiply.exe along with Desktop and Start Menu shortcuts.

$ErrorActionPreference = 'Stop'

if ($PSScriptRoot) {
    Set-Location -LiteralPath $PSScriptRoot
}

Write-Host "[1/3] Building Shiply release binary (cargo build --release)..." -ForegroundColor Cyan
cargo build --release
if ($LASTEXITCODE -ne 0) {
    throw "Cargo release build failed (exit code $LASTEXITCODE)."
}

Write-Host "[2/3] Populating platform installer directories (./Windows/, ./Linux/, ./Mac/, ./dist/)..." -ForegroundColor Cyan
foreach ($dir in @("./Windows", "./Linux", "./Mac", "./dist")) {
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
}

foreach ($target in @(
    "./Windows/setup.exe",
    "./Windows/shiply.exe",
    "./dist/shiply.exe",
    "./dist/setup.exe",
    "./setup.exe"
)) {
    $old = "$target.old"
    if (Test-Path $old) {
        Remove-Item -Path $old -Force -ErrorAction SilentlyContinue
    }
    if (Test-Path $target) {
        Move-Item -Path $target -Destination $old -Force -ErrorAction SilentlyContinue
    }
    Copy-Item -Path "./target/release/shiply.exe" -Destination $target -Force
    if (Test-Path $old) {
        Remove-Item -Path $old -Force -ErrorAction SilentlyContinue
    }
}

if (Test-Path "./logo.ico") {
    Copy-Item -Path "./logo.ico" -Destination "./Windows/logo.ico" -Force
    Copy-Item -Path "./logo.ico" -Destination "./Linux/logo.ico" -Force
    Copy-Item -Path "./logo.ico" -Destination "./Mac/logo.ico" -Force
}

Write-Host "[3/3] Running self-installer (./target/release/shiply.exe --install)..." -ForegroundColor Cyan
& "./target/release/shiply.exe" --install
if ($LASTEXITCODE -ne 0) {
    throw "Installation failed (exit code $LASTEXITCODE)."
}

Write-Host "Successfully built and installed/updated Shiply!" -ForegroundColor Green
Write-Host "  Windows Installer: $(Resolve-Path './Windows/setup.exe')" -ForegroundColor Gray
Write-Host "  Linux Installer:   $(Resolve-Path './Linux/install-linux.sh')" -ForegroundColor Gray
Write-Host "  Mac Installer:     $(Resolve-Path './Mac/install-mac.command')" -ForegroundColor Gray
Write-Host "  Installed path:    $env:LOCALAPPDATA\Programs\Shiply\shiply.exe" -ForegroundColor Gray

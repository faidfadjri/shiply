# Builds Shiply in release mode, places shiply.exe in ./dist/shiply.exe,
# and installs or updates Shiply in %LOCALAPPDATA%\Programs\Shiply\shiply.exe
# along with Desktop and Start Menu shortcuts.

$ErrorActionPreference = 'Stop'

if ($PSScriptRoot) {
    Set-Location -LiteralPath $PSScriptRoot
}

Write-Host "[1/3] Building Shiply release binary (cargo build --release)..." -ForegroundColor Cyan
cargo build --release
if ($LASTEXITCODE -ne 0) {
    throw "Cargo release build failed (exit code $LASTEXITCODE)."
}

Write-Host "[2/3] Copying standalone installer/executable to ./dist/shiply.exe..." -ForegroundColor Cyan
New-Item -ItemType Directory -Force -Path "./dist" | Out-Null
Copy-Item -Path "./target/release/shiply.exe" -Destination "./dist/shiply.exe" -Force

Write-Host "[3/3] Running self-installer (./dist/shiply.exe --install)..." -ForegroundColor Cyan
& "./dist/shiply.exe" --install
if ($LASTEXITCODE -ne 0) {
    throw "Installation failed (exit code $LASTEXITCODE)."
}

Write-Host "Successfully built and installed/updated Shiply!" -ForegroundColor Green
Write-Host "  Standalone .exe: $(Resolve-Path './dist/shiply.exe')" -ForegroundColor Gray
Write-Host "  Installed path:  $env:LOCALAPPDATA\Programs\Shiply\shiply.exe" -ForegroundColor Gray

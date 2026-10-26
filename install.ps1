# Builds Shiply in release mode, places both setup.exe and shiply.exe in ./dist/ (and ./setup.exe),
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

Write-Host "[2/3] Generating setup.exe and standalone shiply.exe in ./dist/..." -ForegroundColor Cyan
New-Item -ItemType Directory -Force -Path "./dist" | Out-Null

foreach ($target in @("./dist/shiply.exe", "./dist/setup.exe", "./setup.exe")) {
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

Write-Host "[3/3] Running self-installer (./target/release/shiply.exe --install)..." -ForegroundColor Cyan
& "./target/release/shiply.exe" --install
if ($LASTEXITCODE -ne 0) {
    throw "Installation failed (exit code $LASTEXITCODE)."
}

Write-Host "Successfully built and installed/updated Shiply!" -ForegroundColor Green
Write-Host "  Desktop Installer: $(Resolve-Path './setup.exe')" -ForegroundColor Gray
Write-Host "  Dist Installer:    $(Resolve-Path './dist/setup.exe')" -ForegroundColor Gray
Write-Host "  Standalone .exe:   $(Resolve-Path './dist/shiply.exe')" -ForegroundColor Gray
Write-Host "  Installed path:    $env:LOCALAPPDATA\Programs\Shiply\shiply.exe" -ForegroundColor Gray

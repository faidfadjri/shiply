#!/usr/bin/env bash
# Shiply macOS Application Bundle Installer (.command / .sh)
# Builds or copies Shiply into ~/Applications/Shiply.app (or /Applications/Shiply.app)
# with Info.plist and app logo.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

if [[ -w "/Applications" ]]; then
    APP_DIR="/Applications/Shiply.app"
else
    mkdir -p "$HOME/Applications"
    APP_DIR="$HOME/Applications/Shiply.app"
fi

CONTENTS_DIR="$APP_DIR/Contents"
MACOS_DIR="$CONTENTS_DIR/MacOS"
RESOURCES_DIR="$CONTENTS_DIR/Resources"

mkdir -p "$MACOS_DIR" "$RESOURCES_DIR"

if [[ -x "$SCRIPT_DIR/shiply" ]]; then
    echo "[1/3] Copying prebuilt Shiply binary..."
    cp -f "$SCRIPT_DIR/shiply" "$MACOS_DIR/shiply"
else
    echo "[1/3] Building Shiply release binary (cargo build --release)..."
    (cd "$REPO_ROOT" && cargo build --release)
    cp -f "$REPO_ROOT/target/release/shiply" "$MACOS_DIR/shiply"
    cp -f "$REPO_ROOT/target/release/shiply" "$SCRIPT_DIR/shiply"
fi

chmod +x "$MACOS_DIR/shiply"

echo "[2/3] Installing app logo..."
if [[ -f "$SCRIPT_DIR/logo.ico" ]]; then
    cp -f "$SCRIPT_DIR/logo.ico" "$RESOURCES_DIR/logo.ico"
elif [[ -f "$REPO_ROOT/logo.ico" ]]; then
    cp -f "$REPO_ROOT/logo.ico" "$RESOURCES_DIR/logo.ico"
fi

echo "[3/3] Writing macOS Info.plist..."
cat > "$CONTENTS_DIR/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDisplayName</key>
    <string>Shiply</string>
    <key>CFBundleExecutable</key>
    <string>shiply</string>
    <key>CFBundleIconFile</key>
    <string>logo.ico</string>
    <key>CFBundleIdentifier</key>
    <string>io.shiply.desktop</string>
    <key>CFBundleName</key>
    <string>Shiply</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>0.2.0</string>
    <key>CFBundleVersion</key>
    <string>0.2.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
EOF

echo "Shiply installed to: $APP_DIR"

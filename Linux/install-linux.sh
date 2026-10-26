#!/usr/bin/env bash
# Shiply Linux Desktop Installer
# Installs the Shiply binary to ~/.local/bin/shiply, installs the app icon,
# and creates a desktop entry at ~/.local/share/applications/shiply.desktop.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

INSTALL_BIN_DIR="$HOME/.local/bin"
INSTALL_SHARE_DIR="$HOME/.local/share/shiply"
APPLICATIONS_DIR="$HOME/.local/share/applications"

mkdir -p "$INSTALL_BIN_DIR" "$INSTALL_SHARE_DIR" "$APPLICATIONS_DIR"

if [[ -x "$SCRIPT_DIR/shiply" ]]; then
    echo "[1/3] Installing prebuilt Shiply binary from $SCRIPT_DIR/shiply..."
    cp -f "$SCRIPT_DIR/shiply" "$INSTALL_BIN_DIR/shiply"
else
    echo "[1/3] Building Shiply release binary with Cargo..."
    (cd "$REPO_ROOT" && cargo build --release)
    cp -f "$REPO_ROOT/target/release/shiply" "$INSTALL_BIN_DIR/shiply"
    cp -f "$REPO_ROOT/target/release/shiply" "$SCRIPT_DIR/shiply"
fi

chmod +x "$INSTALL_BIN_DIR/shiply"

echo "[2/3] Installing Shiply logo icon..."
if [[ -f "$REPO_ROOT/logo.png" ]]; then
    cp -f "$REPO_ROOT/logo.png" "$INSTALL_SHARE_DIR/logo.png"
    ICON_PATH="$INSTALL_SHARE_DIR/logo.png"
elif [[ -f "$SCRIPT_DIR/logo.ico" ]]; then
    cp -f "$SCRIPT_DIR/logo.ico" "$INSTALL_SHARE_DIR/logo.ico"
    ICON_PATH="$INSTALL_SHARE_DIR/logo.ico"
elif [[ -f "$REPO_ROOT/logo.ico" ]]; then
    cp -f "$REPO_ROOT/logo.ico" "$INSTALL_SHARE_DIR/logo.ico"
    ICON_PATH="$INSTALL_SHARE_DIR/logo.ico"
else
    ICON_PATH="utilities-terminal"
fi

echo "[3/3] Creating Linux desktop launcher ($APPLICATIONS_DIR/shiply.desktop)..."
cat > "$APPLICATIONS_DIR/shiply.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Shiply
Comment=Universal Desktop Deployment & Release Automation Platform
Exec=$INSTALL_BIN_DIR/shiply
Path=$INSTALL_SHARE_DIR
Icon=$ICON_PATH
Terminal=false
Categories=Development;Utility;
StartupWMClass=Shiply
EOF

chmod +x "$APPLICATIONS_DIR/shiply.desktop"

echo "Shiply installed successfully!"
echo "  Binary:   $INSTALL_BIN_DIR/shiply"
echo "  Launcher: $APPLICATIONS_DIR/shiply.desktop"

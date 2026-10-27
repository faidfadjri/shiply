#!/usr/bin/env bash
# ==============================================================================
# Shiply macOS Application Installer
# ==============================================================================
# Installs Shiply into /Applications/Shiply.app (or ~/Applications/Shiply.app),
# writes the macOS application bundle, icon, Info.plist, and creates a CLI symlink
# in /usr/local/bin/shiply (or ~/.local/bin/shiply).
#
# If a prebuilt binary isn't present locally, it automatically downloads the
# latest matching macOS release (Apple Silicon or Intel) from GitHub Releases.
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." 2>/dev/null && pwd || echo "$SCRIPT_DIR")"
GITHUB_REPO="faidfadjri/shiply"
APP_NAME="Shiply"
VERSION="0.9.0"

echo "========================================================"
echo "           Shiply macOS Application Installer           "
echo "========================================================"

# 1. Determine CPU Architecture
ARCH="$(uname -m)"
case "$ARCH" in
    arm64|aarch64)
        ASSET_NAME="shiply-macos-aarch64"
        ARCH_DESC="Apple Silicon (arm64)"
        ;;
    x86_64)
        ASSET_NAME="shiply-macos-x86_64"
        ARCH_DESC="Intel (x86_64)"
        ;;
    *)
        ASSET_NAME="shiply-macos-aarch64"
        ARCH_DESC="$ARCH"
        ;;
esac
echo "[1/4] Detected system architecture: $ARCH_DESC"

# 2. Resolve or Download Shiply macOS Executable
TMP_BIN="/tmp/shiply_macos_binary_$$"
FINAL_BIN=""

if [[ -f "$SCRIPT_DIR/shiply" && -x "$SCRIPT_DIR/shiply" ]]; then
    echo "      Found local prebuilt binary: $SCRIPT_DIR/shiply"
    FINAL_BIN="$SCRIPT_DIR/shiply"
elif [[ -f "$SCRIPT_DIR/$ASSET_NAME" && -x "$SCRIPT_DIR/$ASSET_NAME" ]]; then
    echo "      Found local architecture binary: $SCRIPT_DIR/$ASSET_NAME"
    FINAL_BIN="$SCRIPT_DIR/$ASSET_NAME"
elif [[ -f "$REPO_ROOT/target/release/shiply" && -x "$REPO_ROOT/target/release/shiply" ]]; then
    echo "      Found local cargo release build: $REPO_ROOT/target/release/shiply"
    FINAL_BIN="$REPO_ROOT/target/release/shiply"
fi

if [[ -z "$FINAL_BIN" ]]; then
    # Try downloading from GitHub Releases
    DOWNLOAD_URL="https://github.com/${GITHUB_REPO}/releases/latest/download/${ASSET_NAME}"
    echo "[2/4] Downloading ${APP_NAME} binary from GitHub Releases..."
    echo "      URL: $DOWNLOAD_URL"

    DOWNLOAD_OK=false
    if command -v curl >/dev/null 2>&1; then
        if curl -fsSL -o "$TMP_BIN" "$DOWNLOAD_URL"; then
            chmod +x "$TMP_BIN"
            FINAL_BIN="$TMP_BIN"
            DOWNLOAD_OK=true
            echo "      Download succeeded."
        fi
    elif command -v wget >/dev/null 2>&1; then
        if wget -q -O "$TMP_BIN" "$DOWNLOAD_URL"; then
            chmod +x "$TMP_BIN"
            FINAL_BIN="$TMP_BIN"
            DOWNLOAD_OK=true
            echo "      Download succeeded."
        fi
    fi

    # Fallback: Try building with Cargo if Rust is installed
    if [[ "$DOWNLOAD_OK" != "true" ]]; then
        if command -v cargo >/dev/null 2>&1 && [[ -f "$REPO_ROOT/Cargo.toml" ]]; then
            echo "      Download unavailable; building from local source (cargo build --release)..."
            (cd "$REPO_ROOT" && cargo build --release)
            FINAL_BIN="$REPO_ROOT/target/release/shiply"
        else
            echo "ERROR: Could not find prebuilt binary and could not download from GitHub." >&2
            echo "       Please check your internet connection or run: cargo build --release" >&2
            exit 1
        fi
    fi
else
    echo "[2/4] Using available prebuilt binary: $FINAL_BIN"
fi

# 3. Determine Installation Destination
if [[ -w "/Applications" ]]; then
    APP_DIR="/Applications/Shiply.app"
else
    mkdir -p "$HOME/Applications"
    APP_DIR="$HOME/Applications/Shiply.app"
fi

echo "[3/4] Creating macOS application bundle at: $APP_DIR"
CONTENTS_DIR="$APP_DIR/Contents"
MACOS_DIR="$CONTENTS_DIR/MacOS"
RESOURCES_DIR="$CONTENTS_DIR/Resources"

mkdir -p "$MACOS_DIR" "$RESOURCES_DIR"

# Copy executable
cp -f "$FINAL_BIN" "$MACOS_DIR/shiply"
chmod +x "$MACOS_DIR/shiply"

# Clean up temp download if used
if [[ -f "$TMP_BIN" ]]; then
    rm -f "$TMP_BIN"
fi

# Copy logo & icons
if [[ -f "$SCRIPT_DIR/logo.png" ]]; then
    cp -f "$SCRIPT_DIR/logo.png" "$RESOURCES_DIR/logo.png"
elif [[ -f "$REPO_ROOT/logo.png" ]]; then
    cp -f "$REPO_ROOT/logo.png" "$RESOURCES_DIR/logo.png"
fi

if [[ -f "$SCRIPT_DIR/logo.ico" ]]; then
    cp -f "$SCRIPT_DIR/logo.ico" "$RESOURCES_DIR/logo.ico"
elif [[ -f "$REPO_ROOT/logo.ico" ]]; then
    cp -f "$REPO_ROOT/logo.ico" "$RESOURCES_DIR/logo.ico"
fi

# Write Info.plist
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
    <string>logo.png</string>
    <key>CFBundleIdentifier</key>
    <string>io.shiply.desktop</string>
    <key>CFBundleName</key>
    <string>Shiply</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>${VERSION}</string>
    <key>CFBundleVersion</key>
    <string>${VERSION}</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
EOF

# 4. Set up CLI Symlink in /usr/local/bin or ~/.local/bin
echo "[4/4] Setting up CLI symlink..."
CLI_INSTALLED=false

if [[ -d "/usr/local/bin" && -w "/usr/local/bin" ]]; then
    ln -sf "$MACOS_DIR/shiply" "/usr/local/bin/shiply"
    echo "      Linked CLI to /usr/local/bin/shiply"
    CLI_INSTALLED=true
elif [[ -d "$HOME/.local/bin" || -d "$HOME/bin" ]]; then
    mkdir -p "$HOME/.local/bin"
    ln -sf "$MACOS_DIR/shiply" "$HOME/.local/bin/shiply"
    echo "      Linked CLI to $HOME/.local/bin/shiply"
    CLI_INSTALLED=true
fi

# Create Desktop Shortcut
DESKTOP_DIR="$HOME/Desktop"
if [[ -d "$DESKTOP_DIR" && -w "$DESKTOP_DIR" ]]; then
    ln -sf "$APP_DIR" "$DESKTOP_DIR/Shiply.app" 2>/dev/null || true
    echo "      Created Desktop shortcut: $DESKTOP_DIR/Shiply.app"
fi

# Send native notification if osascript is available
if command -v osascript >/dev/null 2>&1; then
    osascript -e 'display notification "Shiply has been installed successfully!" with title "Shiply Installer"' 2>/dev/null || true
fi

echo "========================================================"
echo " Installation Complete!"
echo " App Bundle: $APP_DIR"
if [[ "$CLI_INSTALLED" == "true" ]]; then
    echo " Terminal Command: 'shiply'"
fi
echo "========================================================"

# Launch the app if requested or interactive
if command -v open >/dev/null 2>&1; then
    echo "Launching Shiply..."
    open "$APP_DIR" || true
fi

# If invoked by double-clicking in Finder, keep Terminal window open until user acknowledges
if [[ "${TERM_PROGRAM:-}" == "Apple_Terminal" ]] || [[ -t 0 ]]; then
    read -r -p "Press [Enter] to close this window..." || true
fi

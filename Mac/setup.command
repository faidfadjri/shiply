#!/usr/bin/env bash
# Double-clickable macOS Finder Setup Launcher for Shiply
set -e
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"
exec "$SCRIPT_DIR/install-mac.sh" "$@"

#!/usr/bin/env bash
# Double-clickable macOS Finder installer for Shiply
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec "$SCRIPT_DIR/install-mac.sh" "$@"

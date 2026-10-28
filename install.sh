#!/usr/bin/env bash
# Build and install the applet for the current user (no sudo).
#   ./install.sh            build + install
#   ./install.sh uninstall  remove installed files
set -euo pipefail

APP_ID=io.github.farzodd.CosmicAppletClaudeUsage
BIN_DIR="$HOME/.local/bin"
DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}"
BIN="$BIN_DIR/cosmic-applet-claude-usage"
DESKTOP="$DATA_DIR/applications/$APP_ID.desktop"
ICON="$DATA_DIR/icons/hicolor/scalable/apps/$APP_ID.svg"

cd "$(dirname "$0")"

if [[ "${1:-}" == "uninstall" ]]; then
    rm -fv "$BIN" "$DESKTOP" "$ICON"
    exit 0
fi

# -j limits parallelism; see README "Build notes".
cargo build --release -j "${JOBS:-4}"

install -Dm755 target/release/cosmic-applet-claude-usage "$BIN"
install -Dm644 res/claude.svg "$ICON"
# Absolute Exec path: the panel's PATH may not include ~/.local/bin.
install -d "$(dirname "$DESKTOP")"
sed "s|^Exec=.*|Exec=$BIN|" "res/$APP_ID.desktop" > "$DESKTOP"

echo "Installed. Add it via Settings → Desktop → Panel → Configure panel applets → Claude Usage."
echo "If it was already on the panel, restart it with: pkill -f cosmic-applet-claude-usage"

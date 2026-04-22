#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
cargo build --profile=fast-release -p helix-term
# Install the binary somewhere on PATH.
# Adjust DEST if you prefer ~/bin, /usr/local/bin, etc.
# DEST="${HELIX_INSTALL_DIR:-$HOME/.local/bin}"
# mkdir -p "$DEST"
# install -m 0755 target/fast-release/hx "$DEST/hx"
# echo "Installed $DEST/hx"
# "$DEST/hx" --version
cargo install --profile=fast-release --project=helix-term

echo Helix still needs its runtime files at runtime (grammars/queries/themes).
echo Either set for example, HELIX_RUNTIME=~/projects/helix/runtime in your shell rc, or copy/symlink the runtime/ directory to one of the standard locations (~/.config/helix/runtime, /usr/local/share/helix/runtime, etc.). cargo install won't handle that for you.


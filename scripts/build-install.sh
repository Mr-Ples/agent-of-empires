#!/usr/bin/env bash
set -euo pipefail

PROJECT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
INSTALL_DIR=${INSTALL_DIR:-"$HOME/.local/bin"}

usage() {
    cat <<'EOF'
Usage: scripts/build-install.sh

Build Agent of Empires from this checkout and install the aoe binary.

Environment:
  INSTALL_DIR  Installation directory. Defaults to ~/.local/bin.
EOF
}

for argument in "$@"; do
    case "$argument" in
        -h|--help) usage; exit 0 ;;
        *)
            printf 'Unknown option: %s\n' "$argument" >&2
            usage >&2
            exit 1
            ;;
    esac
done

cd "$PROJECT_DIR"
cargo build --release --features serve

mkdir -p "$INSTALL_DIR"
cp target/release/aoe "$INSTALL_DIR/aoe"
chmod +x "$INSTALL_DIR/aoe"

printf 'Installed aoe to %s/aoe\n' "$INSTALL_DIR"
printf 'Ensure %s is on your PATH, then run: aoe --version\n' "$INSTALL_DIR"

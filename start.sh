#!/usr/bin/env bash
# Start Pin (X11 backend so "always on top" works under GNOME Wayland).
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ELECTRON="${ELECTRON:-$HERE/node_modules/electron/dist/electron}"
exec "$ELECTRON" --ozone-platform=x11 "$HERE" "$@"

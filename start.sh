#!/usr/bin/env bash
# Start Pin (X11 backend so "always on top" works under GNOME Wayland).
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
NATIVE="${PIN_NATIVE:-$HOME/.local/lib/pin-native/pin-native}"
if [[ -x "$NATIVE" ]]; then
  exec env GDK_BACKEND=x11 WEBKIT_DISABLE_COMPOSITING_MODE=1 "$NATIVE" "$@"
fi
ELECTRON="${ELECTRON:-$HERE/node_modules/electron/dist/electron}"
exec "$ELECTRON" --ozone-platform=x11 "$HERE" "$@"

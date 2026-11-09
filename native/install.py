#!/usr/bin/env python3
"""Install a verified native binary and both menu entries for this user."""
import os
from pathlib import Path
import shlex
import shutil
import sys

source = Path(sys.argv[1]).resolve()
if not source.is_file():
    raise SystemExit("Pass the built pin-native executable.")
home = Path.home()
data = Path(os.environ.get("XDG_DATA_HOME", home / ".local/share"))
config = Path(os.environ.get("XDG_CONFIG_HOME", home / ".config"))
binary = home / ".local/lib/pin-native/pin-native"
binary.parent.mkdir(parents=True, exist_ok=True)
staged = binary.with_suffix(".install")
shutil.copyfile(source, staged)
staged.chmod(0o755)
staged.replace(binary)

for name, notes in [("pin-native", False), ("sticky-notes", True)]:
    launcher = home / ".local/bin" / name
    launcher.parent.mkdir(parents=True, exist_ok=True)
    launcher.write_text("#!/bin/sh\nexec env GDK_BACKEND=x11 WEBKIT_DISABLE_COMPOSITING_MODE=1 "
                        + shlex.quote(str(binary)) + (" --notes" if notes else "") + ' "$@"\n')
    launcher.chmod(0o755)
    desktop = ("[Desktop Entry]\nType=Application\n"
               f"Name={'Sticky Notes' if notes else 'Pin'}\n"
               f"Comment={'A plain text scratch pad' if notes else 'Floating priorities with daily tasks'}\n"
               f'Exec="{str(launcher).replace(chr(34), chr(92) + chr(34))}"\n'
               f"Icon={'sticky-notes' if notes else 'pin'}\n"
               "Terminal=false\nCategories=Utility;\n")
    entry = data / "applications" / ("sticky-notes.desktop" if notes else "pin.desktop")
    entry.parent.mkdir(parents=True, exist_ok=True)
    entry.write_text(desktop)
    autostart = config / "autostart/pin.desktop"
    if not notes and autostart.exists():
        autostart.write_text(desktop)

icons = data / "icons/hicolor/256x256/apps"
icons.mkdir(parents=True, exist_ok=True)
for name, filename in [("pin", "pin-256.png"), ("sticky-notes", "notes-256.png")]:
    shutil.copyfile(Path(__file__).resolve().parent.parent / "logo" / filename, icons / (name + ".png"))

print("Installed Pin and Sticky Notes.")

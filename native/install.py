#!/usr/bin/env python3
"""Install a verified native binary and both menu entries for this user."""
import os
import argparse
from pathlib import Path
import shlex
import shutil

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('binary', type=Path)
native = parser.add_mutually_exclusive_group()
native.add_argument('--notes-iced', action='store_true', help='Install native Notes only.')
native.add_argument('--pin-iced', action='store_true', help='Install native Pin only.')
args = parser.parse_args()
source = args.binary.resolve()
if not source.is_file():
    raise SystemExit("Pass the built pin-native executable.")
home = Path.home()
data = Path(os.environ.get("XDG_DATA_HOME", home / ".local/share"))
config = Path(os.environ.get("XDG_CONFIG_HOME", home / ".config"))
is_native = args.notes_iced or args.pin_iced
binary = home / ".local/lib/pin-native" / ('notes-iced' if args.notes_iced else 'pin-iced' if args.pin_iced else 'pin-native')
binary.parent.mkdir(parents=True, exist_ok=True)
staged = binary.with_suffix(".install")
shutil.copyfile(source, staged)
staged.chmod(0o755)
staged.replace(binary)

for name, notes in ([("sticky-notes", True)] if args.notes_iced else [("pin-native", False)] if args.pin_iced else [("pin-native", False), ("sticky-notes", True)]):
    launcher = home / ".local/bin" / name
    launcher.parent.mkdir(parents=True, exist_ok=True)
    launch = ('exec ' if is_native else 'exec env GDK_BACKEND=x11 WEBKIT_DISABLE_COMPOSITING_MODE=1 ')
    launcher.write_text("#!/bin/sh\n" + launch + shlex.quote(str(binary))
                        + (" --notes" if notes and not is_native else "") + ' "$@"\n')
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

print("Installed native Sticky Notes." if args.notes_iced else "Installed native Pin." if args.pin_iced else "Installed Pin and Sticky Notes.")

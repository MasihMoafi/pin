# Pin and Sticky Notes

Two floating Linux apps use native Rust/Iced widgets with tiny-skia software
rendering. Both recreate the neutral shadcn style with Lucide icons and need
no WebView. Pin reads the existing Markdown list and
`~/.config/Pin/settings.json`; Notes is a plain text scratch pad.

Run `pin-native` for priorities and `sticky-notes` for the scratch pad. Both
windows resize, remember their bounds, and can be pinned above other windows.
Install verified builds from the repository root:

```sh
python3 native/install.py /path/to/pin-iced --pin-iced
python3 native/install.py /path/to/notes-iced --notes-iced
```

Each command updates only its app's executable and menu entry. The Pin command
also updates its existing login entry if enabled.
The Notes icon uses Pin's cream tile and tilted dark silhouette. Each app
follows the desktop light/dark preference, including changes while open.

In Pin, open a task's **⋯ → Repeat daily**. Checking it completes today's
occurrence. It becomes unfinished on the next local date, including when Pin
is closed overnight. A dated task keeps its time and advances to the new day.
The Markdown suffix is `🔁 daily`; completed daily tasks also store
`✅ YYYY-MM-DD`. Regular tasks keep their current behavior.

Both apps have Undo, Redo and Save buttons. Ctrl+Z undoes typing and task
changes; Ctrl+Shift+Z or Ctrl+Y redoes them. Ctrl+S saves immediately. The
shortcuts use physical key positions so they work with English and Persian
layouts. Typing is grouped by pauses; task actions are separate undo steps.

Ctrl+N adds a task, Ctrl+T adds a subtask, Ctrl+Enter completes it, Ctrl+D opens
its options, and Ctrl+Shift+Backspace deletes it. Enter splits a task, Tab and
Shift+Tab change its level, and Alt+↑ / Alt+↓ reorder it. Only the grip starts
a mouse drag, and the drop line shows its destination. Ctrl+Tab moves from a
task to the header controls. The task menu opens over the list. Date opens a month calendar: use the
month arrows or Page Up/Page Down, move between days with the arrow keys,
and pick with Enter. Time opens hour and minute dropdowns; Color opens the
palette and custom hex field. Escape goes back, then closes the menu.
Custom colors apply with their button or Enter.
Task rows grow with their text and wrap when the window becomes narrower.

Sticky Notes saves to `Documents/Sticky Notes.txt`, using the desktop's
Documents folder setting. Set `STICKY_NOTES_FILE`, or `file` in
`~/.config/StickyNotes/settings.json`, to choose another location. It is a
continuous scratch pad with standard selection, clipboard and a context menu.
Window settings live in `Pin/iced.json` and `StickyNotes/iced.json`. Each app
migrates the existing `native.json` window bounds and pin state on its first
launch, including display scaling. Pin also preserves its folded branches.

Both apps save automatically and atomically. Concurrent file changes pause
saving until you choose **Use disk** or **Keep mine**. A cooperating second
writer uses the same lock; other editors must still coordinate simultaneous
saves. The older Electron releases do not understand recurrence markers;
use the Rust app to edit daily tasks.

## Build and verify

The native apps need Rust and an X11 desktop or XWayland. Install the X11
keyboard runtime (`libxkbcommon-x11-0` on Ubuntu). Rust builds run on GitHub
Actions in the Iced workflow; no Node or WebView build is needed.

```sh
cargo fmt --manifest-path native/Cargo.toml --check
cargo test --locked --manifest-path native/Cargo.toml --no-default-features --features iced-ui
cargo clippy --locked --manifest-path native/Cargo.toml --no-default-features --features iced-ui --all-targets -- -D warnings
cargo build --locked --manifest-path native/Cargo.toml --release --no-default-features --features iced-ui --bins
```

`native/test/pin_iced.py` drives Pin on a hidden X11 display. It checks physical
English/Persian shortcuts, toolbar actions, task wrapping at three widths,
recurrence, file conflicts, drag behavior, pinning and window persistence.
It needs Xvfb, an X11 window manager, xdotool, xprop, setxkbmap, xclip and Pillow.
With the hidden display and window manager active:

```sh
DISPLAY=:94 dbus-run-session -- /usr/bin/python3 -I native/test/pin_iced.py /path/to/webview-baseline /path/to/pin-iced /path/to/screenshots
```

## Native Sticky Notes

`notes-iced` is the native scratch pad. It uses the same document path and
atomic Rust storage as Pin. Installing it updates only the Sticky Notes menu
entry and launcher.

Build with `--no-default-features --features iced-ui --bin notes-iced`. It
needs the Linux X11 keyboard runtime (`libxkbcommon-x11-0` on Ubuntu). The app
keeps the matching Notes logo, Undo/Redo/Save buttons and physical shortcuts.

```sh
cargo build --locked --manifest-path native/Cargo.toml --release --no-default-features --features iced-ui --bin notes-iced
python3 native/install.py /path/to/notes-iced --notes-iced
```

`native/test/iced.py BASELINE ICED OUTPUT` drives both real windows on an
isolated X11 display. It checks physical English/Persian shortcuts, typing,
clipboard, toolbar actions, keyboard navigation, file conflicts, window
persistence and migration, dark rendering and long-note undo, then records
paired memory, startup and idle CPU measurements. Run with
`ICED_TEST_SCALE=2` to check display scaling. The Iced workflow retains the
raw results and screenshots. Use isolated note files while testing.

## Previous WebView implementation

The Tauri/shadcn implementation remains in `native/ui` and `src/shell.rs`.
Its default-feature build needs Node.js 22, GTK 3 and WebKitGTK 4.1 development
files. `native/test/desktop.py` verifies that version. It shares the same
Markdown/plain-text storage paths with the native apps.

# Pin and Sticky Notes

Two floating Linux windows share a Rust executable and use the system WebKit
WebView. Their interface uses shadcn/ui components and Lucide icons. There is
no bundled browser runtime. Pin reads the existing Markdown list and
`~/.config/Pin/settings.json`.

Run `pin-native` for priorities, or `pin-native --notes` for a plain text
scratch pad. Both windows resize, remember their bounds, and can be pinned
above other windows. Install a verified build from the repository root:

```sh
python3 native/install.py /path/to/pin-native
```

It adds both apps to the menu and updates Pin's existing login entry if enabled.
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
a mouse drag. Options also contain the color palette, date and delete.
Task rows grow with their text and wrap when the window becomes narrower.

Sticky Notes saves to `Documents/Sticky Notes.txt`, using the desktop's
Documents folder setting. Set `STICKY_NOTES_FILE`, or `file` in
`~/.config/StickyNotes/settings.json`, to choose another location. It is a
continuous scratch pad with standard selection, clipboard and a context menu.
Window settings live in each app's `native.json` configuration file.

Both apps save automatically and atomically. Concurrent file changes pause
saving until you choose **Use disk** or **Keep mine**. A cooperating second
writer uses the same lock; other editors must still coordinate simultaneous
saves. The older Electron releases do not understand recurrence markers;
use the Rust app to edit daily tasks.

## Build and verify

Requires Rust, Node.js 22 or newer, GTK 3 and WebKitGTK 4.1 development files.
See [Tauri's Linux prerequisites](https://tauri.app/start/prerequisites/#linux).
The release workflow builds on GitHub Actions.

```sh
npm --prefix native/ui ci
npm --prefix native/ui run build
cargo fmt --manifest-path native/Cargo.toml --check
cargo test --manifest-path native/Cargo.toml --no-default-features
cargo clippy --manifest-path native/Cargo.toml --all-targets -- -D warnings
cargo build --manifest-path native/Cargo.toml --release
```

`native/test/desktop.py` drives the real executable on a hidden X11 display.
It checks physical English/Persian shortcuts, toolbar actions, task wrapping
at three widths, recurrence, file conflicts, pinning and window persistence.
Requires AT-SPI introspection, Xvfb, an X11 window manager, xdotool, xprop,
setxkbmap and Pillow. Run with the hidden display and window manager active:

```sh
DISPLAY=:94 dbus-run-session -- /usr/bin/python3 -I native/test/desktop.py /path/to/pin-native /path/to/screenshots
```

## Native Iced prototype

`notes-iced` is the feasibility build for replacing the WebView interface with
native Rust widgets and tiny-skia software rendering. It uses the existing
Rust file storage and keeps its window preferences in
`StickyNotes/iced-prototype.json`. The installed launchers currently run the
verified WebView version.

Build the prototype with `--no-default-features --features iced-ui --bin
notes-iced`. It needs the Linux X11 keyboard runtime (`libxkbcommon-x11-0` on
Ubuntu). The interface recreates the neutral shadcn style and uses the same
Lucide icons and Notes app logo.

`native/test/iced.py BASELINE ICED OUTPUT` drives both real windows on an
isolated X11 display. It checks physical English/Persian shortcuts, typing,
clipboard, toolbar actions and keyboard navigation, then records paired
memory, startup and idle CPU measurements. The Iced workflow retains the
raw results and screenshots. Use isolated note files while testing.

# Native Pin and Sticky Notes

Two small Linux windows share one Rust executable and the system's GTK 4
text controls. Each runs as one process, without a browser runtime. Pin reads
the existing Markdown list and `~/.config/Pin/settings.json`.

Run `pin-native` for priorities, or `pin-native --notes` for a plain text
scratch pad. Both windows resize, remember their bounds, and can be pinned
above other windows. The launcher uses X11 (including XWayland) for pinning.

In Pin, open a task's **⋮ → Repeat daily**. Checking it completes today's
occurrence. It becomes unfinished on the next local date, including when Pin
is closed overnight. A dated task keeps its time and advances to the new day.
The Markdown suffix is `🔁 daily`; completed daily tasks also store
`✅ YYYY-MM-DD`. Regular tasks keep their current behavior.

Ctrl+Z undoes typing and task changes; Ctrl+Shift+Z or Ctrl+Y redoes them.
Ctrl+N adds a task, Ctrl+T adds a subtask, Ctrl+Enter completes it, Ctrl+D opens
its options, and Ctrl+Shift+Backspace deletes it. Enter splits a task, Tab and
Shift+Tab change its level, and Alt+↑ / Alt+↓ reorder it. Only the grip starts
a mouse drag. The options also contain the color palette, date and delete.

Sticky Notes saves to `Documents/Sticky Notes.txt`, using the desktop's
Documents folder setting. Set `STICKY_NOTES_FILE`, or `file` in
`~/.config/StickyNotes/settings.json`, to choose another location. It is a
continuous scratch pad with standard selection, clipboard, context menu and
undo. Notes stay private on this machine. Window settings live in each app's
`native.json` configuration file.

Both apps save atomically and watch the file. Concurrent changes pause
saving until you choose **Use disk** or **Keep mine**. A cooperating second
writer uses the same lock; other editors must still coordinate simultaneous
saves. The existing Electron version does not understand recurrence markers;
use the native app to edit daily tasks.

## Build and verify

Requires Rust and GTK 4.8 or newer (`libgtk-4-dev` on Ubuntu). The release
workflow builds on GitHub Actions. To build on another development machine:

```sh
cargo fmt --check
cargo test --no-default-features
cargo clippy --all-targets -- -D warnings
cargo build --release
```

`test/desktop.py` drives the actual app with isolated files on a hidden X11
display. It checks text undo/redo, daily tasks, task changes, file conflicts
and resizing. It requires GTK 4 introspection, AT-SPI, Xvfb and xdotool.

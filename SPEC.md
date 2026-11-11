# Pin: spec

An always-on-top priority note. Standard app behavior (undo, text selection, windows, aligned layout) is assumed;
this file lists only what is particular to Pin. The native Rust apps live in `native/`;
[native/README.md](native/README.md) covers daily tasks, Sticky Notes and native verification.
The original TypeScript implementation (`src/`, built to `dist/`) remains for existing releases.

## Data
- One Markdown file: `$PRIORITIES_FILE`, else `file` in `~/.config/Pin/settings.json`, else `Documents/Pin.md`.
- Line: `- [ ] ==🟢text== 📅 2026-11-07 09:30`; two spaces of indent per subtask level; text above the list is kept.
  Six named colors use Rustidian's `==🟢…==` markers; any other color is `<mark style="background: #rrggbb;">`.
- Line order is priority.
- External changes reload while idle. During editing or an unsaved change, keep the local text and ask which version
  to use; compare the disk before each save and report storage errors. Saves replace the file atomically through a
  unique temporary file and preserve a list-file symlink.

## Window
- Both apps use native Rust/Iced widgets with tiny-skia software rendering, a consistent shadcn-style interface and Lucide icons; no WebView.
- Sticky Notes uses Pin's cream icon tile and tilted dark silhouette, and remembers its resizable window.
- Resize grips are easy to acquire on every edge and corner, follow the pointer continuously, and leave the scrollbar usable, including at the actual desktop's fractional scale.
- Floats on top; the header pin (Lucide's, as in Elpis) is filled in its own color while pinned, outline when not.
- Opens on the right of the screen the first time; starts at log-in (`~/.config/autostart/pin.desktop`).
- App icon: the tilted pin in `logo/` (approved 7 Nov), on a light tile.
- Follow the desktop light/dark preference live, including menus, fields and task colors.

## Tasks
- Task text wraps and grows vertically at every window width; no line may be clipped.
- The task options offer daily recurrence; completed daily tasks reopen on the next local date, preserving any time.
- Completed tasks go last among their siblings; a newly completed task goes to the very bottom with its subtasks.
  Unfinished siblings keep their priority order. Reopening a task puts it after the unfinished siblings; undo restores its previous place and status together.
- Each row has a checkbox, text and ⋯ (daily recurrence, color, date, subtask, delete). Options open as dropdown menus over the list; dates use a month calendar and times use hour/minute dropdowns, as in the earlier calendar UI.
- Checkboxes have a clear gap before the text; chevrons, checkboxes and trailing controls share the center of the first text line, including wrapped tasks. Separate rows have visible space between them.
- Unchecked shadcn checkboxes keep a solid background and an outline with at least 3:1 contrast against task colors in light and dark themes.
- A new subtask goes first under its task (8 Nov). A task with subtasks has a chevron in the left gutter that folds them
  away (also Ctrl+.); the folded tasks are remembered on this computer, never written to the file.
- Hotkeys: Ctrl+T subtask, Ctrl+D date, Ctrl+Shift+Backspace delete, Ctrl+Enter done, Alt+↑/↓ move;
  Enter splits at the caret; Tab / Shift+Tab nest and lift; Backspace on an empty task deletes it.
- The task options offer twelve colors plus any custom color. A new task takes its neighbor's color.
- Dates read as Elpis Automations shows them; today is green, past and not done is red.

## Release
- `electron-builder.yml`: Snap (strict, `home` plug), AppImage and `.deb`; CI in `.github/workflows/release.yml`
  builds on every push and packages on a `v*` tag. Store text: `SNAP_LISTING.md`.

## Acceptance
Both Rust apps provide toolbar Undo, Redo and Save. Ctrl+Z, Ctrl+Shift+Z / Ctrl+Y and Ctrl+S call the same actions under English and Persian keyboard layouts. Sticky Notes styles Markdown emphasis while editing and offers a rendered reading view; see [native/README.md](native/README.md).

Drive the native executables with `native/test/pin_iced.py` and `native/test/iced.py` on a hidden display; see `native/README.md`. `native/test/desktop.py` covers the previous WebView version.
On an isolated GNOME Wayland desktop, `native/test/resize.py PIN NOTES OUTPUT` captures continuous edge drags and checks checkbox contrast, completion and undo. Resizing must retain the painted contents without black frames or edges. During a fast top-left drag, the painted bottom-right corner stays fixed, including when the minimum size is reached.

Against the app on a hidden display with `--remote-debugging-port=9377`, run `python3 test/e2e.py <scratch list>`.
Run `python3 test/sync_e2e.py <scratch list> 9377 <screenshot directory>` for conflicts, reload, theme and contrast;
it closes the app after checking the last save.
File round trip and storage: `npm test`.

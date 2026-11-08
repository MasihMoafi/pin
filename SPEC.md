# Pin: spec

An always-on-top priority note. Standard app behavior (undo, text selection, windows, aligned layout) is assumed;
this file lists only what is particular to Pin. Code is TypeScript (`src/`), built to `dist/`.

## Data
- One Markdown file: `$PRIORITIES_FILE`, else `file` in `~/.config/Pin/settings.json`, else `Documents/Pin.md`.
- Line: `- [ ] ==🟢text== 📅 2026-11-07 09:30`; two spaces of indent per subtask level; text above the list is kept.
  Six named colors use Rustidian's `==🟢…==` markers; any other color is `<mark style="background: #rrggbb;">`.
- Line order is priority.
- External changes reload while idle. During editing or an unsaved change, keep the local text and ask which version
  to use; compare the disk before each save and report storage errors. Saves replace the file atomically through a
  unique temporary file and preserve a list-file symlink.

## Window
- Floats on top; the header pin (Lucide's, as in Elpis) is filled in its own color while pinned, outline when not.
- Opens on the right of the screen the first time; starts at log-in (`~/.config/autostart/pin.desktop`).
- App icon: the tilted pin in `logo/` (approved 7 Nov), on a light tile.
- Follow the desktop light/dark preference live, including menus, fields and task colors.

## Tasks
- Each row on hover: + (subtask), trash (delete), ⋯ (color, date, subtask, delete).
- A new subtask goes first under its task (8 Nov). A task with subtasks has a chevron in the left gutter that folds them
  away (also Ctrl+. and the ⋯ menu); the folded tasks are remembered on this computer, never written to the file.
- Hotkeys: Ctrl+T subtask, Ctrl+D date, Ctrl+Shift+Backspace delete, Ctrl+Enter done, Alt+↑/↓ move;
  Enter splits at the caret; Tab / Shift+Tab nest and lift; Backspace on an empty task deletes it.
- The color dot opens twelve colors plus any custom color. A new task takes its neighbor's color.
- Dates read as Elpis Automations shows them; today is green, past and not done is red.

## Release
- `electron-builder.yml`: Snap (strict, `home` plug), AppImage and `.deb`; CI in `.github/workflows/release.yml`
  builds on every push and packages on a `v*` tag. Store text: `SNAP_LISTING.md`.

## Acceptance
Against the app on a hidden display with `--remote-debugging-port=9377`, run `python3 test/e2e.py <scratch list>`.
Run `python3 test/sync_e2e.py <scratch list> 9377 <screenshot directory>` for conflicts, reload, theme and contrast;
it closes the app after checking the last save.
File round trip and storage: `npm test`.

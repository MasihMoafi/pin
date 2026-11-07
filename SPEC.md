# Pin: spec

An always-on-top priority note. Standard app behavior (undo, text selection, windows, aligned layout) is assumed;
this file lists only what is particular to Pin. Code is TypeScript (`src/`), built to `dist/`.

## Data
- One Markdown file: `$PRIORITIES_FILE`, else `file` in `~/.config/Pin/settings.json`, else `Documents/Pin.md`.
- Line: `- [ ] ==🟢text== 📅 2026-11-07 09:30`; two spaces of indent per subtask level; text above the list is kept.
  Six named colors use Rustidian's `==🟢…==` markers; any other color is `<mark style="background: #rrggbb;">`.
- Line order is priority.

## Window
- Floats on top; the header pin (Lucide's, as in Elpis) is filled in its own color while pinned, outline when not.
- Opens on the right of the screen the first time; starts at log-in (`~/.config/autostart/pin.desktop`).
- App icon: the tilted pin in `logo/` (approved 7 Nov), on a light tile.

## Tasks
- Each row on hover: + (subtask), trash (delete), ⋯ (color, date, subtask, delete).
- Hotkeys: Ctrl+T subtask, Ctrl+D date, Ctrl+Shift+Backspace delete, Ctrl+Enter done, Alt+↑/↓ move;
  Enter splits at the caret; Tab / Shift+Tab nest and lift; Backspace on an empty task deletes it.
- The color dot opens twelve colors plus any custom color. A new task takes its neighbor's color.
- Dates read as Elpis Automations shows them; today is green, past and not done is red.

## Release
- `electron-builder.yml`: Snap (strict, `home` plug), AppImage and `.deb`; CI in `.github/workflows/release.yml`
  builds on every push and packages on a `v*` tag. Store text: `SNAP_LISTING.md`.

## Acceptance
`python3 test/e2e.py <scratch copy>` against the app on a hidden display with `--remote-debugging-port=9377`:
30 checks pass (7 Nov). Format round trip: `npm test`.

# Pin

![Pin](assets/screenshot.png)

Pin keeps your ranked priorities on top of every window. Each line is a task: drag it up or down, color it, give it
subtasks and a date. The list is a plain Markdown file, so any notes app can read and edit it too.

## Install

- Snap: `sudo snap install pinned` (after the first store release)
- AppImage and `.deb`: from the GitHub release page

## Use

| Do | How |
|---|---|
| New task | **New task**, or Enter at the end of a task (Enter mid-text splits it) |
| Subtask | **+** on the row, Ctrl+T, or Tab (Shift+Tab lifts it back) |
| Delete | trash icon on the row, Ctrl+Shift+Backspace, or Backspace on an empty task |
| Date and time | Ctrl+D, or **⋯ → Add date** |
| Color | click the round dot: twelve colors or any custom color |
| Done | the checkbox, or Ctrl+Enter |
| Reorder | drag the handle, or Alt+↑ / Alt+↓ |
| Undo / redo | Ctrl+Z / Ctrl+Shift+Z or Ctrl+Y |
| Keep on top | the pin in the header: filled while Pin floats on top |

## The list file

Pin edits `Pin.md` in your Documents folder. To use another file, write its path in
`~/.config/Pin/settings.json` (Snap: `~/snap/pinned/current/.config/Pin/settings.json`):

```json
{ "file": "~/Notes/priorities.md" }
```

A task line looks like `- [ ] ==🟢Ship the release notes== 📅 2026-11-07 10:00`; two spaces of indent make a subtask.
The six named colors use the `==🟢…==` highlight markers; any other color is saved as
`<mark style="background: #ec4899;">…</mark>`.

## Build

```sh
npm ci
npm run build && npm start
npm test                 # file-format round trip
npm run dist             # Snap, AppImage and .deb into release/
```

MIT licence. Icons: [Lucide](https://lucide.dev) (ISC); see `THIRD_PARTY_NOTICES.md`.

<!-- more-projects:start -->
## More projects

- [Elpis](https://github.com/MasihMoafi/Elpis): a coding agent for the terminal and desktop, with explicit context control
- [Rustidian](https://github.com/MasihMoafi/rustidian): a native Markdown notebook for Linux that opens Obsidian-format vaults
- [rag-mcp](https://github.com/MasihMoafi/rag-mcp): local hybrid search over your documents, as a CLI and MCP server
- [Skills](https://github.com/MasihMoafi/skills): SKILL.md workflows for coding agents
- [WhisperType](https://github.com/MasihMoafi/WhisperType): local voice input for developers
- [Read Aloud](https://github.com/MasihMoafi/read-aloud): select text, press Super+R, hear it in a local voice
- [Love Spot](https://github.com/MasihMoafi/love-spot): a shared drawing board where characters talk, draw and compete with you
- [SVG Studio](https://svg.masihmoafi.com): high-quality image vectorization
- [Clocki](https://github.com/MasihMoafi/clocki): a floating desktop timer for tracking focus hours

All of them: [masihmoafi.com](https://masihmoafi.com/projects)
<!-- more-projects:end -->

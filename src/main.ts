// Pin: an always-on-top, resizable window that edits a Markdown priority list.
import { app, BrowserWindow, ipcMain, nativeImage, screen } from 'electron';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const ROOT = path.join(__dirname, '..');
const STATE = path.join(app.getPath('userData'), 'bounds.json');
const SETTINGS = path.join(app.getPath('userData'), 'settings.json');

/** The list Pin edits: $PRIORITIES_FILE, else `file` in settings.json, else Pin.md in the Documents folder. */
function listFile(): string {
  if (process.env.PRIORITIES_FILE) return process.env.PRIORITIES_FILE;
  try {
    const { file } = JSON.parse(fs.readFileSync(SETTINGS, 'utf8')) as { file?: unknown };
    if (typeof file === 'string' && file) return file.replace(/^~(?=\/)/, os.homedir());
  } catch { /* no settings yet: use the default */ }
  return path.join(app.getPath('documents'), 'Pin.md');
}
const ICON = path.join(ROOT, 'logo/pin-256.png');

interface Bounds { x: number; y: number; width: number; height: number }

function bounds(): Bounds {
  try {
    return JSON.parse(fs.readFileSync(STATE, 'utf8')) as Bounds;
  } catch {
    const { width } = screen.getPrimaryDisplay().workAreaSize;
    return { width: 460, height: 720, x: width - 480, y: 60 };
  }
}

app.whenReady().then(() => {
  const win = new BrowserWindow({
    ...bounds(), minWidth: 300, minHeight: 220, frame: false, alwaysOnTop: true,
    title: 'Pin', icon: ICON, backgroundColor: '#fdfcf7',
    webPreferences: { preload: path.join(__dirname, 'preload.js'), contextIsolation: true },
  });
  win.setAlwaysOnTop(true, 'floating');
  const logo = nativeImage.createFromPath(ICON);
  if (!logo.isEmpty()) win.setIcon(logo);
  void win.loadFile(path.join(ROOT, 'index.html'));
  const save = (): void => {
    fs.mkdirSync(path.dirname(STATE), { recursive: true });
    fs.writeFileSync(STATE, JSON.stringify(win.getBounds()));
  };
  win.on('moved', save);
  win.on('resized', save);

  const FILE = listFile();
  let lastWrite = '';
  ipcMain.handle('read', () => {
    try { return fs.readFileSync(FILE, 'utf8'); } catch { return ''; }
  });
  ipcMain.handle('write', (_e, text: string) => {
    lastWrite = text;
    const tmp = FILE + '.tmp';
    fs.mkdirSync(path.dirname(FILE), { recursive: true });
    fs.writeFileSync(tmp, text);
    fs.renameSync(tmp, FILE);
  });
  ipcMain.on('close', () => app.quit());
  ipcMain.on('pin', (_e, on: boolean) => win.setAlwaysOnTop(on, 'floating'));
  fs.watchFile(FILE, { interval: 1000 }, () => {
    let text: string;
    try { text = fs.readFileSync(FILE, 'utf8'); } catch { return; }
    if (text !== lastWrite) win.webContents.send('changed', text);
  });
});
app.on('window-all-closed', () => app.quit());

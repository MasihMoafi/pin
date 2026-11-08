// Disk versions are the exact file contents; null means that the file is absent.
import fs from 'node:fs';
import path from 'node:path';
import { randomUUID } from 'node:crypto';

export type DiskVersion = string | null;
export type SaveResult = { saved: true } | { saved: false; disk: DiskVersion };

export function readList(file: string): DiskVersion {
  try { return fs.readFileSync(file, 'utf8'); }
  catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT') return null;
    throw error;
  }
}

// Follow a list-file symlink, even when its target has not been created yet.
function targetOf(file: string): string {
  let target = path.resolve(file);
  for (let i = 0; i < 40; i++) {
    try {
      if (!fs.lstatSync(target).isSymbolicLink()) return target;
      target = path.resolve(path.dirname(target), fs.readlinkSync(target));
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === 'ENOENT') return target;
      throw error;
    }
  }
  throw new Error('Too many symlinks in the list path');
}

/** Replace the observed version, or return the newer version without writing over it. */
export function saveList(file: string, text: string, expected: DiskVersion): SaveResult {
  const target = targetOf(file);
  const dir = path.dirname(target);
  fs.mkdirSync(dir, { recursive: true });
  let mode: number | undefined;
  try { mode = fs.statSync(target).mode & 0o777; }
  catch (error) { if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error; }
  // Snap's home interface forbids hidden files directly in the real home directory.
  const atSnapHome = process.env.SNAP_REAL_HOME && path.resolve(dir) === path.resolve(process.env.SNAP_REAL_HOME);
  const tmp = path.join(dir, `${atSnapHome ? '' : '.'}${path.basename(target)}.pin-${randomUUID()}.tmp`);
  let fd: number | undefined;
  let renamed = false;
  try {
    fd = fs.openSync(tmp, 'wx', mode ?? 0o666);
    if (mode !== undefined) fs.fchmodSync(fd, mode);
    fs.writeFileSync(fd, text);
    fs.fsyncSync(fd);
    fs.closeSync(fd); fd = undefined;
    const disk = readList(file);
    if (disk !== expected || targetOf(file) !== target) return { saved: false, disk };
    fs.renameSync(tmp, target);
    renamed = true;
    return { saved: true };
  } finally {
    if (fd !== undefined) fs.closeSync(fd);
    if (!renamed) {
      try { fs.unlinkSync(tmp); }
      catch (error) { if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error; }
    }
  }
}

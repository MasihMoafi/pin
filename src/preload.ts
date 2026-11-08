import { contextBridge, ipcRenderer } from 'electron';
import { type DiskVersion, type SaveResult } from './storage';

/** The renderer's only door to the file and the window; its type is declared in renderer/host.d.ts. */
contextBridge.exposeInMainWorld('host', {
  read: (): Promise<DiskVersion> => ipcRenderer.invoke('read'),
  write: (text: string, expected: DiskVersion): Promise<SaveResult> => ipcRenderer.invoke('write', text, expected),
  onChanged: (fn: (disk: DiskVersion) => void): void => { ipcRenderer.on('changed', (_e, disk: DiskVersion) => fn(disk)); },
  onStorageError: (fn: () => void): void => { ipcRenderer.on('storage-error', fn); },
  onCloseRequested: (fn: () => void): void => { ipcRenderer.on('request-close', fn); },
  close: (): void => ipcRenderer.send('close'),
  pin: (on: boolean): void => ipcRenderer.send('pin', on),
});

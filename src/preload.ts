import { contextBridge, ipcRenderer } from 'electron';

/** The renderer's only door to the file and the window; its type is declared in renderer/host.d.ts. */
contextBridge.exposeInMainWorld('host', {
  read: (): Promise<string> => ipcRenderer.invoke('read'),
  write: (text: string): Promise<void> => ipcRenderer.invoke('write', text),
  onChanged: (fn: (text: string) => void): void => { ipcRenderer.on('changed', (_e, text: string) => fn(text)); },
  close: (): void => ipcRenderer.send('close'),
  pin: (on: boolean): void => ipcRenderer.send('pin', on),
});

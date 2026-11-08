// What preload.ts exposes to the page.
interface Host {
  read(): Promise<string | null>;
  write(text: string, expected: string | null): Promise<{ saved: true } | { saved: false; disk: string | null }>;
  onChanged(fn: (disk: string | null) => void): void;
  onStorageError(fn: () => void): void;
  onCloseRequested(fn: () => void): void;
  close(): void;
  pin(on: boolean): void;
}
declare const host: Host;

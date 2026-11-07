// What preload.ts exposes to the page.
interface Host {
  read(): Promise<string>;
  write(text: string): Promise<void>;
  onChanged(fn: (text: string) => void): void;
  close(): void;
  pin(on: boolean): void;
}
declare const host: Host;

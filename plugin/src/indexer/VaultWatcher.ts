import { App, Plugin, TFile, TAbstractFile } from 'obsidian';

export type FileChangeCallback = (file: TFile, changeType: 'create' | 'modify' | 'delete' | 'rename') => void;

export class VaultWatcher {
  private app: App;
  private callback: FileChangeCallback;
  private debounceMs: number;
  private pendingChanges: Map<string, { file: TFile; type: 'create' | 'modify' | 'delete' | 'rename' }> = new Map();
  private debounceTimer: number | null = null;
  private enabled: boolean = false;

  constructor(app: App, callback: FileChangeCallback, debounceMs: number = 1000) {
    this.app = app;
    this.callback = callback;
    this.debounceMs = debounceMs;
  }

  register(plugin: Plugin): void {
    plugin.registerEvent(
      this.app.vault.on('create', (file) => {
        if (this.enabled && file instanceof TFile && file.extension === 'md') {
          this.queueChange(file, 'create');
        }
      })
    );

    plugin.registerEvent(
      this.app.vault.on('modify', (file) => {
        if (this.enabled && file instanceof TFile && file.extension === 'md') {
          this.queueChange(file, 'modify');
        }
      })
    );

    plugin.registerEvent(
      this.app.vault.on('delete', (file) => {
        if (this.enabled && file instanceof TFile && file.extension === 'md') {
          this.queueChange(file, 'delete');
        }
      })
    );

    plugin.registerEvent(
      this.app.vault.on('rename', (file, oldPath) => {
        if (this.enabled && file instanceof TFile && file.extension === 'md') {
          const oldFile = { path: oldPath } as TFile;
          this.queueChange(oldFile, 'delete');
          this.queueChange(file, 'create');
        }
      })
    );
  }

  enable(): void {
    this.enabled = true;
    console.log('VaultWatcher enabled');
  }

  disable(): void {
    this.enabled = false;
    this.pendingChanges.clear();
    if (this.debounceTimer !== null) {
      window.clearTimeout(this.debounceTimer);
      this.debounceTimer = null;
    }
  }

  isEnabled(): boolean {
    return this.enabled;
  }

  private queueChange(file: TFile, type: 'create' | 'modify' | 'delete' | 'rename'): void {
    this.pendingChanges.set(file.path, { file, type });

    if (this.debounceTimer !== null) {
      window.clearTimeout(this.debounceTimer);
    }

    this.debounceTimer = window.setTimeout(() => {
      this.processQueue();
    }, this.debounceMs);
  }

  private processQueue(): void {
    const changes = Array.from(this.pendingChanges.values());
    this.pendingChanges.clear();
    this.debounceTimer = null;

    for (const { file, type } of changes) {
      this.callback(file, type);
    }
  }

  setDebounceMs(ms: number): void {
    this.debounceMs = ms;
  }

  flush(): void {
    if (this.debounceTimer !== null) {
      window.clearTimeout(this.debounceTimer);
      this.processQueue();
    }
  }
}

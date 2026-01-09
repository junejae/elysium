import { App, Modal, Notice, Plugin, PluginSettingTab, Setting, TFile, WorkspaceLeaf, SuggestModal, Editor, EditorPosition, EditorSuggest, EditorSuggestContext, EditorSuggestTriggerInfo, MarkdownView } from 'obsidian';
import init, { embed_text, cosine_similarity, get_embedding_dim, HnswIndex } from './wasm-pkg/elysium_wasm';
import wasmBinary from './wasm-pkg/elysium_wasm_bg.wasm';
import { IndexedDbStorage } from './storage/IndexedDbStorage';
import { Indexer } from './indexer/Indexer';
import { VaultWatcher } from './indexer/VaultWatcher';
import { RelatedNotesView, RELATED_NOTES_VIEW_TYPE } from './ui/RelatedNotesView';
import { ElysiumConfig } from './config/ElysiumConfig';
import { SetupWizard } from './ui/SetupWizard';

interface ElysiumSettings {
  autoIndex: boolean;
  indexOnStartup: boolean;
  debounceMs: number;
  showRelatedNotes: boolean;
  debugMode: boolean;
}

const DEFAULT_SETTINGS: ElysiumSettings = {
  autoIndex: true,
  indexOnStartup: true,
  debounceMs: 1000,
  showRelatedNotes: true,
  debugMode: false,
};

class Logger {
  private prefix = '[Elysium]';
  private enabled = false;

  setEnabled(enabled: boolean) {
    this.enabled = enabled;
  }

  debug(component: string, message: string, data?: unknown) {
    if (!this.enabled) return;
    const timestamp = new Date().toISOString().slice(11, 23);
    if (data !== undefined) {
      console.log(`${this.prefix}[${timestamp}][${component}] ${message}`, data);
    } else {
      console.log(`${this.prefix}[${timestamp}][${component}] ${message}`);
    }
  }

  info(component: string, message: string, data?: unknown) {
    const timestamp = new Date().toISOString().slice(11, 23);
    if (data !== undefined) {
      console.log(`${this.prefix}[${timestamp}][${component}] ${message}`, data);
    } else {
      console.log(`${this.prefix}[${timestamp}][${component}] ${message}`);
    }
  }

  error(component: string, message: string, error?: unknown) {
    const timestamp = new Date().toISOString().slice(11, 23);
    console.error(`${this.prefix}[${timestamp}][${component}] ${message}`, error ?? '');
  }
}

export const logger = new Logger();

export default class ElysiumPlugin extends Plugin {
  settings: ElysiumSettings;
  wasmInitialized: boolean = false;
  elysiumConfig: ElysiumConfig;
  
  private storage: IndexedDbStorage | null = null;
  private indexer: Indexer | null = null;
  private watcher: VaultWatcher | null = null;
  private statusBarEl: HTMLElement | null = null;
  private isIndexing: boolean = false;

  async onload() {
    await this.loadSettings();
    logger.setEnabled(this.settings.debugMode);
    logger.info('Plugin', 'Loading Elysium plugin');
    
    await this.loadElysiumConfig();
    
    await this.initializeWasm();
    await this.initializeStorage();
    await this.initializeIndexer();

    this.registerViews();
    this.registerCommands();
    this.registerEditorExtensions();
    this.addSettingTab(new ElysiumSettingTab(this.app, this));
    this.statusBarEl = this.addStatusBarItem();
    this.updateStatusBar();

    if (this.settings.autoIndex) {
      this.setupVaultWatcher();
    }

    this.app.workspace.onLayoutReady(async () => {
      logger.debug('Plugin', 'Layout ready, starting initialization sequence');
      
      if (this.settings.indexOnStartup && this.indexer) {
        logger.debug('Plugin', 'Starting syncOnStartup');
        await this.syncOnStartup();
        logger.debug('Plugin', 'syncOnStartup complete, indexCount:', this.getIndexCount());
      }
      
      if (this.settings.showRelatedNotes) {
        logger.debug('Plugin', 'Activating RelatedNotesView');
        this.activateRelatedNotesView();
      }
    });

    logger.info('Plugin', 'Elysium plugin loaded');
  }

  private async loadElysiumConfig(): Promise<void> {
    this.elysiumConfig = new ElysiumConfig(this.app);
    const exists = await this.elysiumConfig.load();
    
    if (!exists) {
      this.app.workspace.onLayoutReady(() => {
        new SetupWizard(this.app, this.elysiumConfig, () => {
          new Notice('Elysium configured! Run "Reindex Vault" to start.');
        }).open();
      });
    }
  }

  private async initializeStorage(): Promise<void> {
    try {
      this.storage = new IndexedDbStorage();
      await this.storage.open();
      console.log('IndexedDB storage initialized');
    } catch (error) {
      console.error('Failed to initialize IndexedDB:', error);
      new Notice('Failed to initialize Elysium storage');
    }
  }

  private async initializeIndexer(): Promise<void> {
    if (!this.wasmInitialized || !this.storage) {
      console.error('Cannot initialize indexer: WASM or storage not ready');
      return;
    }

    const index = new HnswIndex();
    this.indexer = new Indexer(this.app, this.storage, index, this.elysiumConfig);
    console.log('Indexer initialized');
  }

  private setupVaultWatcher(): void {
    if (!this.indexer) return;

    this.watcher = new VaultWatcher(
      this.app,
      async (file, changeType) => {
        if (!this.indexer) return;

        console.log(`File ${changeType}: ${file.path}`);

        if (changeType === 'delete') {
          await this.indexer.removeFile(file.path);
          this.updateStatusBar();
        } else {
          const indexed = await this.indexer.indexFile(file);
          if (indexed) {
            await this.indexer.persistIndex();
            console.log(`Indexed: ${file.path}`);
          }
          this.updateStatusBar();
        }
      },
      this.settings.debounceMs
    );

    this.watcher.register(this);
    console.log('Vault watcher registered');
  }

  private async syncOnStartup(): Promise<void> {
    if (!this.indexer || !this.storage) return;

    this.setIndexingState(true);
    const startTime = Date.now();
    
    try {
      const restored = await this.indexer.restoreIndex();
      if (restored) {
        const count = this.indexer.getIndex().len();
        console.log(`Restored HNSW index from IndexedDB: ${count} notes`);
      } else {
        console.log('No saved index found, will build from scratch');
      }

      const { added, updated, removed } = await this.indexer.incrementalSync();
      const elapsed = Date.now() - startTime;

      const count = this.indexer.getIndex().len();
      console.log(`Sync complete: +${added} ~${updated} -${removed} = ${count} total (${elapsed}ms)`);

      if (this.watcher) {
        this.watcher.enable();
      }
    } finally {
      this.setIndexingState(false);
    }
  }

  private updateStatusBar(): void {
    if (!this.statusBarEl) return;
    
    const count = this.indexer?.getIndex().len() ?? 0;
    
    let text: string;
    if (!this.wasmInitialized) {
      text = '⚠️ Elysium: Error';
    } else if (this.isIndexing) {
      text = `⟳ Elysium: Indexing...`;
    } else {
      text = `✓ Elysium: ${count} notes`;
    }
    
    this.statusBarEl.setText(text);
  }

  setIndexingState(indexing: boolean): void {
    this.isIndexing = indexing;
    this.updateStatusBar();
  }

  private registerViews() {
    this.registerView(
      RELATED_NOTES_VIEW_TYPE,
      (leaf) => new RelatedNotesView(leaf, this)
    );
  }

  private registerEditorExtensions() {
    this.registerEditorSuggest(new WikilinkSuggest(this.app, this));
  }

  async activateRelatedNotesView() {
    const { workspace } = this.app;

    let leaf = workspace.getLeavesOfType(RELATED_NOTES_VIEW_TYPE)[0];

    if (!leaf) {
      const rightLeaf = workspace.getRightLeaf(false);
      if (rightLeaf) {
        leaf = rightLeaf;
        await leaf.setViewState({
          type: RELATED_NOTES_VIEW_TYPE,
          active: true,
        });
      }
    }

    if (leaf) {
      workspace.revealLeaf(leaf);
      const view = leaf.view;
      if (view instanceof RelatedNotesView) {
        logger.debug('Plugin', 'Triggering RelatedNotesView update');
        view.refresh();
      }
    }
  }

  closeRelatedNotesView() {
    const leaves = this.app.workspace.getLeavesOfType(RELATED_NOTES_VIEW_TYPE);
    for (const leaf of leaves) {
      leaf.detach();
    }
  }

  private registerCommands() {
    this.addCommand({
      id: 'elysium-search',
      name: 'Semantic Search',
      callback: () => new ElysiumSearchModal(this.app, this).open(),
      hotkeys: [{ modifiers: ['Mod', 'Shift'], key: 's' }],
    });

    this.addCommand({
      id: 'elysium-reindex',
      name: 'Reindex Vault',
      callback: async () => {
        if (!this.indexer) {
          new Notice('Indexer not initialized');
          return;
        }
        if (this.isIndexing) {
          new Notice('Already indexing...');
          return;
        }
        this.setIndexingState(true);
        new Notice('Reindexing vault...');
        try {
          const count = await this.indexer.fullReindex();
          new Notice(`Indexed ${count} notes`);
        } finally {
          this.setIndexingState(false);
        }
      },
    });

    this.addCommand({
      id: 'elysium-show-related',
      name: 'Show Related Notes',
      callback: () => this.activateRelatedNotesView(),
    });

    this.addCommand({
      id: 'elysium-quick-switcher',
      name: 'Quick Switcher (Semantic)',
      callback: () => new ElysiumQuickSwitcher(this.app, this).open(),
      hotkeys: [{ modifiers: ['Mod', 'Shift'], key: 'o' }],
    });

    this.addCommand({
      id: 'elysium-test-wasm',
      name: 'Test WASM Embedding',
      callback: () => this.testWasm(),
    });

    this.addCommand({
      id: 'elysium-test-hnsw',
      name: 'Test HNSW Index',
      callback: () => this.testHnsw(),
    });

    this.addCommand({
      id: 'elysium-clear-index',
      name: 'Clear Index',
      callback: async () => {
        if (!this.storage || !this.indexer) return;
        await this.storage.clearAll();
        this.indexer.setIndex(new HnswIndex());
        this.updateStatusBar();
        new Notice('Index cleared');
      },
    });

    this.addCommand({
      id: 'elysium-debug-status',
      name: 'Debug: Show Status',
      callback: () => {
        const indexCount = this.indexer?.getIndex().len() ?? 0;
        const msg = [
          `WASM: ${this.wasmInitialized ? 'OK' : 'Failed'}`,
          `Storage: ${this.storage ? 'OK' : 'Failed'}`,
          `Indexer: ${this.indexer ? 'OK' : 'Failed'}`,
          `Index count: ${indexCount}`,
        ].join('\n');
        new Notice(msg, 10000);
        console.log('Elysium Debug:', { 
          wasm: this.wasmInitialized, 
          storage: !!this.storage, 
          indexer: !!this.indexer,
          indexCount 
        });
      },
    });

    this.addCommand({
      id: 'elysium-open-inbox',
      name: 'Open Inbox',
      callback: async () => {
        if (!this.elysiumConfig.isInboxEnabled()) {
          new Notice('Inbox is disabled in settings');
          return;
        }
        const inboxPath = this.elysiumConfig.getInboxPath();
        const file = this.app.vault.getAbstractFileByPath(inboxPath);
        if (file) {
          await this.app.workspace.openLinkText(inboxPath, '', false);
        } else {
          await this.app.vault.create(inboxPath, '');
          await this.app.workspace.openLinkText(inboxPath, '', false);
          new Notice(`Created ${inboxPath}`);
        }
      },
      hotkeys: [{ modifiers: ['Mod', 'Shift'], key: 'i' }],
    });

    this.addCommand({
      id: 'elysium-quick-capture',
      name: 'Quick Capture',
      callback: () => {
        if (!this.elysiumConfig.isInboxEnabled()) {
          new Notice('Inbox is disabled in settings');
          return;
        }
        new QuickCaptureModal(this.app, this).open();
      },
      hotkeys: [{ modifiers: ['Mod', 'Shift'], key: 'n' }],
    });
  }

  private testWasm() {
    if (!this.wasmInitialized) {
      new Notice('WASM not initialized');
      return;
    }

    const dim = get_embedding_dim();
    const text1 = 'GPU memory sharing kernel optimization';
    const text2 = 'GPU 메모리 공유 커널 최적화';
    const text3 = 'cooking recipes for beginners';

    const emb1 = embed_text(text1);
    const emb2 = embed_text(text2);
    const emb3 = embed_text(text3);

    const sim12 = cosine_similarity(emb1, emb2);
    const sim13 = cosine_similarity(emb1, emb3);

    const msg = [
      `Embedding dim: ${dim}`,
      `Similar: ${(sim12 * 100).toFixed(1)}%`,
      `Different: ${(sim13 * 100).toFixed(1)}%`,
    ].join('\n');

    new Notice(msg, 10000);
    console.log('WASM Test Results:', { dim, sim12, sim13 });
  }

  private testHnsw() {
    if (!this.wasmInitialized) {
      new Notice('WASM not initialized');
      return;
    }

    const testIndex = new HnswIndex();
    
    testIndex.insert_text('doc1', 'GPU memory optimization and CUDA kernels');
    testIndex.insert_text('doc2', 'Machine learning neural networks deep learning');
    testIndex.insert_text('doc3', 'Cooking recipes and kitchen tips');
    testIndex.insert_text('doc4', 'GPU parallel computing and graphics');
    testIndex.insert_text('doc5', 'Baking bread and pastry techniques');

    const results = testIndex.search_text('GPU programming CUDA', 3, 50) as Array<[string, number]>;

    const msg = [
      `HNSW Index: ${testIndex.len()} docs`,
      `Query: "GPU programming CUDA"`,
      `Top 3:`,
      ...results.map(([id, score], i) => `${i + 1}. ${id}: ${(score * 100).toFixed(1)}%`),
    ].join('\n');

    new Notice(msg, 15000);
    console.log('HNSW Test Results:', results);
    
    testIndex.free();
  }

  onunload() {
    console.log('Unloading Elysium plugin');
    this.watcher?.flush();
    this.indexer?.getIndex().free();
    this.storage?.close();
  }

  async loadSettings() {
    this.settings = Object.assign({}, DEFAULT_SETTINGS, await this.loadData());
  }

  async saveSettings() {
    await this.saveData(this.settings);
    
    if (this.watcher) {
      this.watcher.setDebounceMs(this.settings.debounceMs);
    }
  }

  async initializeWasm(): Promise<void> {
    if (this.wasmInitialized) return;
    
    try {
      await init(wasmBinary);
      this.wasmInitialized = true;
      console.log('WASM module initialized, embedding dim:', get_embedding_dim());
    } catch (error) {
      console.error('Failed to initialize WASM:', error);
      new Notice('Failed to initialize Elysium WASM module');
    }
  }

  searchVault(query: string, k: number = 10): Array<{ path: string; score: number }> {
    const index = this.indexer?.getIndex();
    if (!index || index.is_empty()) {
      return [];
    }

    const results = index.search_text(query, k, 50) as Array<[string, number]>;
    return results.map(([path, score]) => ({ path, score }));
  }

  async searchVaultWithGist(query: string, k: number = 10): Promise<Array<{ path: string; score: number; gist: string | null; fields: Record<string, string>; tags?: string[] }>> {
    const results = this.searchVault(query, k);
    if (!this.storage) return results.map(r => ({ ...r, gist: null, fields: {} }));

    const withGist = await Promise.all(
      results.map(async (r) => {
        const note = await this.storage!.getNote(r.path);
        return { ...r, gist: note?.gist ?? null, fields: note?.fields ?? {}, tags: note?.tags };
      })
    );

    return withGist;
  }

  async searchVaultFiltered(
    query: string, 
    filters: Record<string, string | undefined>,
    tag: string | undefined,
    k: number = 10
  ): Promise<Array<{ path: string; score: number; gist: string | null; fields: Record<string, string>; tags?: string[] }>> {
    const rawResults = this.searchVault(query, k * 3);
    if (!this.storage) return [];

    const filtered: Array<{ path: string; score: number; gist: string | null; fields: Record<string, string>; tags?: string[] }> = [];

    for (const r of rawResults) {
      if (filtered.length >= k) break;
      
      const note = await this.storage.getNote(r.path);
      if (!note) continue;

      let match = true;
      for (const [fieldKey, filterValue] of Object.entries(filters)) {
        if (filterValue && note.fields[fieldKey] !== filterValue) {
          match = false;
          break;
        }
      }
      if (!match) continue;
      
      if (tag && (!note.tags || !note.tags.includes(tag))) continue;

      filtered.push({
        ...r,
        gist: note.gist ?? null,
        fields: note.fields,
        tags: note.tags,
      });
    }

    return filtered;
  }

  async getGistForPath(path: string): Promise<string | null> {
    if (!this.storage) return null;
    const note = await this.storage.getNote(path);
    return note?.gist ?? null;
  }

  getIndexCount(): number {
    return this.indexer?.getIndex().len() ?? 0;
  }
}

class ElysiumSearchModal extends Modal {
  plugin: ElysiumPlugin;
  inputEl: HTMLInputElement;
  resultsEl: HTMLElement;
  private selectedIndex: number = 0;
  private resultItems: HTMLElement[] = [];
  private activeFilters: Record<string, string | undefined> = {};

  constructor(app: App, plugin: ElysiumPlugin) {
    super(app);
    this.plugin = plugin;
  }

  onOpen() {
    const { contentEl } = this;
    contentEl.empty();
    contentEl.addClass('elysium-search-modal');

    contentEl.createEl('h2', { text: 'Semantic Search' });

    this.renderFilterButtons(contentEl);

    const inputContainer = contentEl.createDiv({ cls: 'elysium-search-input-container' });
    this.inputEl = inputContainer.createEl('input', {
      type: 'text',
      placeholder: 'Search your vault semantically...',
      cls: 'elysium-search-input',
    });
    this.inputEl.focus();

    this.resultsEl = contentEl.createDiv({ cls: 'elysium-search-results' });
    
    const count = this.plugin.getIndexCount();
    if (!this.plugin.wasmInitialized) {
      this.resultsEl.createEl('p', { 
        text: 'WASM module not initialized',
        cls: 'elysium-search-error'
      });
    } else if (count === 0) {
      this.resultsEl.createEl('p', { 
        text: 'Index empty. Run "Reindex Vault" first.',
        cls: 'elysium-search-placeholder'
      });
    } else {
      this.resultsEl.createEl('p', { 
        text: `${count} notes indexed. Type to search...`,
        cls: 'elysium-search-placeholder'
      });
    }

    this.inputEl.addEventListener('input', () => this.handleInput());
    this.inputEl.addEventListener('keydown', (e) => this.handleKeydown(e));
  }

  private renderFilterButtons(container: HTMLElement) {
    const config = this.plugin.elysiumConfig;
    if (!config) return;

    const filterContainer = container.createDiv({ cls: 'elysium-filter-container' });
    
    const filterConfigs = [
      { key: 'type', label: 'Type', values: config.getTypeValues() },
      { key: 'area', label: 'Area', values: config.getAreaValues() },
    ];

    for (const { key, label, values } of filterConfigs) {
      const row = filterContainer.createDiv({ cls: 'elysium-filter-row' });
      row.createEl('span', { text: `${label}:`, cls: 'elysium-filter-label' });
      
      const buttonGroup = row.createDiv({ cls: 'elysium-filter-group' });
      
      const allBtn = buttonGroup.createEl('button', { text: 'All', cls: 'elysium-filter-btn is-active' });
      allBtn.addEventListener('click', () => this.setFilter(key, undefined, buttonGroup));

      for (const value of values) {
        const btn = buttonGroup.createEl('button', { text: value, cls: 'elysium-filter-btn' });
        btn.addEventListener('click', () => this.setFilter(key, value, buttonGroup));
      }
    }
  }

  private setFilter(fieldKey: string, value: string | undefined, container: HTMLElement) {
    this.activeFilters[fieldKey] = value;
    container.querySelectorAll('.elysium-filter-btn').forEach(btn => btn.removeClass('is-active'));
    if (value) {
      container.querySelectorAll('button').forEach(btn => {
        if (btn.textContent === value) btn.addClass('is-active');
      });
    } else {
      container.querySelector('button')?.addClass('is-active');
    }
    this.handleInput();
  }

  private async handleInput() {
    const query = this.inputEl.value.trim();
    if (query.length < 2) {
      this.resultsEl.empty();
      const count = this.plugin.getIndexCount();
      this.resultsEl.createEl('p', { 
        text: count > 0 ? `${count} notes indexed. Type to search...` : 'Index empty.',
        cls: 'elysium-search-placeholder' 
      });
      return;
    }
    await this.search(query);
  }

  private handleKeydown(e: KeyboardEvent) {
    if (this.resultItems.length === 0) return;

    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault();
        this.selectIndex(this.selectedIndex + 1);
        break;
      case 'ArrowUp':
        e.preventDefault();
        this.selectIndex(this.selectedIndex - 1);
        break;
      case 'Enter':
        e.preventDefault();
        if (this.resultItems[this.selectedIndex]) {
          this.resultItems[this.selectedIndex].click();
        }
        break;
    }
  }

  private selectIndex(index: number) {
    if (this.resultItems.length === 0) return;

    if (index < 0) index = this.resultItems.length - 1;
    if (index >= this.resultItems.length) index = 0;

    this.resultItems[this.selectedIndex]?.removeClass('is-selected');
    this.selectedIndex = index;
    this.resultItems[this.selectedIndex]?.addClass('is-selected');
    this.resultItems[this.selectedIndex]?.scrollIntoView({ block: 'nearest' });
  }

  private parseQuery(input: string): { query: string; filters: Record<string, string>; tag?: string } {
    const filters: Record<string, string> = {};
    let processedInput = input;
    
    const filterKeys = ['type', 'status', 'area'];
    for (const fieldKey of filterKeys) {
      const regex = new RegExp(`${fieldKey}:(\\S+)`, 'g');
      const match = input.match(regex);
      if (match) {
        const valueMatch = match[0].match(new RegExp(`${fieldKey}:(\\S+)`));
        if (valueMatch) filters[fieldKey] = valueMatch[1];
        processedInput = processedInput.replace(regex, '');
      }
    }
    
    const tagMatch = input.match(/tag:(\S+)/);
    const tag = tagMatch ? tagMatch[1] : undefined;
    processedInput = processedInput.replace(/tag:\S+/g, '').trim();

    return { query: processedInput, filters, tag };
  }

  async search(rawQuery: string) {
    this.resultsEl.empty();
    
    if (!this.plugin.wasmInitialized) {
      this.resultsEl.createEl('p', { 
        text: 'WASM module not initialized',
        cls: 'elysium-search-error'
      });
      return;
    }

    this.resultsEl.createEl('p', { 
      text: 'Searching...',
      cls: 'elysium-search-placeholder'
    });

    const { query, filters: textFilters, tag } = this.parseQuery(rawQuery);
    
    const filters: Record<string, string | undefined> = { ...this.activeFilters };
    for (const [key, value] of Object.entries(textFilters)) {
      if (!filters[key]) filters[key] = value;
    }
    
    const hasFilters = Object.values(filters).some(v => v) || !!tag;

    let results: Array<{ path: string; score: number; gist: string | null; fields: Record<string, string>; tags?: string[] }>;
    
    if (hasFilters) {
      results = await this.plugin.searchVaultFiltered(query || 'note', filters, tag, 10);
    } else {
      results = await this.plugin.searchVaultWithGist(query, 10);
    }
    
    this.renderResults(results, hasFilters);
  }

  renderResults(
    results: Array<{ path: string; score: number; gist: string | null; fields?: Record<string, string>; tags?: string[] }>,
    showMeta: boolean = false
  ) {
    this.resultsEl.empty();
    this.resultItems = [];
    this.selectedIndex = 0;

    if (results.length === 0) {
      this.resultsEl.createEl('p', { 
        text: 'No results found',
        cls: 'elysium-search-no-results'
      });
      return;
    }

    for (const result of results) {
      const item = this.resultsEl.createDiv({ cls: 'elysium-result-item' });
      
      const title = result.path.replace(/\.md$/, '').split('/').pop() ?? result.path;
      item.createEl('div', { cls: 'elysium-result-title', text: title });
      
      if (showMeta && result.fields) {
        const metaValues = Object.values(result.fields).filter(Boolean);
        if (metaValues.length > 0) {
          item.createEl('div', { cls: 'elysium-result-meta', text: metaValues.join(' · ') });
        }
      }
      
      const gistText = result.gist 
        ? (result.gist.length > 120 ? result.gist.slice(0, 120) + '...' : result.gist)
        : result.path;
      item.createEl('div', { cls: 'elysium-result-gist', text: gistText });
      item.createEl('div', { cls: 'elysium-result-score', text: `${Math.round(result.score * 100)}%` });

      item.addEventListener('click', () => {
        this.app.workspace.openLinkText(result.path, '', false);
        this.close();
      });

      this.resultItems.push(item);
    }

    if (this.resultItems.length > 0) {
      this.resultItems[0].addClass('is-selected');
    }
  }

  onClose() {
    this.contentEl.empty();
  }
}

class ElysiumSettingTab extends PluginSettingTab {
  plugin: ElysiumPlugin;

  constructor(app: App, plugin: ElysiumPlugin) {
    super(app, plugin);
    this.plugin = plugin;
  }

  display(): void {
    const { containerEl } = this;
    containerEl.empty();

    containerEl.createEl('h2', { text: 'Elysium Settings' });

    new Setting(containerEl)
      .setName('Auto-index on file changes')
      .setDesc('Automatically re-index notes when they are modified')
      .addToggle(toggle => toggle
        .setValue(this.plugin.settings.autoIndex)
        .onChange(async (value) => {
          this.plugin.settings.autoIndex = value;
          await this.plugin.saveSettings();
        }));

    new Setting(containerEl)
      .setName('Index on startup')
      .setDesc('Sync index when Obsidian starts')
      .addToggle(toggle => toggle
        .setValue(this.plugin.settings.indexOnStartup)
        .onChange(async (value) => {
          this.plugin.settings.indexOnStartup = value;
          await this.plugin.saveSettings();
        }));

    new Setting(containerEl)
      .setName('Debounce delay (ms)')
      .setDesc('Wait time before re-indexing after file changes')
      .addText(text => text
        .setPlaceholder('1000')
        .setValue(String(this.plugin.settings.debounceMs))
        .onChange(async (value) => {
          const num = parseInt(value, 10);
          if (!isNaN(num) && num >= 0) {
            this.plugin.settings.debounceMs = num;
            await this.plugin.saveSettings();
          }
        }));

    new Setting(containerEl)
      .setName('Show Related Notes panel')
      .setDesc('Open Related Notes sidebar on startup')
      .addToggle(toggle => toggle
        .setValue(this.plugin.settings.showRelatedNotes)
        .onChange(async (value) => {
          this.plugin.settings.showRelatedNotes = value;
          await this.plugin.saveSettings();
          if (value) {
            this.plugin.activateRelatedNotesView();
          } else {
            this.plugin.closeRelatedNotesView();
          }
        }));

    new Setting(containerEl)
      .setName('Debug mode')
      .setDesc('Enable verbose logging to console for troubleshooting')
      .addToggle(toggle => toggle
        .setValue(this.plugin.settings.debugMode)
        .onChange(async (value) => {
          this.plugin.settings.debugMode = value;
          logger.setEnabled(value);
          await this.plugin.saveSettings();
        }));

    containerEl.createEl('h3', { text: 'Schema' });
    
    const config = this.plugin.elysiumConfig;
    if (config) {
      new Setting(containerEl)
        .setName('Type values')
        .setDesc('Valid type values (comma-separated)')
        .addText(text => text
          .setValue(config.getTypeValues().join(', '))
          .onChange(async (value) => {
            const values = value.split(',').map(v => v.trim()).filter(v => v);
            config.updateTypeValues(values);
            await config.save();
          }));

      new Setting(containerEl)
        .setName('Area values')
        .setDesc('Valid area values (comma-separated)')
        .addText(text => text
          .setValue(config.getAreaValues().join(', '))
          .onChange(async (value) => {
            const values = value.split(',').map(v => v.trim()).filter(v => v);
            config.updateAreaValues(values);
            await config.save();
          }));

      containerEl.createEl('h3', { text: 'Gist' });
      
      const gistDesc = containerEl.createEl('p', { cls: 'setting-item-description' });
      gistDesc.setText('Gist is a short summary (2-3 sentences) stored in frontmatter. It powers semantic search—finding notes by meaning, not just keywords. Without gist, Elysium falls back to filename-based search.');

      const gistConfig = config.getGistConfig();
      
      new Setting(containerEl)
        .setName('Enable Gist')
        .setDesc('Store note summaries in frontmatter for semantic search')
        .addToggle(toggle => toggle
          .setValue(gistConfig.enabled)
          .onChange(async (value) => {
            config.updateGistConfig({ enabled: value });
            await config.save();
            this.display();
          }));

      if (gistConfig.enabled) {
        new Setting(containerEl)
          .setName('Max length')
          .setDesc('Maximum characters for gist field')
          .addText(text => text
            .setValue(String(gistConfig.maxLength))
            .onChange(async (value) => {
              const num = parseInt(value, 10);
              if (!isNaN(num) && num > 0) {
                config.updateGistConfig({ maxLength: num });
                await config.save();
              }
            }));

        const gistNote = containerEl.createEl('p', { cls: 'setting-item-description' });
        gistNote.setText('Gist is filled by AI (via MCP) or written manually. No auto-generation to avoid YAML corruption.');
      }

      containerEl.createEl('h3', { text: 'Inbox' });
      
      const inboxDesc = containerEl.createEl('p', { cls: 'setting-item-description' });
      inboxDesc.setText('Quick capture file for fleeting notes. Use Cmd+Shift+I to open. AI assistants can process inbox via MCP.');

      new Setting(containerEl)
        .setName('Enable Inbox')
        .setDesc('Enable inbox file for quick capture')
        .addToggle(toggle => toggle
          .setValue(config.isInboxEnabled())
          .onChange(async (value) => {
            config.updateInboxConfig({ enabled: value });
            await config.save();
            this.display();
          }));

      if (config.isInboxEnabled()) {
        new Setting(containerEl)
          .setName('Inbox path')
          .setDesc('Path to inbox file (relative to vault root)')
          .addText(text => text
            .setValue(config.getInboxPath())
            .onChange(async (value) => {
              config.updateInboxConfig({ path: value });
              await config.save();
            }));
      }

      containerEl.createEl('h3', { text: 'Folders' });
      
      const foldersDesc = containerEl.createEl('p', { cls: 'setting-item-description' });
      foldersDesc.setText('Folder paths for note organization. MCP uses these settings when creating notes.');

      new Setting(containerEl)
        .setName('Notes folder')
        .setDesc('Folder for note, term, and log types')
        .addText(text => text
          .setValue(config.getNotesFolder())
          .onChange(async (value) => {
            config.updateFoldersConfig({ notes: value });
            await config.save();
          }));

      new Setting(containerEl)
        .setName('Projects folder')
        .setDesc('Folder for active projects')
        .addText(text => text
          .setValue(config.getProjectsFolder())
          .onChange(async (value) => {
            config.updateFoldersConfig({ projects: value });
            await config.save();
          }));

      new Setting(containerEl)
        .setName('Archive folder')
        .setDesc('Folder for completed/archived projects')
        .addText(text => text
          .setValue(config.getArchiveFolder())
          .onChange(async (value) => {
            config.updateFoldersConfig({ archive: value });
            await config.save();
          }));

      containerEl.createEl('h3', { text: 'Setup' });

      new Setting(containerEl)
        .setName('Run Setup Wizard')
        .setDesc('Re-run the initial configuration wizard')
        .addButton(button => button
          .setButtonText('Open Wizard')
          .onClick(() => {
            new SetupWizard(this.app, config, () => {
              this.display();
            }).open();
          }));
    }

    containerEl.createEl('h3', { text: 'Status' });
    
    const indexCount = this.plugin.getIndexCount();
    const statusText = this.plugin.wasmInitialized 
      ? `WASM: OK | Index: ${indexCount} notes | Dim: ${get_embedding_dim()}`
      : 'WASM: Failed';
    containerEl.createEl('p', { text: statusText });

    containerEl.createEl('h3', { text: 'Actions' });

    new Setting(containerEl)
      .setName('Full Reindex')
      .setDesc('Rebuild entire index from vault')
      .addButton(button => button
        .setButtonText('Reindex')
        .onClick(async () => {
          button.setButtonText('Indexing...');
          button.setDisabled(true);
          this.app.commands.executeCommandById('elysium:elysium-reindex');
          setTimeout(() => {
            button.setButtonText('Reindex');
            button.setDisabled(false);
            this.display();
          }, 100);
        }));

    new Setting(containerEl)
      .setName('Clear Index')
      .setDesc('Delete all indexed data')
      .addButton(button => button
        .setButtonText('Clear')
        .setWarning()
        .onClick(async () => {
          this.app.commands.executeCommandById('elysium:elysium-clear-index');
          this.display();
        }));

    containerEl.createEl('h3', { text: 'Debug' });

    new Setting(containerEl)
      .setName('Test WASM')
      .setDesc('Run embedding similarity test')
      .addButton(button => button
        .setButtonText('Test')
        .onClick(() => {
          this.app.commands.executeCommandById('elysium:elysium-test-wasm');
        }));

    new Setting(containerEl)
      .setName('Test HNSW')
      .setDesc('Run HNSW index test')
      .addButton(button => button
        .setButtonText('Test')
        .onClick(() => {
          this.app.commands.executeCommandById('elysium:elysium-test-hnsw');
        }));

    new Setting(containerEl)
      .setName('Show Debug Status')
      .setDesc('Show current initialization status')
      .addButton(button => button
        .setButtonText('Show')
        .onClick(() => {
          this.app.commands.executeCommandById('elysium:elysium-debug-status');
        }));
  }
}

interface QuickSwitchItem {
  path: string;
  title: string;
  score: number;
  gist: string | null;
  type?: string;
  area?: string;
}

class ElysiumQuickSwitcher extends SuggestModal<QuickSwitchItem> {
  plugin: ElysiumPlugin;
  private lastQuery: string = '';
  private cachedResults: QuickSwitchItem[] = [];

  constructor(app: App, plugin: ElysiumPlugin) {
    super(app);
    this.plugin = plugin;
    this.setPlaceholder('Type to search semantically...');
    this.setInstructions([
      { command: '↑↓', purpose: 'navigate' },
      { command: '↵', purpose: 'open' },
      { command: 'esc', purpose: 'dismiss' },
    ]);
  }

  async getSuggestions(query: string): Promise<QuickSwitchItem[]> {
    if (query.length < 2) {
      return this.getRecentFiles();
    }

    if (query === this.lastQuery && this.cachedResults.length > 0) {
      return this.cachedResults;
    }

    this.lastQuery = query;
    
    const results = await this.plugin.searchVaultWithGist(query, 10);
    this.cachedResults = results.map(r => ({
      path: r.path,
      title: r.path.replace(/\.md$/, '').split('/').pop() ?? r.path,
      score: r.score,
      gist: r.gist,
    }));

    return this.cachedResults;
  }

  private getRecentFiles(): QuickSwitchItem[] {
    const recentFiles = this.app.workspace.getLastOpenFiles().slice(0, 10);
    return recentFiles.map(path => ({
      path,
      title: path.replace(/\.md$/, '').split('/').pop() ?? path,
      score: 1,
      gist: null,
    }));
  }

  renderSuggestion(item: QuickSwitchItem, el: HTMLElement) {
    el.addClass('elysium-quick-item');
    
    const titleEl = el.createDiv({ cls: 'elysium-quick-title' });
    titleEl.setText(item.title);

    if (item.gist) {
      const gistEl = el.createDiv({ cls: 'elysium-quick-gist' });
      const truncated = item.gist.length > 80 ? item.gist.slice(0, 80) + '...' : item.gist;
      gistEl.setText(truncated);
    }

    if (item.score < 1) {
      const scoreEl = el.createDiv({ cls: 'elysium-quick-score' });
      scoreEl.setText(`${Math.round(item.score * 100)}%`);
    }
  }

  onChooseSuggestion(item: QuickSwitchItem) {
    this.app.workspace.openLinkText(item.path, '', false);
  }
}

interface WikilinkSuggestion {
  path: string;
  title: string;
  score: number;
  gist: string | null;
}

class WikilinkSuggest extends EditorSuggest<WikilinkSuggestion> {
  plugin: ElysiumPlugin;

  constructor(app: App, plugin: ElysiumPlugin) {
    super(app);
    this.plugin = plugin;
  }

  onTrigger(cursor: EditorPosition, editor: Editor): EditorSuggestTriggerInfo | null {
    const line = editor.getLine(cursor.line);
    const beforeCursor = line.slice(0, cursor.ch);

    const match = beforeCursor.match(/\[\[([^\]]*?)$/);
    if (!match) return null;

    const query = match[1];
    if (query.length < 2) return null;

    return {
      start: { line: cursor.line, ch: cursor.ch - query.length },
      end: cursor,
      query,
    };
  }

  async getSuggestions(context: EditorSuggestContext): Promise<WikilinkSuggestion[]> {
    const query = context.query;
    if (query.length < 2) return [];

    const results = await this.plugin.searchVaultWithGist(query, 5);
    return results.map(r => ({
      path: r.path,
      title: r.path.replace(/\.md$/, '').split('/').pop() ?? r.path,
      score: r.score,
      gist: r.gist,
    }));
  }

  renderSuggestion(item: WikilinkSuggestion, el: HTMLElement) {
    el.addClass('elysium-wikilink-item');

    const titleEl = el.createDiv({ cls: 'elysium-wikilink-title' });
    titleEl.setText(item.title);

    if (item.gist) {
      const gistEl = el.createDiv({ cls: 'elysium-wikilink-gist' });
      const truncated = item.gist.length > 60 ? item.gist.slice(0, 60) + '...' : item.gist;
      gistEl.setText(truncated);
    }

    const scoreEl = el.createDiv({ cls: 'elysium-wikilink-score' });
    scoreEl.setText(`${Math.round(item.score * 100)}%`);
  }

  selectSuggestion(item: WikilinkSuggestion, evt: MouseEvent | KeyboardEvent) {
    const { context } = this;
    if (!context) return;

    const linkText = item.path.replace(/\.md$/, '');
    const replacement = linkText + ']]';

    context.editor.replaceRange(
      replacement,
      context.start,
      context.end
    );
  }
}

class QuickCaptureModal extends Modal {
  plugin: ElysiumPlugin;
  textArea: HTMLTextAreaElement;

  constructor(app: App, plugin: ElysiumPlugin) {
    super(app);
    this.plugin = plugin;
  }

  onOpen() {
    const { contentEl } = this;
    contentEl.empty();
    contentEl.addClass('elysium-quick-capture-modal');

    contentEl.createEl('h2', { text: 'Quick Capture' });

    this.textArea = contentEl.createEl('textarea', {
      cls: 'elysium-quick-capture-input',
      attr: { placeholder: 'Type your memo here...' }
    });
    this.textArea.focus();

    const buttonContainer = contentEl.createDiv({ cls: 'elysium-quick-capture-buttons' });
    
    const cancelBtn = buttonContainer.createEl('button', { text: 'Cancel' });
    cancelBtn.addEventListener('click', () => this.close());

    const saveBtn = buttonContainer.createEl('button', { text: 'Save to Inbox', cls: 'mod-cta' });
    saveBtn.addEventListener('click', () => this.save());

    this.textArea.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        this.save();
      }
    });
  }

  async save() {
    const content = this.textArea.value.trim();
    if (!content) {
      new Notice('Nothing to save');
      return;
    }

    const inboxPath = this.plugin.elysiumConfig.getInboxPath();
    const file = this.app.vault.getAbstractFileByPath(inboxPath);
    
    const separator = '\n---\n\n';
    const newContent = separator + content;

    if (file && file instanceof TFile) {
      const existing = await this.app.vault.read(file);
      await this.app.vault.modify(file, existing + newContent);
    } else {
      await this.app.vault.create(inboxPath, content);
    }

    new Notice('Saved to inbox');
    this.close();
  }

  onClose() {
    this.contentEl.empty();
  }
}

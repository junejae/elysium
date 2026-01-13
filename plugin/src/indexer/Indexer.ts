import { App, TFile } from 'obsidian';
import { HnswIndex } from '../wasm-pkg/elysium_wasm';
import { IndexedDbStorage, NoteRecord } from '../storage/IndexedDbStorage';
import { ElysiumConfig, FIELD_NAMES } from '../config/ElysiumConfig';
import { ModelLoader } from '../embedder/ModelLoader';

const isExcludedPath = (path: string): boolean => {
  return path.split('/').some(part => part.startsWith('.'));
};

export class Indexer {
  private app: App;
  private storage: IndexedDbStorage;
  private index: HnswIndex;
  private config: ElysiumConfig | null;
  private modelLoader: ModelLoader | null = null;
  private useAdvancedSearch: boolean = false;

  constructor(app: App, storage: IndexedDbStorage, index: HnswIndex, config?: ElysiumConfig) {
    this.app = app;
    this.storage = storage;
    this.index = index;
    this.config = config ?? null;
  }

  updateConfig(config: ElysiumConfig): void {
    this.config = config;
  }

  /**
   * Enable advanced search with Model2Vec
   * @param modelPath Path to the model directory
   */
  async enableAdvancedSearch(modelPath: string): Promise<void> {
    if (!this.modelLoader) {
      this.modelLoader = new ModelLoader(this.app);
    }
    await this.modelLoader.loadModel(modelPath);
    this.useAdvancedSearch = true;
    console.log('[Elysium] Advanced search enabled with Model2Vec');
  }

  /**
   * Disable advanced search and switch back to HTP
   */
  disableAdvancedSearch(): void {
    if (this.modelLoader) {
      this.modelLoader.unload();
    }
    this.useAdvancedSearch = false;
    console.log('[Elysium] Advanced search disabled, using HTP');
  }

  /**
   * Check if advanced search is currently active
   */
  isAdvancedSearchEnabled(): boolean {
    return this.useAdvancedSearch && (this.modelLoader?.isLoaded() ?? false);
  }

  /**
   * Get the current embedding mode
   */
  getEmbeddingMode(): 'htp' | 'model2vec' {
    return this.isAdvancedSearchEnabled() ? 'model2vec' : 'htp';
  }

  private filterExcludedFiles(files: TFile[]): TFile[] {
    return files.filter(file => !isExcludedPath(file.path));
  }

  async indexFile(file: TFile): Promise<boolean> {
    const content = await this.app.vault.cachedRead(file);
    const fm = this.extractFrontmatter(content);

    const gistEnabled = this.config?.isGistEnabled() ?? false;
    const searchText = gistEnabled && fm?.gist
      ? fm.gist
      : this.getFilenameAsSearchText(file.path);

    const existing = await this.storage.getNote(file.path);
    const needsUpdate = !existing || existing.gist !== searchText || existing.mtime !== file.stat.mtime;

    if (needsUpdate) {
      this.index.delete(file.path);

      // Use Model2Vec or HTP based on settings
      if (this.isAdvancedSearchEnabled() && this.modelLoader) {
        try {
          const embedding = this.modelLoader.encode(searchText);
          this.index.insert(file.path, Array.from(embedding));
        } catch (e) {
          console.warn(`[Elysium] Model2Vec encode failed for ${file.path}, falling back to HTP:`, e);
          this.index.insert_text(file.path, searchText);
        }
      } else {
        this.index.insert_text(file.path, searchText);
      }

      const record: NoteRecord = {
        path: file.path,
        gist: searchText,
        mtime: file.stat.mtime,
        indexed: true,
        fields: fm?.fields ?? {},
        tags: fm?.tags,
      };

      await this.storage.saveNote(record);
      return true;
    }

    return false;
  }

  private getFilenameAsSearchText(path: string): string {
    const filename = path.replace(/\.md$/, '').split('/').pop() ?? path;
    return filename.replace(/[-_]/g, ' ');
  }

  async removeFile(path: string): Promise<void> {
    this.index.delete(path);
    await this.storage.deleteNote(path);
  }

  async fullReindex(onProgress?: (current: number, total: number) => void): Promise<number> {
    const allFiles = this.app.vault.getMarkdownFiles();
    const files = this.filterExcludedFiles(allFiles);
    let indexed = 0;
    const total = files.length;
    const BATCH_SIZE = 50;

    for (let i = 0; i < files.length; i += BATCH_SIZE) {
      const batch = files.slice(i, i + BATCH_SIZE);
      
      await Promise.all(batch.map(async (file) => {
        const wasIndexed = await this.indexFile(file);
        if (wasIndexed) indexed++;
      }));

      onProgress?.(Math.min(i + BATCH_SIZE, total), total);
      
      await new Promise(r => setTimeout(r, 0));
    }

    await this.persistIndex();
    return indexed;
  }

  async incrementalSync(): Promise<{ added: number; updated: number; removed: number }> {
    const allFiles = this.app.vault.getMarkdownFiles();
    const files = this.filterExcludedFiles(allFiles);
    const storedNotes = await this.storage.getAllNotes();
    let storedByPath = new Map(storedNotes.map(n => [n.path, n]));
    const currentPaths = new Set(files.map(f => f.path));
    const indexSize = this.index.len();
    
    console.log(`[Elysium] Sync: ${allFiles.length} total files, ${files.length} after filter, ${storedNotes.length} stored, ${indexSize} in HNSW`);
    
    if (storedNotes.length > 0 && indexSize === 0) {
      console.log('[Elysium] HNSW index empty but storage has records - rebuilding index');
      await this.storage.clearAll();
      storedByPath = new Map();
    }

    let added = 0;
    let updated = 0;
    let removed = 0;

    const filesToProcess: Array<{ file: TFile; isNew: boolean }> = [];
    
    for (const file of files) {
      const stored = storedByPath.get(file.path);
      
      if (!stored) {
        filesToProcess.push({ file, isNew: true });
      } else if (stored.mtime !== file.stat.mtime) {
        filesToProcess.push({ file, isNew: false });
      }
    }

    const BATCH_SIZE = 50;
    for (let i = 0; i < filesToProcess.length; i += BATCH_SIZE) {
      const batch = filesToProcess.slice(i, i + BATCH_SIZE);
      
      await Promise.all(batch.map(async ({ file, isNew }) => {
        const wasIndexed = await this.indexFile(file);
        if (wasIndexed) {
          if (isNew) added++;
          else updated++;
        }
      }));
      
      await new Promise(r => setTimeout(r, 0));
    }

    for (const stored of storedNotes) {
      if (!currentPaths.has(stored.path)) {
        await this.removeFile(stored.path);
        removed++;
      }
    }

    if (added > 0 || updated > 0 || removed > 0) {
      await this.persistIndex();
    }

    return { added, updated, removed };
  }

  async persistIndex(): Promise<void> {
    const serialized = this.index.serialize();
    await this.storage.saveHnswIndex(serialized);

    // Also export to files for MCP access
    await this.exportToFiles(serialized);
  }

  /**
   * Export index to files for MCP access
   * Files are saved to .obsidian/plugins/elysium/index/
   */
  private async exportToFiles(hnswData: Uint8Array): Promise<void> {
    const indexDir = '.obsidian/plugins/elysium/index';

    try {
      // Ensure directory exists
      if (!await this.app.vault.adapter.exists(indexDir)) {
        await this.app.vault.adapter.mkdir(indexDir);
      }

      // 1. Save HNSW binary
      await this.app.vault.adapter.writeBinary(`${indexDir}/hnsw.bin`, hnswData);

      // 2. Save notes metadata
      const notes = await this.storage.getAllNotes();
      const notesJson = JSON.stringify(notes, null, 2);
      await this.app.vault.adapter.write(`${indexDir}/notes.json`, notesJson);

      // 3. Save meta info
      const meta = {
        embeddingMode: this.getEmbeddingMode(),
        dimension: this.isAdvancedSearchEnabled() ? 256 : 384,
        noteCount: notes.length,
        indexSize: hnswData.length,
        exportedAt: Date.now(),
        version: 1,
      };
      await this.app.vault.adapter.write(`${indexDir}/meta.json`, JSON.stringify(meta, null, 2));

      console.log(`[Elysium] Exported index to files: ${notes.length} notes, ${hnswData.length} bytes`);
    } catch (e) {
      console.error('[Elysium] Failed to export index to files:', e);
    }
  }

  async restoreIndex(): Promise<boolean> {
    const data = await this.storage.loadHnswIndex();
    if (!data) {
      console.log('[Elysium] No HNSW data in storage');
      return false;
    }
    
    console.log(`[Elysium] HNSW data size: ${data.length} bytes`);

    let restored: HnswIndex | undefined;
    try {
      restored = HnswIndex.deserialize(data);
    } catch (e) {
      console.error('[Elysium] Deserialize threw:', e);
      return false;
    }
    
    if (!restored) {
      console.log('[Elysium] Failed to deserialize HNSW data');
      return false;
    }

    const restoredLen = restored.len();
    console.log(`[Elysium] Deserialized index: ${restoredLen} notes`);
    
    if (data.length > 100 && restoredLen === 0) {
      console.warn('[Elysium] Index data appears corrupted (has bytes but 0 notes), clearing...');
      restored.free();
      await this.storage.clearAll();
      return false;
    }

    this.index.free();
    this.index = restored;
    return true;
  }

  /**
   * Search the index with the appropriate embedding model
   * Uses Model2Vec if advanced search is enabled, otherwise HTP
   */
  search(query: string, k: number = 10, ef: number = 50): Array<[string, number]> {
    if (this.index.is_empty()) {
      return [];
    }

    if (this.isAdvancedSearchEnabled() && this.modelLoader) {
      try {
        const embedding = this.modelLoader.encode(query);
        return this.index.search(Array.from(embedding), k, ef) as Array<[string, number]>;
      } catch (e) {
        console.warn('[Elysium] Model2Vec search failed, falling back to HTP:', e);
        return this.index.search_text(query, k, ef) as Array<[string, number]>;
      }
    } else {
      return this.index.search_text(query, k, ef) as Array<[string, number]>;
    }
  }

  getIndex(): HnswIndex {
    return this.index;
  }

  setIndex(index: HnswIndex): void {
    this.index = index;
  }

  extractFrontmatter(content: string): { gist?: string; fields: Record<string, string | string[]>; tags?: string[] } | null {
    const match = content.match(/^---\s*\n([\s\S]*?)\n---/);
    if (!match) return null;

    const frontmatter = match[1];
    const result: { gist?: string; fields: Record<string, string | string[]>; tags?: string[] } = { fields: {} };

    // Extract gist (supports multiline YAML folding)
    const gistBlockRegex = new RegExp(`${FIELD_NAMES.GIST}:\\s*>\\s*\\n([\\s\\S]*?)(?=\\n[a-zA-Z_]+:|$)`);
    const gistBlockMatch = frontmatter.match(gistBlockRegex);
    if (gistBlockMatch) {
      result.gist = gistBlockMatch[1].trim().replace(/\n\s*/g, ' ');
    } else {
      const gistInlineRegex = new RegExp(`${FIELD_NAMES.GIST}:\\s*["']?([^"'\\n]+)["']?`);
      const gistInlineMatch = frontmatter.match(gistInlineRegex);
      if (gistInlineMatch) {
        result.gist = gistInlineMatch[1].trim();
      }
    }

    // Dynamically extract all elysium_* fields
    const elysiumFieldRegex = /^(elysium_\w+):\s*(.*)$/gm;
    let fieldMatch: RegExpExecArray | null;

    while ((fieldMatch = elysiumFieldRegex.exec(frontmatter)) !== null) {
      const fullKey = fieldMatch[1];
      const valueStr = fieldMatch[2].trim();

      // Remove elysium_ prefix for cleaner key names
      const key = fullKey.replace(/^elysium_/, '');

      // Skip gist (handled separately above)
      if (key === 'gist') continue;

      // Parse value: list [...] or string
      const listMatch = valueStr.match(/^\[([^\]]*)\]$/);
      if (listMatch) {
        // List value
        const items = listMatch[1]
          .split(',')
          .map(s => s.trim().replace(/^["']|["']$/g, ''))
          .filter(s => s.length > 0);
        result.fields[key] = items;

        // Also populate tags for backward compatibility
        if (key === 'tags') {
          result.tags = items;
        }
      } else {
        // String value
        const cleaned = valueStr.replace(/^["']|["']$/g, '');
        result.fields[key] = cleaned;
      }
    }

    return result;
  }
}

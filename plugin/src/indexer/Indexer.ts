import { App, TFile } from 'obsidian';
import { HnswIndex } from '../wasm-pkg/elysium_wasm';
import { IndexedDbStorage, NoteRecord } from '../storage/IndexedDbStorage';
import { ElysiumConfig } from '../config/ElysiumConfig';

export interface FieldNames {
  type: string;
  area: string;
  gist: string;
  tags: string;
}

const isExcludedPath = (path: string): boolean => {
  return path.split('/').some(part => part.startsWith('.'));
};

export class Indexer {
  private app: App;
  private storage: IndexedDbStorage;
  private index: HnswIndex;
  private fieldNames: FieldNames;
  private gistEnabled: boolean;

  constructor(app: App, storage: IndexedDbStorage, index: HnswIndex, config?: ElysiumConfig) {
    this.app = app;
    this.storage = storage;
    this.index = index;
    this.gistEnabled = config?.isGistEnabled() ?? false;
    this.fieldNames = config ? {
      type: config.getTypeFieldName(),
      area: config.getAreaFieldName(),
      gist: config.getGistFieldName(),
      tags: config.getTagsFieldName(),
    } : {
      type: 'type',
      area: 'area',
      gist: 'gist',
      tags: 'tags',
    };
  }

  updateConfig(config: ElysiumConfig): void {
    this.gistEnabled = config.isGistEnabled();
    this.fieldNames = {
      type: config.getTypeFieldName(),
      area: config.getAreaFieldName(),
      gist: config.getGistFieldName(),
      tags: config.getTagsFieldName(),
    };
  }

  private filterExcludedFiles(files: TFile[]): TFile[] {
    return files.filter(file => !isExcludedPath(file.path));
  }

  async indexFile(file: TFile): Promise<boolean> {
    const content = await this.app.vault.cachedRead(file);
    const fm = this.extractFrontmatter(content);

    const searchText = this.gistEnabled && fm?.gist 
      ? fm.gist 
      : this.getFilenameAsSearchText(file.path);

    const existing = await this.storage.getNote(file.path);
    const needsUpdate = !existing || existing.gist !== searchText || existing.mtime !== file.stat.mtime;

    if (needsUpdate) {
      this.index.delete(file.path);
      this.index.insert_text(file.path, searchText);

      await this.storage.saveNote({
        path: file.path,
        gist: searchText,
        mtime: file.stat.mtime,
        indexed: true,
        type: fm?.type,
        area: fm?.area,
        tags: fm?.tags,
      });

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
    const storedByPath = new Map(storedNotes.map(n => [n.path, n]));
    const currentPaths = new Set(files.map(f => f.path));

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
  }

  async restoreIndex(): Promise<boolean> {
    const data = await this.storage.loadHnswIndex();
    if (!data) return false;

    const restored = HnswIndex.deserialize(data);
    if (!restored) return false;

    this.index.free();
    this.index = restored;
    return true;
  }

  getIndex(): HnswIndex {
    return this.index;
  }

  setIndex(index: HnswIndex): void {
    this.index = index;
  }

  private extractGist(content: string): string | null {
    const fm = this.extractFrontmatter(content);
    return fm?.gist ?? null;
  }

  extractFrontmatter(content: string): { gist?: string; type?: string; area?: string; tags?: string[] } | null {
    const match = content.match(/^---\s*\n([\s\S]*?)\n---/);
    if (!match) return null;

    const frontmatter = match[1];
    const result: { gist?: string; type?: string; area?: string; tags?: string[] } = {};
    
    const gistField = this.fieldNames.gist;
    const gistBlockRegex = new RegExp(`${gistField}:\\s*>\\s*\\n([\\s\\S]*?)(?=\\n[a-zA-Z_]+:|$)`);
    const gistBlockMatch = frontmatter.match(gistBlockRegex);
    if (gistBlockMatch) {
      result.gist = gistBlockMatch[1].trim().replace(/\n\s*/g, ' ');
    } else {
      const gistInlineRegex = new RegExp(`${gistField}:\\s*["']?([^"'\\n]+)["']?`);
      const gistInlineMatch = frontmatter.match(gistInlineRegex);
      if (gistInlineMatch) {
        result.gist = gistInlineMatch[1].trim();
      }
    }

    const typeField = this.fieldNames.type;
    const typeRegex = new RegExp(`${typeField}:\\s*["']?([^"'\\n\\[\\]]+)["']?`);
    const typeMatch = frontmatter.match(typeRegex);
    if (typeMatch) {
      result.type = typeMatch[1].trim();
    }

    const areaField = this.fieldNames.area;
    const areaRegex = new RegExp(`${areaField}:\\s*["']?([^"'\\n\\[\\]]+)["']?`);
    const areaMatch = frontmatter.match(areaRegex);
    if (areaMatch) {
      result.area = areaMatch[1].trim();
    }

    const tagsField = this.fieldNames.tags;
    const tagsRegex = new RegExp(`${tagsField}:\\s*\\[([^\\]]*)\\]`);
    const tagsMatch = frontmatter.match(tagsRegex);
    if (tagsMatch) {
      result.tags = tagsMatch[1]
        .split(',')
        .map(t => t.trim().replace(/["']/g, ''))
        .filter(t => t.length > 0);
    }

    return result;
  }
}

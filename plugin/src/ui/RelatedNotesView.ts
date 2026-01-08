import { ItemView, WorkspaceLeaf, TFile, MarkdownView } from 'obsidian';
import type ElysiumPlugin from '../main';
import { logger } from '../main';

export const RELATED_NOTES_VIEW_TYPE = 'elysium-related-notes';

export class RelatedNotesView extends ItemView {
  plugin: ElysiumPlugin;
  private contentEl: HTMLElement;
  private currentFile: TFile | null = null;

  constructor(leaf: WorkspaceLeaf, plugin: ElysiumPlugin) {
    super(leaf);
    this.plugin = plugin;
  }

  getViewType(): string {
    return RELATED_NOTES_VIEW_TYPE;
  }

  getDisplayText(): string {
    return 'Related Notes';
  }

  getIcon(): string {
    return 'git-branch';
  }

  async onOpen() {
    logger.debug('RelatedNotes', 'onOpen called');
    const container = this.containerEl.children[1];
    container.empty();
    container.addClass('elysium-related-view');

    const header = container.createDiv({ cls: 'elysium-related-header' });
    header.createEl('h4', { text: 'Related Notes' });

    this.contentEl = container.createDiv({ cls: 'elysium-related-content' });
    this.showPlaceholder('Open a note to see related notes');

    this.registerEvent(
      this.app.workspace.on('active-leaf-change', () => {
        logger.debug('RelatedNotes', 'active-leaf-change event');
        this.updateRelatedNotes();
      })
    );

    this.registerEvent(
      this.app.workspace.on('file-open', () => {
        logger.debug('RelatedNotes', 'file-open event');
        this.updateRelatedNotes();
      })
    );

    this.updateRelatedNotes();
  }

  async onClose() {
    this.contentEl?.empty();
  }

  refresh() {
    logger.debug('RelatedNotes', 'refresh() called, resetting currentFile');
    this.currentFile = null;
    this.updateRelatedNotes();
  }

  private showPlaceholder(text: string) {
    this.contentEl.empty();
    this.contentEl.createEl('p', { 
      text, 
      cls: 'elysium-related-placeholder' 
    });
  }

  private showError(text: string) {
    this.contentEl.empty();
    this.contentEl.createEl('p', { 
      text, 
      cls: 'elysium-related-error' 
    });
  }

  async updateRelatedNotes() {
    logger.debug('RelatedNotes', 'updateRelatedNotes called');
    
    const markdownLeaves = this.app.workspace.getLeavesOfType('markdown');
    logger.debug('RelatedNotes', `Found ${markdownLeaves.length} markdown leaves`);
    
    const mainMarkdownLeaf = markdownLeaves.find(leaf => leaf.getRoot() === this.app.workspace.rootSplit);
    
    if (!mainMarkdownLeaf) {
      logger.debug('RelatedNotes', 'No main markdown leaf found');
      if (!this.currentFile) {
        this.showPlaceholder('Open a note to see related notes');
      }
      return;
    }

    const view = mainMarkdownLeaf.view;
    if (!(view instanceof MarkdownView)) {
      logger.debug('RelatedNotes', 'View is not MarkdownView');
      return;
    }

    const file = view.file;
    if (!file) {
      logger.debug('RelatedNotes', 'No file in view');
      if (!this.currentFile) {
        this.showPlaceholder('No file open');
      }
      return;
    }

    if (this.currentFile?.path === file.path) {
      logger.debug('RelatedNotes', 'Same file, skipping update');
      return;
    }

    logger.debug('RelatedNotes', `Updating for file: ${file.path}`);
    this.currentFile = file;
    await this.findRelatedNotes(file);
  }

  private async findRelatedNotes(file: TFile) {
    logger.debug('RelatedNotes', `findRelatedNotes for: ${file.path}`);
    this.contentEl.empty();

    const indexCount = this.plugin.getIndexCount();
    logger.debug('RelatedNotes', `Index count: ${indexCount}`);
    
    if (indexCount === 0) {
      logger.debug('RelatedNotes', 'Index is empty, showing placeholder');
      this.showPlaceholder('Index empty. Run "Reindex Vault" first.');
      return;
    }

    const content = await this.app.vault.cachedRead(file);
    const gist = this.extractGist(content);
    logger.debug('RelatedNotes', `Extracted gist: ${gist ? gist.slice(0, 50) + '...' : 'null'}`);

    if (!gist) {
      this.showPlaceholder('No gist found in this note');
      return;
    }

    const results = this.plugin.searchVault(gist, 6);
    logger.debug('RelatedNotes', `Search returned ${results.length} results`);
    
    const filtered = results.filter(r => r.path !== file.path).slice(0, 5);
    logger.debug('RelatedNotes', `After filtering: ${filtered.length} results`);

    if (filtered.length === 0) {
      this.showPlaceholder('No related notes found');
      return;
    }

    this.renderResults(filtered);
  }

  private async renderResults(results: Array<{ path: string; score: number }>) {
    this.contentEl.empty();

    for (const result of results) {
      const item = this.contentEl.createDiv({ cls: 'elysium-related-item' });
      
      const title = result.path.replace(/\.md$/, '').split('/').pop() ?? result.path;
      
      const titleEl = item.createDiv({ cls: 'elysium-related-title' });
      titleEl.setText(title);

      const gist = await this.plugin.getGistForPath(result.path);
      if (gist) {
        const gistEl = item.createDiv({ cls: 'elysium-related-gist' });
        const truncated = gist.length > 80 ? gist.slice(0, 80) + '...' : gist;
        gistEl.setText(truncated);
      }

      const scoreEl = item.createDiv({ cls: 'elysium-related-score' });
      scoreEl.setText(`${Math.round(result.score * 100)}%`);

      item.addEventListener('click', async (e: MouseEvent) => {
        e.preventDefault();
        await this.openNote(result.path, e.ctrlKey || e.metaKey);
      });
    }
  }

  private async openNote(path: string, newTab: boolean = false): Promise<void> {
    logger.debug('RelatedNotes', `openNote: ${path}, newTab: ${newTab}`);
    const file = this.app.vault.getAbstractFileByPath(path);
    if (file instanceof TFile) {
      const leaf = this.app.workspace.getLeaf(newTab ? 'tab' : false);
      logger.debug('RelatedNotes', `Got leaf, opening file`);
      await leaf.openFile(file);
      logger.debug('RelatedNotes', `File opened`);
    } else {
      logger.debug('RelatedNotes', `File not found: ${path}`);
    }
  }

  private extractGist(content: string): string | null {
    const match = content.match(/^---\s*\n([\s\S]*?)\n---/);
    if (!match) return null;

    const frontmatter = match[1];
    
    const gistBlockMatch = frontmatter.match(/gist:\s*>\s*\n([\s\S]*?)(?=\n[a-zA-Z_]+:|$)/);
    if (gistBlockMatch) {
      return gistBlockMatch[1].trim().replace(/\n\s*/g, ' ');
    }

    const gistInlineMatch = frontmatter.match(/gist:\s*["']?([^"'\n]+)["']?/);
    if (gistInlineMatch) {
      return gistInlineMatch[1].trim();
    }

    return null;
  }
}

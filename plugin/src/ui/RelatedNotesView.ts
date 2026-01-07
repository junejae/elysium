import { ItemView, WorkspaceLeaf, TFile, MarkdownView } from 'obsidian';
import type ElysiumPlugin from '../main';

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
    const container = this.containerEl.children[1];
    container.empty();
    container.addClass('elysium-related-view');

    const header = container.createDiv({ cls: 'elysium-related-header' });
    header.createEl('h4', { text: 'Related Notes' });

    this.contentEl = container.createDiv({ cls: 'elysium-related-content' });
    this.showPlaceholder('Open a note to see related notes');

    this.registerEvent(
      this.app.workspace.on('active-leaf-change', () => {
        this.updateRelatedNotes();
      })
    );

    this.registerEvent(
      this.app.workspace.on('file-open', () => {
        this.updateRelatedNotes();
      })
    );

    this.updateRelatedNotes();
  }

  async onClose() {
    this.contentEl?.empty();
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
    const activeView = this.app.workspace.getActiveViewOfType(MarkdownView);
    if (!activeView) {
      this.showPlaceholder('Open a note to see related notes');
      this.currentFile = null;
      return;
    }

    const file = activeView.file;
    if (!file) {
      this.showPlaceholder('No file open');
      this.currentFile = null;
      return;
    }

    if (this.currentFile?.path === file.path) {
      return;
    }

    this.currentFile = file;
    await this.findRelatedNotes(file);
  }

  private async findRelatedNotes(file: TFile) {
    this.contentEl.empty();

    const indexCount = this.plugin.getIndexCount();
    if (indexCount === 0) {
      this.showPlaceholder('Index empty. Run "Reindex Vault" first.');
      return;
    }

    const content = await this.app.vault.cachedRead(file);
    const gist = this.extractGist(content);

    if (!gist) {
      this.showPlaceholder('No gist found in this note');
      return;
    }

    const results = this.plugin.searchVault(gist, 6);
    const filtered = results.filter(r => r.path !== file.path).slice(0, 5);

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

      item.addEventListener('click', () => {
        this.app.workspace.openLinkText(result.path, '', false);
      });
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

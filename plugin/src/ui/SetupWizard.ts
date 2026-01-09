import { App, Modal, Setting, Notice, PluginManifest } from 'obsidian';
import { VaultScanner, VaultAnalysis, SchemaRecommendation } from '../config/VaultScanner';
import { MigrationEngine, MigrationPlan, MigrationProgress } from '../migration/MigrationEngine';
import { ElysiumConfig, GistConfig, FIELD_NAMES, AdvancedSemanticSearchConfig } from '../config/ElysiumConfig';
import { ModelDownloader, DownloadProgress } from '../config/ModelDownloader';

type WizardStep = 'welcome' | 'analyzing' | 'review' | 'mapping' | 'preview' | 'migrating' | 'advanced' | 'inbox' | 'complete';

const GIST_DESCRIPTION = 'Gist is a short summary (2-3 sentences) stored in frontmatter. It powers semantic search—finding notes by meaning, not just keywords. Without gist, Elysium falls back to filename-based search.';

export class SetupWizard extends Modal {
  private config: ElysiumConfig;
  private scanner: VaultScanner;
  private analysis: VaultAnalysis | null = null;
  private recommendation: SchemaRecommendation | null = null;
  private migrationPlan: MigrationPlan | null = null;
  private currentStep: WizardStep = 'welcome';
  private onComplete: () => void;
  private skipMigration: boolean = false;
  private gistSettings: GistConfig;
  private inboxPath: string;
  private manifest: PluginManifest;
  private modelDownloader: ModelDownloader;
  private advancedSearchSettings: AdvancedSemanticSearchConfig;

  constructor(app: App, config: ElysiumConfig, manifest: PluginManifest, onComplete: () => void) {
    super(app);
    this.config = config;
    this.manifest = manifest;
    this.scanner = new VaultScanner(app);
    this.onComplete = onComplete;
    this.gistSettings = { ...config.getGistConfig() };
    this.inboxPath = config.getInboxPath();
    this.modelDownloader = new ModelDownloader(app, manifest);
    this.advancedSearchSettings = { ...config.getAdvancedSemanticSearchConfig() };
  }

  onOpen() {
    this.modalEl.addClass('elysium-wizard-modal');
    this.renderStep();
  }

  onClose() {
    this.contentEl.empty();
  }

  private renderStep() {
    this.contentEl.empty();
    this.contentEl.addClass('elysium-wizard');

    switch (this.currentStep) {
      case 'welcome':
        this.renderWelcome();
        break;
      case 'analyzing':
        this.renderAnalyzing();
        break;
      case 'review':
        this.renderReview();
        break;
      case 'mapping':
        this.renderMapping();
        break;
      case 'preview':
        this.renderPreview();
        break;
      case 'migrating':
        this.renderMigrating();
        break;
      case 'advanced':
        this.renderAdvanced();
        break;
      case 'inbox':
        this.renderInbox();
        break;
      case 'complete':
        this.renderComplete();
        break;
    }
  }

  private renderWelcome() {
    const { contentEl } = this;

    contentEl.createEl('h2', { text: 'Welcome to Elysium' });
    
    const intro = contentEl.createDiv({ cls: 'elysium-wizard-intro' });
    intro.createEl('p', { 
      text: 'Elysium adds semantic search to your Obsidian vault. It works by analyzing the "gist" (summary) field in your note frontmatter.'
    });

    const features = intro.createEl('ul');
    features.createEl('li', { text: 'Semantic search - find notes by meaning, not just keywords' });
    features.createEl('li', { text: 'Related notes - automatically discover connections' });
    features.createEl('li', { text: 'Smart filters - filter by type, area, or tags' });

    intro.createEl('p', { 
      text: "Let's analyze your vault to understand your current setup and recommend a schema.",
      cls: 'elysium-wizard-highlight'
    });

    const buttonContainer = contentEl.createDiv({ cls: 'elysium-wizard-buttons' });
    
    const analyzeBtn = buttonContainer.createEl('button', { 
      text: 'Analyze My Vault',
      cls: 'mod-cta'
    });
    analyzeBtn.addEventListener('click', () => {
      this.currentStep = 'analyzing';
      this.renderStep();
      this.startAnalysis();
    });

    const skipBtn = buttonContainer.createEl('button', { text: 'Skip & Use Defaults' });
    skipBtn.addEventListener('click', () => {
      this.skipMigration = true;
      this.currentStep = 'complete';
      this.renderStep();
    });
  }

  private renderAnalyzing() {
    const { contentEl } = this;

    contentEl.createEl('h2', { text: 'Analyzing Your Vault' });
    
    const progress = contentEl.createDiv({ cls: 'elysium-wizard-progress' });
    progress.createEl('div', { cls: 'elysium-wizard-spinner' });
    
    const statusEl = progress.createEl('p', { text: 'Scanning notes and analyzing frontmatter patterns...' });
    this.analysisStatusEl = statusEl;
  }

  private analysisStatusEl: HTMLElement | null = null;

  private async startAnalysis() {
    try {
      this.analysis = await this.scanner.analyzeVault(200);
      this.recommendation = this.scanner.generateRecommendation(this.analysis);
      
      this.currentStep = 'review';
      this.renderStep();
    } catch (e) {
      console.error('Analysis failed:', e);
      new Notice('Failed to analyze vault');
      this.currentStep = 'welcome';
      this.renderStep();
    }
  }

  private renderReview() {
    const { contentEl } = this;
    if (!this.analysis || !this.recommendation) return;

    contentEl.createEl('h2', { text: 'Vault Analysis Results' });

    const stats = contentEl.createDiv({ cls: 'elysium-wizard-stats' });
    
    const healthEl = stats.createDiv({ cls: 'elysium-wizard-health' });
    healthEl.createEl('span', { text: 'Health Score: ', cls: 'label' });
    healthEl.createEl('span', { 
      text: `${this.analysis.healthScore}/100`,
      cls: `score ${this.analysis.healthScore >= 70 ? 'good' : this.analysis.healthScore >= 40 ? 'fair' : 'poor'}`
    });

    const statsGrid = stats.createDiv({ cls: 'elysium-wizard-stats-grid' });
    this.createStatItem(statsGrid, 'Total Notes', this.analysis.totalFiles.toString());
    this.createStatItem(statsGrid, 'With Frontmatter', 
      `${this.analysis.filesWithFrontmatter} (${Math.round(this.analysis.frontmatterCoverage * 100)}%)`);
    this.createStatItem(statsGrid, 'Fields Detected', this.analysis.fields.size.toString());

    if (this.analysis.issues.length > 0) {
      const issuesEl = contentEl.createDiv({ cls: 'elysium-wizard-issues' });
      issuesEl.createEl('h4', { text: 'Issues Detected' });
      const issuesList = issuesEl.createEl('ul');
      for (const issue of this.analysis.issues) {
        issuesList.createEl('li', { text: issue });
      }
    }

    contentEl.createEl('h3', { text: 'Schema Recommendation' });
    
    const recEl = contentEl.createDiv({ cls: 'elysium-wizard-recommendation' });
    
    this.renderFieldRecommendation(recEl, 'Type Field', this.recommendation.typeField, FIELD_NAMES.TYPE);
    this.renderFieldRecommendation(recEl, 'Area Field', this.recommendation.areaField, FIELD_NAMES.AREA);
    this.renderGistSection(recEl, this.recommendation.gistField);

    const migrationSummary = contentEl.createDiv({ cls: 'elysium-wizard-migration-summary' });
    migrationSummary.createEl('h4', { text: 'Migration Summary' });

    const { migrationStats } = this.recommendation;
    const totalChanges = migrationStats.notesNeedingTypeUpdate +
                        migrationStats.notesNeedingAreaUpdate +
                        migrationStats.notesNeedingNewFrontmatter;

    if (totalChanges === 0) {
      migrationSummary.createEl('p', {
        text: '✓ Your vault is already well-structured! No migration needed.',
        cls: 'elysium-wizard-success'
      });
    } else {
      const changesList = migrationSummary.createEl('ul');
      if (migrationStats.notesNeedingNewFrontmatter > 0) {
        changesList.createEl('li', {
          text: `${migrationStats.notesNeedingNewFrontmatter} notes need frontmatter added`
        });
      }
      if (migrationStats.notesNeedingTypeUpdate > 0) {
        changesList.createEl('li', {
          text: `${migrationStats.notesNeedingTypeUpdate} notes need type field update`
        });
      }
    }

    // Show empty gist info separately
    if (this.gistSettings.enabled && migrationStats.notesNeedingGistGeneration > 0) {
      migrationSummary.createEl('p', {
        text: `ℹ️ ${migrationStats.notesNeedingGistGeneration} notes have empty gist (can be filled by AI or human later)`,
        cls: 'elysium-wizard-info'
      });
    }

    const buttonContainer = contentEl.createDiv({ cls: 'elysium-wizard-buttons' });

    const backBtn = buttonContainer.createEl('button', { text: 'Back' });
    backBtn.addEventListener('click', () => {
      this.currentStep = 'welcome';
      this.renderStep();
    });

    if (this.recommendation.typeField.needsMigration || this.recommendation.areaField.needsMigration) {
      const mappingBtn = buttonContainer.createEl('button', { text: 'Customize Mapping' });
      mappingBtn.addEventListener('click', () => {
        this.currentStep = 'mapping';
        this.renderStep();
      });
    }

    if (totalChanges > 0) {
      const previewBtn = buttonContainer.createEl('button', { 
        text: 'Preview Changes',
        cls: 'mod-cta'
      });
      previewBtn.addEventListener('click', () => {
        this.currentStep = 'preview';
        this.renderStep();
        this.generatePreview();
      });
    } else {
      const finishBtn = buttonContainer.createEl('button', { 
        text: 'Finish Setup',
        cls: 'mod-cta'
      });
      finishBtn.addEventListener('click', () => this.finishSetup());
    }

    const skipBtn = buttonContainer.createEl('button', { text: 'Skip Migration' });
    skipBtn.addEventListener('click', () => {
      this.skipMigration = true;
      this.finishSetup();
    });
  }

  private createStatItem(container: HTMLElement, label: string, value: string) {
    const item = container.createDiv({ cls: 'stat-item' });
    item.createEl('span', { text: label, cls: 'stat-label' });
    item.createEl('span', { text: value, cls: 'stat-value' });
  }

  private renderFieldRecommendation(
    container: HTMLElement, 
    label: string, 
    rec: SchemaRecommendation['typeField'] | SchemaRecommendation['areaField'],
    fieldName: string
  ) {
    const fieldEl = container.createDiv({ cls: 'elysium-wizard-field-rec' });
    fieldEl.createEl('h4', { text: label });

    if (rec.existingField) {
      fieldEl.createEl('p', { text: `Existing: "${rec.existingField}" with ${rec.existingValues.length} unique values` });
      
      if (rec.needsMigration) {
        fieldEl.createEl('p', { 
          text: `→ Will migrate to "${fieldName}" with ${rec.recommendedValues.length} values`,
          cls: 'recommendation'
        });
        
        const valuesEl = fieldEl.createDiv({ cls: 'values-list' });
        valuesEl.createEl('span', { text: 'Values: ' });
        valuesEl.createEl('span', { text: rec.recommendedValues.join(', '), cls: 'values' });
      } else {
        fieldEl.createEl('p', { text: '✓ Already well-structured', cls: 'success' });
      }
    } else {
      fieldEl.createEl('p', { text: 'Not found in your vault' });
      fieldEl.createEl('p', { 
        text: `→ Will add "${fieldName}" field`,
        cls: 'recommendation'
      });
    }
  }

  private renderGistSection(container: HTMLElement, rec: SchemaRecommendation['gistField']) {
    const fieldEl = container.createDiv({ cls: 'elysium-wizard-field-rec elysium-wizard-gist-section' });
    fieldEl.createEl('h4', { text: 'Gist Field' });

    const descEl = fieldEl.createEl('p', { cls: 'elysium-wizard-gist-desc' });
    descEl.setText(GIST_DESCRIPTION);

    new Setting(fieldEl)
      .setName('Enable Gist')
      .setDesc('Store note summaries in frontmatter for semantic search')
      .addToggle(toggle => {
        toggle.setValue(this.gistSettings.enabled);
        toggle.onChange(value => {
          this.gistSettings.enabled = value;
          this.renderGistOptions(optionsEl, rec);
        });
      });

    const optionsEl = fieldEl.createDiv({ cls: 'elysium-wizard-gist-options' });
    this.renderGistOptions(optionsEl, rec);
  }

  private renderGistOptions(container: HTMLElement, rec: SchemaRecommendation['gistField']) {
    container.empty();

    if (!this.gistSettings.enabled) {
      container.createEl('p', {
        text: 'Semantic search will use filenames instead of summaries.',
        cls: 'elysium-wizard-gist-disabled-note'
      });
      return;
    }

    if (rec.existingField) {
      container.createEl('p', { text: `Found existing field: "${rec.existingField}"` });
      if (rec.existingField !== FIELD_NAMES.GIST) {
        container.createEl('p', {
          text: `→ Will migrate to "${FIELD_NAMES.GIST}"`,
          cls: 'recommendation'
        });
      }
      if (rec.notesWithoutGist > 0) {
        container.createEl('p', {
          text: `${rec.notesWithoutGist} notes missing gist (AI or human can fill later)`,
          cls: 'recommendation'
        });
      }
    }

    new Setting(container)
      .setName('Max length')
      .setDesc('Maximum characters for gist field')
      .addText(text => {
        text.setValue(String(this.gistSettings.maxLength));
        text.onChange(value => {
          const num = parseInt(value, 10);
          if (!isNaN(num) && num > 0) {
            this.gistSettings.maxLength = num;
          }
        });
      });

    container.createEl('p', {
      text: 'Gist will be filled by AI (via MCP) or written manually. No auto-generation to avoid YAML issues.',
      cls: 'elysium-wizard-gist-note'
    });
  }

  private renderMapping() {
    const { contentEl } = this;
    if (!this.recommendation) return;

    contentEl.createEl('h2', { text: 'Customize Value Mapping' });
    contentEl.createEl('p', { 
      text: 'Map your existing values to the standardized schema. Your original values will be preserved.',
      cls: 'elysium-wizard-desc'
    });

    if (this.recommendation.typeField.needsMigration) {
      this.renderMappingTable(contentEl, 'Type', this.recommendation.typeField);
    }

    if (this.recommendation.areaField.needsMigration) {
      this.renderMappingTable(contentEl, 'Area', this.recommendation.areaField);
    }

    const buttonContainer = contentEl.createDiv({ cls: 'elysium-wizard-buttons' });

    const backBtn = buttonContainer.createEl('button', { text: 'Back' });
    backBtn.addEventListener('click', () => {
      this.currentStep = 'review';
      this.renderStep();
    });

    const previewBtn = buttonContainer.createEl('button', { 
      text: 'Preview Changes',
      cls: 'mod-cta'
    });
    previewBtn.addEventListener('click', () => {
      this.currentStep = 'preview';
      this.renderStep();
      this.generatePreview();
    });
  }

  private renderMappingTable(
    container: HTMLElement, 
    label: string,
    rec: SchemaRecommendation['typeField'] | SchemaRecommendation['areaField']
  ) {
    const section = container.createDiv({ cls: 'elysium-wizard-mapping-section' });
    section.createEl('h3', { text: `${label} Mapping` });

    for (const existingValue of rec.existingValues) {
      const currentMapping = rec.valueMapping.get(existingValue) ?? rec.recommendedValues[0];
      
      new Setting(section)
        .setName(`"${existingValue}"`)
        .setDesc(`Map to:`)
        .addDropdown(dropdown => {
          for (const recValue of rec.recommendedValues) {
            dropdown.addOption(recValue, recValue);
          }
          dropdown.setValue(currentMapping);
          dropdown.onChange(value => {
            rec.valueMapping.set(existingValue, value);
          });
        });
    }
  }

  private renderPreview() {
    const { contentEl } = this;

    contentEl.createEl('h2', { text: 'Preview Changes' });
    
    this.previewProgressEl = contentEl.createDiv({ cls: 'elysium-wizard-progress' });
    this.previewProgressEl.createEl('div', { cls: 'elysium-wizard-spinner' });
    this.previewStatusEl = this.previewProgressEl.createEl('p', { text: 'Generating migration preview...' });
    this.previewContentEl = contentEl.createDiv({ cls: 'elysium-wizard-preview-content' });
  }

  private previewProgressEl: HTMLElement | null = null;
  private previewStatusEl: HTMLElement | null = null;
  private previewContentEl: HTMLElement | null = null;

  private async generatePreview() {
    if (!this.recommendation) {
      console.error('SetupWizard: No recommendation available');
      return;
    }

    try {
      console.log('SetupWizard: Starting migration preview...');
      const engine = new MigrationEngine(this.app, this.recommendation, this.gistSettings);
      
      this.migrationPlan = await engine.createMigrationPlan((progress) => {
        if (this.previewStatusEl) {
          this.previewStatusEl.setText(`Analyzing ${progress.current}/${progress.total}: ${progress.currentFile}`);
        }
      });

      console.log('SetupWizard: Migration plan created', this.migrationPlan);
      this.renderPreviewResults();
    } catch (e) {
      console.error('SetupWizard: Failed to generate preview', e);
      if (this.previewStatusEl) {
        this.previewStatusEl.setText(`Error: ${e instanceof Error ? e.message : 'Unknown error'}`);
      }
      new Notice(`Failed to generate preview: ${e instanceof Error ? e.message : 'Unknown error'}`);
    }
  }

  private renderPreviewResults() {
    if (!this.migrationPlan || !this.previewContentEl) {
      console.error('SetupWizard: Missing required elements for preview');
      return;
    }

    this.previewProgressEl?.remove();
    const container = this.previewContentEl;
    container.empty();

    const { summary, filesToModify } = this.migrationPlan;

    const summaryEl = container.createDiv({ cls: 'elysium-wizard-preview-summary' });
    summaryEl.createEl('h3', { text: 'Changes Summary' });
    
    const summaryList = summaryEl.createEl('ul');
    if (summary.addFrontmatter > 0) {
      summaryList.createEl('li', { text: `Add frontmatter to ${summary.addFrontmatter} files` });
    }
    if (summary.addType > 0) {
      summaryList.createEl('li', { text: `Add type field to ${summary.addType} files` });
    }
    if (summary.updateType > 0) {
      summaryList.createEl('li', { text: `Update type field in ${summary.updateType} files` });
    }

    summaryEl.createEl('p', { 
      text: `Total: ${filesToModify.length} files will be modified`,
      cls: 'elysium-wizard-total'
    });

    if (filesToModify.length > 0) {
      const detailsEl = container.createDiv({ cls: 'elysium-wizard-preview-details' });
      detailsEl.createEl('h4', { text: 'Sample Changes (first 5 files)' });

      const table = detailsEl.createEl('div', { cls: 'elysium-wizard-preview-table' });
      
      for (const mod of filesToModify.slice(0, 5)) {
        const row = table.createDiv({ cls: 'preview-row' });
        
        const fileName = mod.path.split('/').pop() ?? mod.path;
        row.createEl('span', { text: fileName, cls: 'file-path' });
        
        const fieldNames = mod.changes.map(c => c.field).join(', ');
        const addCount = mod.changes.filter(c => c.action === 'add').length;
        const updateCount = mod.changes.filter(c => c.action === 'update').length;
        
        const changesEl = row.createDiv({ cls: 'changes' });
        if (addCount > 0) {
          changesEl.createEl('span', { 
            text: `+${addCount} field${addCount > 1 ? 's' : ''}`, 
            cls: 'change add' 
          });
        }
        if (updateCount > 0) {
          changesEl.createEl('span', { 
            text: `~${updateCount} field${updateCount > 1 ? 's' : ''}`, 
            cls: 'change update' 
          });
        }
      }

      if (filesToModify.length > 5) {
        detailsEl.createEl('p', { 
          text: `... and ${filesToModify.length - 5} more files`,
          cls: 'more-files'
        });
      }
    }

    const buttonContainer = container.createDiv({ cls: 'elysium-wizard-buttons' });

    const backBtn = buttonContainer.createEl('button', { text: 'Back' });
    backBtn.addEventListener('click', () => {
      this.currentStep = 'review';
      this.renderStep();
    });

    if (filesToModify.length > 0) {
      const migrateBtn = buttonContainer.createEl('button', { 
        text: `Apply Changes (${filesToModify.length} files)`,
        cls: 'mod-cta mod-warning'
      });
      migrateBtn.addEventListener('click', () => {
        this.currentStep = 'migrating';
        this.renderStep();
        this.executeMigration();
      });
    }

    const skipBtn = buttonContainer.createEl('button', { text: 'Skip & Finish' });
    skipBtn.addEventListener('click', () => {
      this.skipMigration = true;
      this.finishSetup();
    });
  }

  private renderMigrating() {
    const { contentEl } = this;

    contentEl.createEl('h2', { text: 'Applying Changes' });
    
    const progress = contentEl.createDiv({ cls: 'elysium-wizard-progress' });
    progress.createEl('div', { cls: 'elysium-wizard-spinner' });
    this.migrationStatusEl = progress.createEl('p', { text: 'Starting migration...' });
    
    const progressBar = progress.createDiv({ cls: 'elysium-wizard-progress-bar' });
    this.progressBarFill = progressBar.createDiv({ cls: 'fill' });
  }

  private migrationStatusEl: HTMLElement | null = null;
  private progressBarFill: HTMLElement | null = null;

  private async executeMigration() {
    if (!this.recommendation || !this.migrationPlan) return;

    const engine = new MigrationEngine(this.app, this.recommendation, this.gistSettings);
    
    const result = await engine.executeMigration(this.migrationPlan, (progress) => {
      if (this.migrationStatusEl) {
        this.migrationStatusEl.setText(`${progress.current}/${progress.total}: ${progress.currentFile}`);
      }
      if (this.progressBarFill) {
        const percent = (progress.current / progress.total) * 100;
        this.progressBarFill.style.width = `${percent}%`;
      }
    });

    if (result.errors.length > 0) {
      console.error('Migration errors:', result.errors);
    }

    await this.finishSetup();
  }

  private async finishSetup() {
    if (this.recommendation) {
      this.config.updateTypeValues(this.recommendation.typeField.recommendedValues);
      this.config.updateAreaValues(this.recommendation.areaField.recommendedValues);
    }

    this.config.updateGistConfig(this.gistSettings);

    try {
      await this.config.save();
    } catch (e) {
      console.error('Failed to save config:', e);
    }

    this.currentStep = 'advanced';
    this.renderStep();
  }

  private advancedProgressEl: HTMLElement | null = null;
  private advancedToggle: any = null;

  private renderAdvanced() {
    const { contentEl } = this;

    contentEl.createEl('h2', { text: 'Advanced Semantic Search' });

    const intro = contentEl.createDiv({ cls: 'elysium-wizard-intro' });
    intro.createEl('p', {
      text: 'Enable Model2Vec for improved semantic search accuracy and tag recommendations. This requires downloading a ~8MB model file.'
    });

    // Feature comparison table
    const comparisonEl = contentEl.createDiv({ cls: 'elysium-wizard-comparison' });
    comparisonEl.createEl('h4', { text: 'Feature Comparison' });

    const table = comparisonEl.createEl('table');
    const thead = table.createEl('thead');
    const headerRow = thead.createEl('tr');
    headerRow.createEl('th', { text: '' });
    headerRow.createEl('th', { text: 'Basic (HTP)' });
    headerRow.createEl('th', { text: 'Advanced (Model2Vec)' });

    const tbody = table.createEl('tbody');
    const features = [
      ['Model Size', '0 MB (built-in)', '~8 MB download'],
      ['Embedding Quality', 'Good', 'Better (neural network)'],
      ['Speed', 'Fast (~1.5ms)', 'Very Fast (~0.04ms)'],
      ['Tag Recommendations', 'No', 'Yes'],
      ['Multilingual', 'Limited', '101 languages'],
    ];

    for (const [feature, basic, advanced] of features) {
      const row = tbody.createEl('tr');
      row.createEl('td', { text: feature });
      row.createEl('td', { text: basic });
      row.createEl('td', { text: advanced });
    }

    // Toggle setting
    const toggleSetting = new Setting(contentEl)
      .setName('Enable Advanced Semantic Search')
      .setDesc('Uses Model2Vec (potion-base-8M) for better semantic understanding');

    toggleSetting.addToggle(toggle => {
      this.advancedToggle = toggle;
      toggle.setValue(this.advancedSearchSettings.enabled);
      toggle.onChange(async (value) => {
        this.advancedSearchSettings.enabled = value;
        if (value && !this.advancedSearchSettings.modelDownloaded) {
          await this.downloadAdvancedModel();
        }
      });
    });

    // Progress container
    this.advancedProgressEl = contentEl.createDiv({ cls: 'elysium-wizard-download-progress' });
    this.advancedProgressEl.style.display = 'none';

    // Model status
    const statusEl = contentEl.createDiv({ cls: 'elysium-wizard-model-status' });
    if (this.advancedSearchSettings.modelDownloaded) {
      statusEl.createEl('p', {
        text: '✓ Model already downloaded',
        cls: 'elysium-wizard-success'
      });
    }

    // Note about reindexing
    const noteEl = contentEl.createDiv({ cls: 'elysium-wizard-note' });
    noteEl.createEl('p', {
      text: 'Note: Enabling or disabling this feature will require reindexing your vault.',
      cls: 'elysium-wizard-info'
    });

    const buttonContainer = contentEl.createDiv({ cls: 'elysium-wizard-buttons' });

    const continueBtn = buttonContainer.createEl('button', {
      text: 'Continue',
      cls: 'mod-cta'
    });
    continueBtn.addEventListener('click', () => this.saveAdvancedAndContinue());

    const skipBtn = buttonContainer.createEl('button', { text: 'Skip (Use Basic)' });
    skipBtn.addEventListener('click', () => {
      this.advancedSearchSettings.enabled = false;
      this.saveAdvancedAndContinue();
    });
  }

  private async downloadAdvancedModel(): Promise<void> {
    if (!this.advancedProgressEl) return;

    this.advancedProgressEl.style.display = 'block';
    this.advancedProgressEl.empty();

    const progressBar = this.advancedProgressEl.createDiv({ cls: 'elysium-wizard-progress-bar' });
    const fill = progressBar.createDiv({ cls: 'fill' });
    const statusText = this.advancedProgressEl.createEl('p', { text: 'Downloading model...' });

    try {
      const modelPath = await this.modelDownloader.downloadModel((progress: DownloadProgress) => {
        const overallPercent = ((progress.currentFile - 1) / progress.totalFiles * 100) +
          (progress.percent / progress.totalFiles);
        fill.style.width = `${overallPercent}%`;
        statusText.setText(`Downloading ${progress.file}... (${progress.currentFile}/${progress.totalFiles})`);
      });

      this.advancedSearchSettings.modelDownloaded = true;
      this.advancedSearchSettings.modelPath = modelPath;

      fill.style.width = '100%';
      statusText.setText('✓ Model downloaded successfully!');
      statusText.addClass('elysium-wizard-success');
    } catch (e) {
      console.error('Failed to download model:', e);
      statusText.setText(`Download failed: ${e instanceof Error ? e.message : 'Unknown error'}`);
      statusText.addClass('elysium-wizard-error');

      this.advancedSearchSettings.enabled = false;
      if (this.advancedToggle) {
        this.advancedToggle.setValue(false);
      }

      new Notice('Failed to download model. You can try again later in Settings.');
    }
  }

  private async saveAdvancedAndContinue(): Promise<void> {
    this.config.updateAdvancedSemanticSearchConfig(this.advancedSearchSettings);

    try {
      await this.config.save();
    } catch (e) {
      console.error('Failed to save config:', e);
    }

    this.currentStep = 'inbox';
    this.renderStep();
  }

  private renderInbox() {
    const { contentEl } = this;

    contentEl.createEl('h2', { text: 'Configure Inbox' });
    
    const intro = contentEl.createDiv({ cls: 'elysium-wizard-intro' });
    intro.createEl('p', { 
      text: 'Inbox is a quick capture file for fleeting notes. Use Cmd+Shift+N to quickly add memos.'
    });

    const existingFile = this.app.vault.getAbstractFileByPath(this.inboxPath);
    if (existingFile) {
      intro.createEl('p', { 
        text: `✓ Found existing inbox at "${this.inboxPath}"`,
        cls: 'elysium-wizard-success'
      });
    }

    new Setting(contentEl)
      .setName('Inbox path')
      .setDesc('Path to inbox file (relative to vault root)')
      .addText(text => {
        text.setValue(this.inboxPath);
        text.setPlaceholder('inbox.md');
        text.onChange(value => {
          this.inboxPath = value.trim() || 'inbox.md';
        });
      });

    const examples = contentEl.createDiv({ cls: 'elysium-wizard-examples' });
    examples.createEl('p', { text: 'Examples:', cls: 'label' });
    const exampleList = examples.createEl('ul');
    exampleList.createEl('li', { text: 'inbox.md (vault root)' });
    exampleList.createEl('li', { text: 'Inbox/inbox.md (in Inbox folder)' });
    exampleList.createEl('li', { text: '_system/inbox.md (in system folder)' });

    const buttonContainer = contentEl.createDiv({ cls: 'elysium-wizard-buttons' });

    const finishBtn = buttonContainer.createEl('button', { 
      text: 'Finish Setup',
      cls: 'mod-cta'
    });
    finishBtn.addEventListener('click', () => this.saveInboxAndComplete());
  }

  private async saveInboxAndComplete() {
    this.config.updateInboxConfig({ 
      enabled: true, 
      path: this.inboxPath 
    });

    try {
      await this.config.save();
    } catch (e) {
      console.error('Failed to save config:', e);
    }

    const file = this.app.vault.getAbstractFileByPath(this.inboxPath);
    if (!file) {
      const parentPath = this.inboxPath.split('/').slice(0, -1).join('/');
      if (parentPath) {
        const parentFolder = this.app.vault.getAbstractFileByPath(parentPath);
        if (!parentFolder) {
          await this.app.vault.createFolder(parentPath);
        }
      }
      
      const inboxContent = '# Inbox\n\n> Quick capture space. Process with AI or manually.\n\n---\n\n';
      await this.app.vault.create(this.inboxPath, inboxContent);
    }

    this.currentStep = 'complete';
    this.renderStep();
  }

  private renderComplete() {
    const { contentEl } = this;

    contentEl.createEl('h2', { text: 'Setup Complete!' });
    
    const successEl = contentEl.createDiv({ cls: 'elysium-wizard-success-panel' });
    successEl.createEl('div', { text: '✓', cls: 'checkmark' });
    successEl.createEl('p', { text: 'Elysium is configured and ready to use.' });

    const nextSteps = contentEl.createDiv({ cls: 'elysium-wizard-next-steps' });
    nextSteps.createEl('h3', { text: 'Next Steps' });
    
    const steps = nextSteps.createEl('ol');
    steps.createEl('li', { text: 'Run "Reindex Vault" to build the search index (Cmd+P → "Elysium: Reindex")' });
    steps.createEl('li', { text: 'Use Cmd+Shift+S for semantic search' });
    steps.createEl('li', { text: 'Use Cmd+Shift+O for quick switcher with semantic search' });
    steps.createEl('li', { text: 'Check the Related Notes panel in the sidebar' });

    const buttonContainer = contentEl.createDiv({ cls: 'elysium-wizard-buttons' });

    const closeBtn = buttonContainer.createEl('button', { 
      text: 'Get Started',
      cls: 'mod-cta'
    });
    closeBtn.addEventListener('click', () => {
      this.close();
      this.onComplete();
    });
  }
}

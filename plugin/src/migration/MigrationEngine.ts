import { App, TFile, Notice } from 'obsidian';
import { SchemaRecommendation } from '../config/VaultScanner';
import { GistConfig, FIELD_NAMES } from '../config/ElysiumConfig';

const isExcludedPath = (path: string): boolean => {
  return path.split('/').some(part => part.startsWith('.'));
};

export interface MigrationPlan {
  totalFiles: number;
  filesToModify: FileModification[];
  summary: {
    addType: number;
    updateType: number;
    addArea: number;
    updateArea: number;
    addGist: number;
    addFrontmatter: number;
  };
}

export interface FileModification {
  file: TFile;
  path: string;
  changes: FieldChange[];
  hasFrontmatter: boolean;
  needsNewFrontmatter: boolean;
}

export interface FieldChange {
  field: string;
  action: 'add' | 'update' | 'keep';
  oldValue?: string;
  newValue: string;
  reason: string;
}

export interface MigrationResult {
  success: boolean;
  filesModified: number;
  filesFailed: number;
  errors: Array<{ path: string; error: string }>;
}

export interface MigrationProgress {
  current: number;
  total: number;
  currentFile: string;
  phase: 'analyzing' | 'migrating' | 'complete';
}

export class MigrationEngine {
  private app: App;
  private recommendation: SchemaRecommendation;
  private gistConfig: GistConfig;

  constructor(app: App, recommendation: SchemaRecommendation, gistConfig?: GistConfig) {
    this.app = app;
    this.recommendation = recommendation;
    this.gistConfig = gistConfig ?? {
      enabled: false,
      maxLength: 200,
    };
  }

  private filterExcludedFiles(files: TFile[]): TFile[] {
    return files.filter(file => !isExcludedPath(file.path));
  }

  async createMigrationPlan(
    onProgress?: (progress: MigrationProgress) => void
  ): Promise<MigrationPlan> {
    const allFiles = this.app.vault.getMarkdownFiles();
    const files = this.filterExcludedFiles(allFiles);
    const filesToModify: FileModification[] = [];
    
    const summary = {
      addType: 0,
      updateType: 0,
      addArea: 0,
      updateArea: 0,
      addGist: 0,
      addFrontmatter: 0,
    };

    for (let i = 0; i < files.length; i++) {
      const file = files[i];
      
      if (i % 10 === 0) {
        onProgress?.({
          current: i + 1,
          total: files.length,
          currentFile: file.path,
          phase: 'analyzing',
        });
        await new Promise(resolve => setTimeout(resolve, 0));
      }

      const cachedMeta = this.app.metadataCache.getFileCache(file);
      const frontmatter = cachedMeta?.frontmatter ?? null;

      const modification = this.analyzeFileWithCache(file, frontmatter);

      if (modification.changes.length > 0 || modification.needsNewFrontmatter) {
        filesToModify.push(modification);

        for (const change of modification.changes) {
          if (change.field === FIELD_NAMES.TYPE) {
            if (change.action === 'add') summary.addType++;
            else if (change.action === 'update') summary.updateType++;
          } else if (change.field === FIELD_NAMES.AREA) {
            if (change.action === 'add') summary.addArea++;
            else if (change.action === 'update') summary.updateArea++;
          } else if (change.field === FIELD_NAMES.GIST) {
            if (change.action === 'add') summary.addGist++;
          }
        }

        if (modification.needsNewFrontmatter) {
          summary.addFrontmatter++;
        }
      }
    }

    return {
      totalFiles: files.length,
      filesToModify,
      summary,
    };
  }

  private analyzeFileWithCache(
    file: TFile,
    frontmatter: Record<string, unknown> | null
  ): FileModification {
    const changes: FieldChange[] = [];
    const hasFrontmatter = frontmatter !== null;
    const needsNewFrontmatter = !hasFrontmatter;

    const typeRec = this.recommendation.typeField;
    if (typeRec.existingField && frontmatter) {
      const currentValue = frontmatter[typeRec.existingField] as string | undefined;
      if (currentValue) {
        const mappedValue = typeRec.valueMapping.get(currentValue);
        if (mappedValue && mappedValue !== currentValue) {
          changes.push({
            field: FIELD_NAMES.TYPE,
            action: 'update',
            oldValue: currentValue,
            newValue: mappedValue,
            reason: `Mapping "${currentValue}" to standardized value "${mappedValue}"`,
          });
        }
      } else if (!frontmatter[FIELD_NAMES.TYPE]) {
        changes.push({
          field: FIELD_NAMES.TYPE,
          action: 'add',
          newValue: 'note',
          reason: 'Adding default type field',
        });
      }
    } else if (!hasFrontmatter || !frontmatter?.[FIELD_NAMES.TYPE]) {
      changes.push({
        field: FIELD_NAMES.TYPE,
        action: 'add',
        newValue: 'note',
        reason: 'Adding default type field',
      });
    }

    const areaRec = this.recommendation.areaField;
    if (areaRec.existingField && frontmatter) {
      const currentValue = frontmatter[areaRec.existingField] as string | undefined;
      if (currentValue) {
        const mappedValue = areaRec.valueMapping.get(currentValue);
        if (mappedValue && mappedValue !== currentValue) {
          changes.push({
            field: FIELD_NAMES.AREA,
            action: 'update',
            oldValue: currentValue,
            newValue: mappedValue,
            reason: `Mapping "${currentValue}" to standardized value "${mappedValue}"`,
          });
        }
      }
    }

    // Note: gist is intentionally left empty for AI or human to fill later
    // No auto-generation to avoid YAML corruption issues

    return {
      file,
      path: file.path,
      changes,
      hasFrontmatter,
      needsNewFrontmatter,
    };
  }

  async executeMigration(
    plan: MigrationPlan,
    onProgress?: (progress: MigrationProgress) => void
  ): Promise<MigrationResult> {
    const errors: Array<{ path: string; error: string }> = [];
    let filesModified = 0;

    for (let i = 0; i < plan.filesToModify.length; i++) {
      const mod = plan.filesToModify[i];
      
      onProgress?.({
        current: i + 1,
        total: plan.filesToModify.length,
        currentFile: mod.path,
        phase: 'migrating',
      });

      try {
        await this.migrateFile(mod);
        filesModified++;
      } catch (e) {
        errors.push({
          path: mod.path,
          error: e instanceof Error ? e.message : String(e),
        });
      }
    }

    onProgress?.({
      current: plan.filesToModify.length,
      total: plan.filesToModify.length,
      currentFile: '',
      phase: 'complete',
    });

    return {
      success: errors.length === 0,
      filesModified,
      filesFailed: errors.length,
      errors,
    };
  }

  private async migrateFile(mod: FileModification): Promise<void> {
    const content = await this.app.vault.read(mod.file);
    const newContent = this.applyChanges(content, mod);
    
    if (newContent !== content) {
      await this.app.vault.modify(mod.file, newContent);
    }
  }

  private applyChanges(content: string, mod: FileModification): string {
    if (mod.needsNewFrontmatter) {
      const frontmatterLines = ['---'];
      for (const change of mod.changes) {
        if (change.field === FIELD_NAMES.GIST) {
          frontmatterLines.push(`${change.field}: >`);
          frontmatterLines.push(`  ${change.newValue}`);
        } else {
          frontmatterLines.push(`${change.field}: ${change.newValue}`);
        }
      }
      frontmatterLines.push('---', '');
      return frontmatterLines.join('\n') + content;
    }

    const frontmatterMatch = content.match(/^(---\s*\n)([\s\S]*?)(\n---)/);
    if (!frontmatterMatch) return content;

    let frontmatterContent = frontmatterMatch[2];
    const beforeFrontmatter = frontmatterMatch[1];
    const afterFrontmatter = frontmatterMatch[3];
    const afterContent = content.slice(frontmatterMatch[0].length);

    for (const change of mod.changes) {
      if (change.action === 'add') {
        if (change.field === FIELD_NAMES.GIST) {
          frontmatterContent += `\n${change.field}: >\n  ${change.newValue}`;
        } else {
          frontmatterContent += `\n${change.field}: ${change.newValue}`;
        }
      } else if (change.action === 'update' && change.oldValue) {
        const existingField = this.recommendation.typeField.existingField === change.field 
          ? this.recommendation.typeField.existingField
          : this.recommendation.areaField.existingField;
        
        if (existingField) {
          const regex = new RegExp(`^(${existingField}:\\s*)${this.escapeRegex(change.oldValue)}`, 'm');
          if (regex.test(frontmatterContent)) {
            frontmatterContent += `\n${change.field}: ${change.newValue}`;
          }
        }
      }
    }

    return beforeFrontmatter + frontmatterContent + afterFrontmatter + afterContent;
  }

  private escapeRegex(str: string): string {
    return str.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  }

  private extractFrontmatter(content: string): Record<string, unknown> | null {
    const match = content.match(/^---\s*\n([\s\S]*?)\n---/);
    if (!match) return null;

    const yaml = match[1];
    const result: Record<string, unknown> = {};

    const lines = yaml.split('\n');
    for (const line of lines) {
      const keyMatch = line.match(/^([a-zA-Z_][a-zA-Z0-9_]*):\s*(.*)/);
      if (keyMatch) {
        const key = keyMatch[1];
        const value = keyMatch[2].trim().replace(/["']/g, '');
        result[key] = value;
      }
    }

    return Object.keys(result).length > 0 ? result : null;
  }
}

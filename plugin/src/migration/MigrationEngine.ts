import { App, TFile, Notice } from 'obsidian';
import { SchemaRecommendation } from '../config/VaultScanner';
import { GistConfig } from '../config/ElysiumConfig';

export type GistSource = 'human' | 'auto' | 'ai';

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
      fieldName: 'gist',
      autoGenerate: true,
      maxLength: 200,
      trackSource: true,
      sourceFieldName: 'gist_source',
      dateFieldName: 'gist_date',
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
      
      onProgress?.({
        current: i + 1,
        total: files.length,
        currentFile: file.path,
        phase: 'analyzing',
      });

      const content = await this.app.vault.cachedRead(file);
      const modification = this.analyzeFile(file, content);

      if (modification.changes.length > 0 || modification.needsNewFrontmatter) {
        filesToModify.push(modification);

        for (const change of modification.changes) {
          if (change.field === this.recommendation.typeField.recommendedName) {
            if (change.action === 'add') summary.addType++;
            else if (change.action === 'update') summary.updateType++;
          } else if (change.field === this.recommendation.areaField.recommendedName) {
            if (change.action === 'add') summary.addArea++;
            else if (change.action === 'update') summary.updateArea++;
          } else if (change.field === this.recommendation.gistField.recommendedName) {
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

  private analyzeFile(file: TFile, content: string): FileModification {
    const changes: FieldChange[] = [];
    const frontmatter = this.extractFrontmatter(content);
    const hasFrontmatter = frontmatter !== null;
    const needsNewFrontmatter = !hasFrontmatter;

    const typeRec = this.recommendation.typeField;
    if (typeRec.existingField && frontmatter) {
      const currentValue = frontmatter[typeRec.existingField] as string | undefined;
      if (currentValue) {
        const mappedValue = typeRec.valueMapping.get(currentValue);
        if (mappedValue && mappedValue !== currentValue) {
          changes.push({
            field: typeRec.recommendedName,
            action: 'update',
            oldValue: currentValue,
            newValue: mappedValue,
            reason: `Mapping "${currentValue}" to standardized value "${mappedValue}"`,
          });
        }
      } else if (!frontmatter[typeRec.recommendedName]) {
        changes.push({
          field: typeRec.recommendedName,
          action: 'add',
          newValue: 'note',
          reason: 'Adding default type field',
        });
      }
    } else if (!hasFrontmatter || !frontmatter?.[typeRec.recommendedName]) {
      changes.push({
        field: typeRec.recommendedName,
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
            field: areaRec.recommendedName,
            action: 'update',
            oldValue: currentValue,
            newValue: mappedValue,
            reason: `Mapping "${currentValue}" to standardized value "${mappedValue}"`,
          });
        }
      }
    }

    if (this.gistConfig.enabled && this.gistConfig.autoGenerate) {
      const gistRec = this.recommendation.gistField;
      const hasGist = frontmatter && (
        frontmatter[gistRec.recommendedName] || 
        (gistRec.existingField && frontmatter[gistRec.existingField])
      );
      
      if (!hasGist) {
        const generatedGist = this.generateGistFromContent(content);
        if (generatedGist) {
          changes.push({
            field: gistRec.recommendedName,
            action: 'add',
            newValue: generatedGist,
            reason: 'Auto-generated from first paragraph',
          });

          if (this.gistConfig.trackSource) {
            changes.push({
              field: this.gistConfig.sourceFieldName,
              action: 'add',
              newValue: 'auto',
              reason: 'Tracking gist source',
            });
            changes.push({
              field: this.gistConfig.dateFieldName,
              action: 'add',
              newValue: new Date().toISOString().split('T')[0],
              reason: 'Tracking gist creation date',
            });
          }
        }
      }
    }

    return {
      file,
      path: file.path,
      changes,
      hasFrontmatter,
      needsNewFrontmatter,
    };
  }

  private generateGistFromContent(content: string): string | null {
    const withoutFrontmatter = content.replace(/^---\s*\n[\s\S]*?\n---\s*\n?/, '');
    
    const withoutHeaders = withoutFrontmatter.replace(/^#+\s+.*$/gm, '');
    
    const paragraphs = withoutHeaders
      .split(/\n\n+/)
      .map(p => p.trim())
      .filter(p => p.length > 20 && !p.startsWith('```') && !p.startsWith('- ') && !p.startsWith('* '));

    if (paragraphs.length === 0) return null;

    let gist = paragraphs[0];
    
    gist = gist.replace(/\[\[([^\]|]+)(\|[^\]]+)?\]\]/g, '$1');
    gist = gist.replace(/\[([^\]]+)\]\([^)]+\)/g, '$1');
    gist = gist.replace(/[*_`]/g, '');
    
    const maxLen = this.gistConfig.maxLength;
    if (gist.length > maxLen) {
      gist = gist.slice(0, maxLen - 3) + '...';
    }

    return gist.length > 30 ? gist : null;
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
    const gistFieldName = this.gistConfig.fieldName;
    
    if (mod.needsNewFrontmatter) {
      const frontmatterLines = ['---'];
      for (const change of mod.changes) {
        if (change.field === gistFieldName) {
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
        if (change.field === gistFieldName) {
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

import { App, TFile } from 'obsidian';

export interface FieldAnalysis {
  name: string;
  count: number;
  coverage: number;
  values: Map<string, number>;
  uniqueCount: number;
  isHighCardinality: boolean;
  dominantValues: Array<{ value: string; count: number; percentage: number }>;
  dataType: 'string' | 'array' | 'number' | 'boolean' | 'mixed';
}

export interface VaultAnalysis {
  totalFiles: number;
  filesWithFrontmatter: number;
  frontmatterCoverage: number;
  fields: Map<string, FieldAnalysis>;
  folders: Map<string, number>;
  potentialTypeFields: FieldAnalysis[];
  potentialAreaFields: FieldAnalysis[];
  potentialGistFields: FieldAnalysis[];
  potentialTagsFields: FieldAnalysis[];
  healthScore: number;
  issues: string[];
}

export interface SchemaRecommendation {
  typeField: {
    existingField: string | null;
    recommendedName: string;
    existingValues: string[];
    recommendedValues: string[];
    valueMapping: Map<string, string>;
    needsMigration: boolean;
  };
  areaField: {
    existingField: string | null;
    recommendedName: string;
    existingValues: string[];
    recommendedValues: string[];
    valueMapping: Map<string, string>;
    needsMigration: boolean;
  };
  gistField: {
    existingField: string | null;
    recommendedName: string;
    notesWithoutGist: number;
    needsGeneration: boolean;
  };
  tagsField: {
    existingField: string | null;
    recommendedName: string;
  };
  migrationStats: {
    notesNeedingTypeUpdate: number;
    notesNeedingAreaUpdate: number;
    notesNeedingGistGeneration: number;
    notesNeedingNewFrontmatter: number;
  };
}

const DEFAULT_TYPE_VALUES = ['note', 'term', 'project', 'log'];
const DEFAULT_AREA_VALUES = ['work', 'tech', 'life', 'career', 'learning', 'reference'];

const TYPE_FIELD_ALIASES = ['type', 'category', 'kind', 'note_type', 'doctype', 'content_type'];
const AREA_FIELD_ALIASES = ['area', 'domain', 'topic', 'subject', 'field', 'scope'];
const GIST_FIELD_ALIASES = ['gist', 'summary', 'description', 'abstract', 'tldr', 'desc', 'excerpt', 'overview'];
const TAGS_FIELD_ALIASES = ['tags', 'keywords', 'labels', 'categories', 'topics'];

const isExcludedPath = (path: string): boolean => {
  return path.split('/').some(part => part.startsWith('.'));
};

export class VaultScanner {
  private app: App;

  constructor(app: App) {
    this.app = app;
  }

  private filterExcludedFiles(files: TFile[]): TFile[] {
    return files.filter(file => !isExcludedPath(file.path));
  }

  async analyzeVault(sampleSize: number = 200): Promise<VaultAnalysis> {
    const allFiles = this.app.vault.getMarkdownFiles();
    const files = this.filterExcludedFiles(allFiles);
    const sampled = this.sampleFiles(files, sampleSize);
    
    const fields = new Map<string, FieldAnalysis>();
    const folders = new Map<string, number>();
    let filesWithFrontmatter = 0;
    const issues: string[] = [];

    for (const file of sampled) {
      const folder = file.parent?.path ?? '/';
      folders.set(folder, (folders.get(folder) ?? 0) + 1);

      const content = await this.app.vault.cachedRead(file);
      const frontmatter = this.parseFrontmatter(content);
      
      if (frontmatter && Object.keys(frontmatter).length > 0) {
        filesWithFrontmatter++;
        this.analyzeFields(frontmatter, fields);
      }
    }

    this.finalizeFieldAnalysis(fields, filesWithFrontmatter);

    const frontmatterCoverage = sampled.length > 0 ? filesWithFrontmatter / sampled.length : 0;
    
    if (frontmatterCoverage < 0.3) {
      issues.push(`Low frontmatter coverage: only ${Math.round(frontmatterCoverage * 100)}% of notes have frontmatter`);
    }

    const potentialTypeFields = this.findPotentialFields(fields, TYPE_FIELD_ALIASES, false);
    const potentialAreaFields = this.findPotentialFields(fields, AREA_FIELD_ALIASES, false);
    const potentialGistFields = this.findPotentialFields(fields, GIST_FIELD_ALIASES, true);
    const potentialTagsFields = this.findPotentialFields(fields, TAGS_FIELD_ALIASES, false, 'array');

    const healthScore = this.calculateHealthScore(frontmatterCoverage, potentialTypeFields, potentialAreaFields, potentialGistFields);

    return {
      totalFiles: files.length,
      filesWithFrontmatter,
      frontmatterCoverage,
      fields,
      folders,
      potentialTypeFields,
      potentialAreaFields,
      potentialGistFields,
      potentialTagsFields,
      healthScore,
      issues,
    };
  }

  generateRecommendation(analysis: VaultAnalysis): SchemaRecommendation {
    const typeRec = this.recommendTypeField(analysis);
    const areaRec = this.recommendAreaField(analysis);
    const gistRec = this.recommendGistField(analysis);
    const tagsRec = this.recommendTagsField(analysis);

    const migrationStats = {
      notesNeedingTypeUpdate: typeRec.needsMigration ? 
        this.estimateMigrationCount(analysis, typeRec.existingField, typeRec.valueMapping) : 0,
      notesNeedingAreaUpdate: areaRec.needsMigration ?
        this.estimateMigrationCount(analysis, areaRec.existingField, areaRec.valueMapping) : 0,
      notesNeedingGistGeneration: gistRec.notesWithoutGist,
      notesNeedingNewFrontmatter: Math.round(analysis.totalFiles * (1 - analysis.frontmatterCoverage)),
    };

    return {
      typeField: typeRec,
      areaField: areaRec,
      gistField: gistRec,
      tagsField: tagsRec,
      migrationStats,
    };
  }

  private recommendTypeField(analysis: VaultAnalysis): SchemaRecommendation['typeField'] {
    const existing = analysis.potentialTypeFields[0];
    
    if (!existing) {
      return {
        existingField: null,
        recommendedName: 'type',
        existingValues: [],
        recommendedValues: DEFAULT_TYPE_VALUES,
        valueMapping: new Map(),
        needsMigration: false,
      };
    }

    const existingValues = [...existing.values.keys()];
    const isClean = !existing.isHighCardinality && existingValues.length <= 10;

    if (isClean && existing.name === 'type') {
      return {
        existingField: existing.name,
        recommendedName: 'type',
        existingValues,
        recommendedValues: existingValues,
        valueMapping: new Map(existingValues.map(v => [v, v])),
        needsMigration: false,
      };
    }

    const valueMapping = this.createValueMapping(existingValues, DEFAULT_TYPE_VALUES);
    
    return {
      existingField: existing.name,
      recommendedName: 'type',
      existingValues,
      recommendedValues: DEFAULT_TYPE_VALUES,
      valueMapping,
      needsMigration: true,
    };
  }

  private recommendAreaField(analysis: VaultAnalysis): SchemaRecommendation['areaField'] {
    const existing = analysis.potentialAreaFields[0];
    
    if (!existing) {
      return {
        existingField: null,
        recommendedName: 'area',
        existingValues: [],
        recommendedValues: DEFAULT_AREA_VALUES,
        valueMapping: new Map(),
        needsMigration: false,
      };
    }

    const existingValues = [...existing.values.keys()];
    const isClean = !existing.isHighCardinality && existingValues.length <= 15;

    if (isClean && existing.name === 'area') {
      return {
        existingField: existing.name,
        recommendedName: 'area',
        existingValues,
        recommendedValues: existingValues,
        valueMapping: new Map(existingValues.map(v => [v, v])),
        needsMigration: false,
      };
    }

    const valueMapping = this.createValueMapping(existingValues, DEFAULT_AREA_VALUES);
    
    return {
      existingField: existing.name,
      recommendedName: 'area',
      existingValues,
      recommendedValues: DEFAULT_AREA_VALUES,
      valueMapping,
      needsMigration: true,
    };
  }

  private recommendGistField(analysis: VaultAnalysis): SchemaRecommendation['gistField'] {
    const existing = analysis.potentialGistFields[0];
    
    const notesWithoutGist = existing 
      ? analysis.filesWithFrontmatter - existing.count
      : analysis.totalFiles;

    return {
      existingField: existing?.name ?? null,
      recommendedName: 'gist',
      notesWithoutGist,
      needsGeneration: notesWithoutGist > 0,
    };
  }

  private recommendTagsField(analysis: VaultAnalysis): SchemaRecommendation['tagsField'] {
    const existing = analysis.potentialTagsFields[0];
    
    return {
      existingField: existing?.name ?? null,
      recommendedName: 'tags',
    };
  }

  private createValueMapping(existingValues: string[], recommendedValues: string[]): Map<string, string> {
    const mapping = new Map<string, string>();
    const recommendedLower = recommendedValues.map(v => v.toLowerCase());

    for (const existing of existingValues) {
      const existingLower = existing.toLowerCase();
      
      const exactMatch = recommendedValues.find(r => r.toLowerCase() === existingLower);
      if (exactMatch) {
        mapping.set(existing, exactMatch);
        continue;
      }

      const partialMatch = recommendedValues.find(r => 
        existingLower.includes(r.toLowerCase()) || r.toLowerCase().includes(existingLower)
      );
      if (partialMatch) {
        mapping.set(existing, partialMatch);
        continue;
      }

      mapping.set(existing, recommendedValues[0]);
    }

    return mapping;
  }

  private estimateMigrationCount(
    analysis: VaultAnalysis, 
    fieldName: string | null, 
    mapping: Map<string, string>
  ): number {
    if (!fieldName) return 0;
    
    const field = analysis.fields.get(fieldName);
    if (!field) return 0;

    let count = 0;
    for (const [value, valueCount] of field.values) {
      const mapped = mapping.get(value);
      if (mapped && mapped !== value) {
        count += valueCount;
      }
    }
    return count;
  }

  private sampleFiles(files: TFile[], size: number): TFile[] {
    if (files.length <= size) return files;
    
    const shuffled = [...files].sort(() => Math.random() - 0.5);
    return shuffled.slice(0, size);
  }

  private parseFrontmatter(content: string): Record<string, unknown> | null {
    const match = content.match(/^---\s*\n([\s\S]*?)\n---/);
    if (!match) return null;

    const yaml = match[1];
    const result: Record<string, unknown> = {};

    const lines = yaml.split('\n');
    let currentKey: string | null = null;
    let multilineValue = '';

    for (const line of lines) {
      const keyMatch = line.match(/^([a-zA-Z_][a-zA-Z0-9_]*):\s*(.*)/);
      
      if (keyMatch) {
        if (currentKey && multilineValue) {
          result[currentKey] = multilineValue.trim();
        }
        
        currentKey = keyMatch[1];
        const value = keyMatch[2].trim();

        if (value === '>' || value === '|') {
          multilineValue = '';
        } else if (value.startsWith('[')) {
          const arrayMatch = value.match(/\[([^\]]*)\]/);
          if (arrayMatch) {
            result[currentKey] = arrayMatch[1]
              .split(',')
              .map(v => v.trim().replace(/["']/g, ''))
              .filter(v => v.length > 0);
          }
          currentKey = null;
        } else if (value === 'true' || value === 'false') {
          result[currentKey] = value === 'true';
          currentKey = null;
        } else if (!isNaN(Number(value)) && value !== '') {
          result[currentKey] = Number(value);
          currentKey = null;
        } else {
          result[currentKey] = value.replace(/["']/g, '');
          currentKey = null;
        }
      } else if (currentKey && line.startsWith('  ')) {
        multilineValue += ' ' + line.trim();
      }
    }

    if (currentKey && multilineValue) {
      result[currentKey] = multilineValue.trim();
    }

    return Object.keys(result).length > 0 ? result : null;
  }

  private analyzeFields(frontmatter: Record<string, unknown>, fields: Map<string, FieldAnalysis>): void {
    for (const [key, value] of Object.entries(frontmatter)) {
      let analysis = fields.get(key);
      if (!analysis) {
        analysis = {
          name: key,
          count: 0,
          coverage: 0,
          values: new Map(),
          uniqueCount: 0,
          isHighCardinality: false,
          dominantValues: [],
          dataType: 'string',
        };
        fields.set(key, analysis);
      }

      analysis.count++;

      const valueType = Array.isArray(value) ? 'array' : typeof value as any;
      if (analysis.dataType !== valueType && analysis.count > 1) {
        analysis.dataType = 'mixed';
      } else {
        analysis.dataType = valueType;
      }

      if (typeof value === 'string' && value.length < 100) {
        analysis.values.set(value, (analysis.values.get(value) ?? 0) + 1);
      } else if (Array.isArray(value)) {
        for (const item of value) {
          if (typeof item === 'string') {
            analysis.values.set(item, (analysis.values.get(item) ?? 0) + 1);
          }
        }
      }
    }
  }

  private finalizeFieldAnalysis(fields: Map<string, FieldAnalysis>, totalWithFrontmatter: number): void {
    for (const analysis of fields.values()) {
      analysis.coverage = totalWithFrontmatter > 0 ? analysis.count / totalWithFrontmatter : 0;
      analysis.uniqueCount = analysis.values.size;
      analysis.isHighCardinality = analysis.uniqueCount > 15;
      
      analysis.dominantValues = [...analysis.values.entries()]
        .sort((a, b) => b[1] - a[1])
        .slice(0, 10)
        .map(([value, count]) => ({
          value,
          count,
          percentage: analysis.count > 0 ? (count / analysis.count) * 100 : 0,
        }));
    }
  }

  private findPotentialFields(
    fields: Map<string, FieldAnalysis>,
    aliases: string[],
    allowLongValues: boolean,
    requiredType?: string
  ): FieldAnalysis[] {
    const matches: FieldAnalysis[] = [];

    for (const [name, analysis] of fields) {
      const nameLower = name.toLowerCase();
      
      if (!aliases.some(alias => nameLower.includes(alias) || alias.includes(nameLower))) {
        continue;
      }

      if (requiredType && analysis.dataType !== requiredType && analysis.dataType !== 'mixed') {
        continue;
      }

      if (!allowLongValues && analysis.values.size === 0) {
        continue;
      }

      matches.push(analysis);
    }

    return matches.sort((a, b) => b.coverage - a.coverage);
  }

  private calculateHealthScore(
    frontmatterCoverage: number,
    typeFields: FieldAnalysis[],
    areaFields: FieldAnalysis[],
    gistFields: FieldAnalysis[]
  ): number {
    let score = 0;

    score += frontmatterCoverage * 30;

    if (typeFields.length > 0) {
      const typeField = typeFields[0];
      score += typeField.coverage * 20;
      if (!typeField.isHighCardinality) score += 10;
    }

    if (areaFields.length > 0) {
      const areaField = areaFields[0];
      score += areaField.coverage * 15;
      if (!areaField.isHighCardinality) score += 5;
    }

    if (gistFields.length > 0) {
      score += gistFields[0].coverage * 20;
    }

    return Math.round(Math.min(100, score));
  }
}

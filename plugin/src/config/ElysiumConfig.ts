import { App } from 'obsidian';

export interface GistConfig {
  enabled: boolean;
  fieldName: string;
  autoGenerate: boolean;
  maxLength: number;
  trackSource: boolean;
  sourceFieldName: string;
  dateFieldName: string;
}

export interface SchemaConfig {
  fields: {
    type: { name: string; values: string[] };
    area: { name: string; values: string[] };
    gist: GistConfig;
    tags: { name: string };
  };
  validation: {
    requireType: boolean;
    requireArea: boolean;
    maxTags: number;
    lowercaseTags: boolean;
  };
}

export interface ElysiumConfigData {
  version: number;
  schema: SchemaConfig;
  inbox: {
    enabled: boolean;
    path: string;
  };
  features: {
    semanticSearch: boolean;
    wikilinkValidation: boolean;
  };
}

const DEFAULT_CONFIG: ElysiumConfigData = {
  version: 1,
  schema: {
    fields: {
      type: { name: 'type', values: ['note', 'term', 'project', 'log'] },
      area: { name: 'area', values: ['work', 'tech', 'life', 'career', 'learning', 'reference'] },
      gist: {
        enabled: false,
        fieldName: 'gist',
        autoGenerate: true,
        maxLength: 200,
        trackSource: true,
        sourceFieldName: 'gist_source',
        dateFieldName: 'gist_date',
      },
      tags: { name: 'tags' },
    },
    validation: {
      requireType: true,
      requireArea: true,
      maxTags: 5,
      lowercaseTags: true,
    },
  },
  inbox: {
    enabled: true,
    path: 'inbox.md',
  },
  features: {
    semanticSearch: true,
    wikilinkValidation: true,
  },
};

export class ElysiumConfig {
  private app: App;
  private config: ElysiumConfigData;
  private configPath = '.elysium.json';

  constructor(app: App) {
    this.app = app;
    this.config = { ...DEFAULT_CONFIG };
  }

  async load(): Promise<boolean> {
    try {
      const file = this.app.vault.getAbstractFileByPath(this.configPath);
      if (!file) {
        return false;
      }

      const content = await this.app.vault.read(file as any);
      const parsed = JSON.parse(content);
      this.config = this.mergeWithDefaults(parsed);
      return true;
    } catch (e) {
      console.error('Failed to load .elysium.json:', e);
      return false;
    }
  }

  async save(): Promise<void> {
    const content = JSON.stringify(this.config, null, 2);
    const file = this.app.vault.getAbstractFileByPath(this.configPath);
    
    if (file) {
      await this.app.vault.modify(file as any, content);
    } else {
      await this.app.vault.create(this.configPath, content);
    }
  }

  async exists(): Promise<boolean> {
    return this.app.vault.getAbstractFileByPath(this.configPath) !== null;
  }

  private mergeWithDefaults(parsed: Partial<ElysiumConfigData>): ElysiumConfigData {
    const parsedGist = parsed.schema?.fields?.gist as Partial<GistConfig> | undefined;
    
    return {
      version: parsed.version ?? DEFAULT_CONFIG.version,
      schema: {
        fields: {
          type: { 
            name: parsed.schema?.fields?.type?.name ?? DEFAULT_CONFIG.schema.fields.type.name,
            values: parsed.schema?.fields?.type?.values ?? DEFAULT_CONFIG.schema.fields.type.values,
          },
          area: {
            name: parsed.schema?.fields?.area?.name ?? DEFAULT_CONFIG.schema.fields.area.name,
            values: parsed.schema?.fields?.area?.values ?? DEFAULT_CONFIG.schema.fields.area.values,
          },
          gist: {
            enabled: parsedGist?.enabled ?? DEFAULT_CONFIG.schema.fields.gist.enabled,
            fieldName: parsedGist?.fieldName ?? DEFAULT_CONFIG.schema.fields.gist.fieldName,
            autoGenerate: parsedGist?.autoGenerate ?? DEFAULT_CONFIG.schema.fields.gist.autoGenerate,
            maxLength: parsedGist?.maxLength ?? DEFAULT_CONFIG.schema.fields.gist.maxLength,
            trackSource: parsedGist?.trackSource ?? DEFAULT_CONFIG.schema.fields.gist.trackSource,
            sourceFieldName: parsedGist?.sourceFieldName ?? DEFAULT_CONFIG.schema.fields.gist.sourceFieldName,
            dateFieldName: parsedGist?.dateFieldName ?? DEFAULT_CONFIG.schema.fields.gist.dateFieldName,
          },
          tags: {
            name: parsed.schema?.fields?.tags?.name ?? DEFAULT_CONFIG.schema.fields.tags.name,
          },
        },
        validation: {
          requireType: parsed.schema?.validation?.requireType ?? DEFAULT_CONFIG.schema.validation.requireType,
          requireArea: parsed.schema?.validation?.requireArea ?? DEFAULT_CONFIG.schema.validation.requireArea,
          maxTags: parsed.schema?.validation?.maxTags ?? DEFAULT_CONFIG.schema.validation.maxTags,
          lowercaseTags: parsed.schema?.validation?.lowercaseTags ?? DEFAULT_CONFIG.schema.validation.lowercaseTags,
        },
      },
      inbox: {
        enabled: parsed.inbox?.enabled ?? DEFAULT_CONFIG.inbox.enabled,
        path: parsed.inbox?.path ?? DEFAULT_CONFIG.inbox.path,
      },
      features: {
        semanticSearch: parsed.features?.semanticSearch ?? DEFAULT_CONFIG.features.semanticSearch,
        wikilinkValidation: parsed.features?.wikilinkValidation ?? DEFAULT_CONFIG.features.wikilinkValidation,
      },
    };
  }

  get data(): ElysiumConfigData {
    return this.config;
  }

  get schema(): SchemaConfig {
    return this.config.schema;
  }

  getTypeFieldName(): string {
    return this.config.schema.fields.type.name;
  }

  getTypeValues(): string[] {
    return this.config.schema.fields.type.values;
  }

  getAreaFieldName(): string {
    return this.config.schema.fields.area.name;
  }

  getAreaValues(): string[] {
    return this.config.schema.fields.area.values;
  }

  getGistFieldName(): string {
    return this.config.schema.fields.gist.fieldName;
  }

  getGistConfig(): GistConfig {
    return this.config.schema.fields.gist;
  }

  isGistEnabled(): boolean {
    return this.config.schema.fields.gist.enabled;
  }

  updateGistConfig(gist: Partial<GistConfig>): void {
    this.config.schema.fields.gist = { ...this.config.schema.fields.gist, ...gist };
  }

  getTagsFieldName(): string {
    return this.config.schema.fields.tags.name;
  }

  updateSchema(schema: Partial<SchemaConfig>): void {
    if (schema.fields) {
      if (schema.fields.type) {
        this.config.schema.fields.type = { ...this.config.schema.fields.type, ...schema.fields.type };
      }
      if (schema.fields.area) {
        this.config.schema.fields.area = { ...this.config.schema.fields.area, ...schema.fields.area };
      }
      if (schema.fields.gist) {
        this.config.schema.fields.gist = { ...this.config.schema.fields.gist, ...schema.fields.gist };
      }
      if (schema.fields.tags) {
        this.config.schema.fields.tags = { ...this.config.schema.fields.tags, ...schema.fields.tags };
      }
    }
    if (schema.validation) {
      this.config.schema.validation = { ...this.config.schema.validation, ...schema.validation };
    }
  }

  getInboxPath(): string {
    return this.config.inbox.path;
  }

  isInboxEnabled(): boolean {
    return this.config.inbox.enabled;
  }

  updateInboxConfig(inbox: Partial<{ enabled: boolean; path: string }>): void {
    this.config.inbox = { ...this.config.inbox, ...inbox };
  }

  static getDefault(): ElysiumConfigData {
    return { ...DEFAULT_CONFIG };
  }
}

import { App } from 'obsidian';

export const FIELD_NAMES = {
  TYPE: 'elysium_type',
  STATUS: 'elysium_status',
  AREA: 'elysium_area',
  GIST: 'elysium_gist',
  TAGS: 'elysium_tags',
  GIST_SOURCE: 'elysium_gist_source',
  GIST_DATE: 'elysium_gist_date',
} as const;

export const DEFAULT_TYPE_VALUES = ['note', 'term', 'project', 'log'] as const;
export const DEFAULT_STATUS_VALUES = ['active', 'done', 'archived'] as const;
export const DEFAULT_AREA_VALUES = ['work', 'tech', 'life', 'career', 'learning', 'reference'] as const;

export interface GistConfig {
  enabled: boolean;
  autoGenerate: boolean;
  maxLength: number;
  trackSource: boolean;
}

export interface SchemaConfig {
  typeValues: string[];
  statusValues: string[];
  areaValues: string[];
  gist: GistConfig;
  tags: { maxCount: number; lowercase: boolean };
}

export interface FoldersConfig {
  notes: string;
  projects: string;
  archive: string;
}

export interface ElysiumConfigData {
  version: number;
  schema: SchemaConfig;
  folders: FoldersConfig;
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
  version: 3,
  schema: {
    typeValues: [...DEFAULT_TYPE_VALUES],
    statusValues: [...DEFAULT_STATUS_VALUES],
    areaValues: [...DEFAULT_AREA_VALUES],
    gist: {
      enabled: false,
      autoGenerate: true,
      maxLength: 200,
      trackSource: true,
    },
    tags: { maxCount: 5, lowercase: true },
  },
  folders: {
    notes: 'Notes',
    projects: 'Projects',
    archive: 'Archive',
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
  private configPath = '.obsidian/plugins/elysium/config.json';
  private legacyConfigPath = '.elysium.json';

  constructor(app: App) {
    this.app = app;
    this.config = { ...DEFAULT_CONFIG };
  }

  async load(): Promise<boolean> {
    try {
      const adapter = this.app.vault.adapter;
      
      let exists = await adapter.exists(this.configPath);
      let pathToRead = this.configPath;
      
      if (!exists) {
        const legacyExists = await adapter.exists(this.legacyConfigPath);
        if (legacyExists) {
          pathToRead = this.legacyConfigPath;
          exists = true;
        }
      }
      
      if (!exists) {
        return false;
      }

      const content = await adapter.read(pathToRead);
      const parsed = JSON.parse(content);
      this.config = this.mergeWithDefaults(parsed);
      
      if (pathToRead === this.legacyConfigPath) {
        await this.save();
        await adapter.remove(this.legacyConfigPath);
        console.log('[Elysium] Migrated config from .elysium.json to .obsidian/plugins/elysium/config.json');
      }
      
      return true;
    } catch (e) {
      console.error('[Elysium] Failed to load config:', e);
      return false;
    }
  }

  async save(): Promise<void> {
    const content = JSON.stringify(this.config, null, 2);
    await this.app.vault.adapter.write(this.configPath, content);
  }

  async exists(): Promise<boolean> {
    const newExists = await this.app.vault.adapter.exists(this.configPath);
    if (newExists) return true;
    return await this.app.vault.adapter.exists(this.legacyConfigPath);
  }

  private mergeWithDefaults(parsed: any): ElysiumConfigData {
    const parsedGist = parsed.schema?.gist as Partial<GistConfig> | undefined;
    const oldValidation = (parsed as any).schema?.validation;
    const parsedTags = parsed.schema?.tags ?? (parsed as any).schema?.fields?.tags;

    let typeValues = [...DEFAULT_CONFIG.schema.typeValues];
    let statusValues = [...DEFAULT_CONFIG.schema.statusValues];
    let areaValues = [...DEFAULT_CONFIG.schema.areaValues];

    if (parsed.version === 3 && parsed.schema) {
      typeValues = parsed.schema.typeValues ?? typeValues;
      statusValues = parsed.schema.statusValues ?? statusValues;
      areaValues = parsed.schema.areaValues ?? areaValues;
    } else if (parsed.version === 2 && parsed.schema?.filterableFields) {
      typeValues = parsed.schema.filterableFields.type?.values ?? typeValues;
      areaValues = parsed.schema.filterableFields.area?.values ?? areaValues;
    } else if (parsed.schema?.fields) {
      typeValues = parsed.schema.fields.type?.values ?? typeValues;
      areaValues = parsed.schema.fields.area?.values ?? areaValues;
    }

    return {
      version: 3,
      schema: {
        typeValues,
        statusValues,
        areaValues,
        gist: {
          enabled: parsedGist?.enabled ?? DEFAULT_CONFIG.schema.gist.enabled,
          autoGenerate: parsedGist?.autoGenerate ?? DEFAULT_CONFIG.schema.gist.autoGenerate,
          maxLength: parsedGist?.maxLength ?? DEFAULT_CONFIG.schema.gist.maxLength,
          trackSource: parsedGist?.trackSource ?? DEFAULT_CONFIG.schema.gist.trackSource,
        },
        tags: {
          maxCount: parsedTags?.maxCount ?? oldValidation?.maxTags ?? DEFAULT_CONFIG.schema.tags.maxCount,
          lowercase: parsedTags?.lowercase ?? oldValidation?.lowercaseTags ?? DEFAULT_CONFIG.schema.tags.lowercase,
        },
      },
      folders: {
        notes: parsed.folders?.notes ?? DEFAULT_CONFIG.folders.notes,
        projects: parsed.folders?.projects ?? DEFAULT_CONFIG.folders.projects,
        archive: parsed.folders?.archive ?? DEFAULT_CONFIG.folders.archive,
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

  getTypeValues(): string[] {
    return this.config.schema.typeValues;
  }

  updateTypeValues(values: string[]): void {
    this.config.schema.typeValues = values;
  }

  getStatusValues(): string[] {
    return this.config.schema.statusValues;
  }

  updateStatusValues(values: string[]): void {
    this.config.schema.statusValues = values;
  }

  getAreaValues(): string[] {
    return this.config.schema.areaValues;
  }

  updateAreaValues(values: string[]): void {
    this.config.schema.areaValues = values;
  }

  getGistConfig(): GistConfig {
    return this.config.schema.gist;
  }

  isGistEnabled(): boolean {
    return this.config.schema.gist.enabled;
  }

  updateGistConfig(gist: Partial<GistConfig>): void {
    this.config.schema.gist = { ...this.config.schema.gist, ...gist };
  }

  getTagsConfig(): { maxCount: number; lowercase: boolean } {
    return this.config.schema.tags;
  }

  updateTagsConfig(tags: Partial<{ maxCount: number; lowercase: boolean }>): void {
    this.config.schema.tags = { ...this.config.schema.tags, ...tags };
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

  getFoldersConfig(): FoldersConfig {
    return this.config.folders;
  }

  getNotesFolder(): string {
    return this.config.folders.notes;
  }

  getProjectsFolder(): string {
    return this.config.folders.projects;
  }

  getArchiveFolder(): string {
    return this.config.folders.archive;
  }

  updateFoldersConfig(folders: Partial<FoldersConfig>): void {
    this.config.folders = { ...this.config.folders, ...folders };
  }

  static getDefault(): ElysiumConfigData {
    return { ...DEFAULT_CONFIG };
  }
}

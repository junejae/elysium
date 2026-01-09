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

export interface FilterableField {
  name: string;
  values: string[];
  filterable: boolean;
  required: boolean;
}

export interface SchemaConfig {
  filterableFields: Record<string, FilterableField>;
  gist: GistConfig;
  tags: { name: string; maxCount: number; lowercase: boolean };
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
  version: 2,
  schema: {
    filterableFields: {
      type: { name: 'type', values: ['note', 'term', 'project', 'log'], filterable: true, required: true },
      area: { name: 'area', values: ['work', 'tech', 'life', 'career', 'learning', 'reference'], filterable: true, required: true },
    },
    gist: {
      enabled: false,
      fieldName: 'gist',
      autoGenerate: true,
      maxLength: 200,
      trackSource: true,
      sourceFieldName: 'gist_source',
      dateFieldName: 'gist_date',
    },
    tags: { name: 'tags', maxCount: 5, lowercase: true },
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

  private migrateFromV1(parsed: any): Record<string, FilterableField> {
    const filterableFields: Record<string, FilterableField> = {};
    const oldFields = parsed.schema?.fields;
    const oldValidation = parsed.schema?.validation;

    if (oldFields?.type) {
      filterableFields.type = {
        name: oldFields.type.name ?? 'type',
        values: oldFields.type.values ?? DEFAULT_CONFIG.schema.filterableFields.type.values,
        filterable: true,
        required: oldValidation?.requireType ?? true,
      };
    }
    if (oldFields?.area) {
      filterableFields.area = {
        name: oldFields.area.name ?? 'area',
        values: oldFields.area.values ?? DEFAULT_CONFIG.schema.filterableFields.area.values,
        filterable: true,
        required: oldValidation?.requireArea ?? true,
      };
    }

    return filterableFields;
  }

  private mergeWithDefaults(parsed: Partial<ElysiumConfigData>): ElysiumConfigData {
    const parsedGist = parsed.schema?.gist as Partial<GistConfig> | undefined;
    
    let filterableFields: Record<string, FilterableField>;
    if (parsed.schema?.filterableFields) {
      filterableFields = { ...DEFAULT_CONFIG.schema.filterableFields, ...parsed.schema.filterableFields };
    } else if ((parsed as any).schema?.fields) {
      filterableFields = { ...DEFAULT_CONFIG.schema.filterableFields, ...this.migrateFromV1(parsed) };
    } else {
      filterableFields = { ...DEFAULT_CONFIG.schema.filterableFields };
    }

    const parsedTags = parsed.schema?.tags ?? (parsed as any).schema?.fields?.tags;
    const oldValidation = (parsed as any).schema?.validation;
    
    return {
      version: 2,
      schema: {
        filterableFields,
        gist: {
          enabled: parsedGist?.enabled ?? DEFAULT_CONFIG.schema.gist.enabled,
          fieldName: parsedGist?.fieldName ?? DEFAULT_CONFIG.schema.gist.fieldName,
          autoGenerate: parsedGist?.autoGenerate ?? DEFAULT_CONFIG.schema.gist.autoGenerate,
          maxLength: parsedGist?.maxLength ?? DEFAULT_CONFIG.schema.gist.maxLength,
          trackSource: parsedGist?.trackSource ?? DEFAULT_CONFIG.schema.gist.trackSource,
          sourceFieldName: parsedGist?.sourceFieldName ?? DEFAULT_CONFIG.schema.gist.sourceFieldName,
          dateFieldName: parsedGist?.dateFieldName ?? DEFAULT_CONFIG.schema.gist.dateFieldName,
        },
        tags: {
          name: parsedTags?.name ?? DEFAULT_CONFIG.schema.tags.name,
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

  getFilterableFields(): Record<string, FilterableField> {
    return this.config.schema.filterableFields;
  }

  getFilterableFieldKeys(): string[] {
    return Object.entries(this.config.schema.filterableFields)
      .filter(([_, field]) => field.filterable)
      .map(([key]) => key);
  }

  getFieldConfig(key: string): FilterableField | undefined {
    return this.config.schema.filterableFields[key];
  }

  getFieldName(key: string): string {
    return this.config.schema.filterableFields[key]?.name ?? key;
  }

  getFieldValues(key: string): string[] {
    return this.config.schema.filterableFields[key]?.values ?? [];
  }

  addFilterableField(key: string, field: FilterableField): void {
    this.config.schema.filterableFields[key] = field;
  }

  updateFilterableField(key: string, updates: Partial<FilterableField>): void {
    if (this.config.schema.filterableFields[key]) {
      this.config.schema.filterableFields[key] = {
        ...this.config.schema.filterableFields[key],
        ...updates,
      };
    }
  }

  removeFilterableField(key: string): void {
    delete this.config.schema.filterableFields[key];
  }

  getTypeFieldName(): string {
    return this.getFieldName('type');
  }

  getTypeValues(): string[] {
    return this.getFieldValues('type');
  }

  getAreaFieldName(): string {
    return this.getFieldName('area');
  }

  getAreaValues(): string[] {
    return this.getFieldValues('area');
  }

  getGistFieldName(): string {
    return this.config.schema.gist.fieldName;
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

  getTagsFieldName(): string {
    return this.config.schema.tags.name;
  }

  getTagsConfig(): { name: string; maxCount: number; lowercase: boolean } {
    return this.config.schema.tags;
  }

  updateTagsConfig(tags: Partial<{ name: string; maxCount: number; lowercase: boolean }>): void {
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

import fs from 'node:fs';
import path from 'node:path';

const EXPECTED_VERSION = 1;

type IndexMeta = {
  embeddingMode: string;
  dimension: number;
  noteCount: number;
  indexSize: number;
  exportedAt: number;
  version: number;
};

type NoteRecord = {
  path: string;
  gist: string;
  mtime: number;
  indexed: boolean;
  fields: Record<string, string | string[]>;
  tags?: string[];
};

type Options = {
  indexDir?: string;
  vaultRoot?: string;
  allowMissingHnsw: boolean;
};

const fail = (message: string): never => {
  console.error(`[smoke-index-export] ${message}`);
  process.exit(1);
};

const parseArgs = (args: string[]): Options => {
  const options: Options = { allowMissingHnsw: false };
  for (let i = 0; i < args.length; i += 1) {
    const arg = args[i];
    if (arg === '--index-dir' && args[i + 1]) {
      options.indexDir = args[i + 1];
      i += 1;
      continue;
    }
    if (arg === '--vault' && args[i + 1]) {
      options.vaultRoot = args[i + 1];
      i += 1;
      continue;
    }
    if (arg === '--allow-missing-hnsw') {
      options.allowMissingHnsw = true;
      continue;
    }
  }
  return options;
};

const readJson = <T>(filePath: string): T => {
  const raw = fs.readFileSync(filePath, 'utf8');
  return JSON.parse(raw) as T;
};

const isStringArray = (value: unknown): value is string[] => {
  return Array.isArray(value) && value.every((item) => typeof item === 'string');
};

const validateMeta = (meta: IndexMeta) => {
  if (!meta) fail('meta.json is empty or invalid');
  if (meta.version !== EXPECTED_VERSION) {
    fail(`meta.json version mismatch: expected ${EXPECTED_VERSION}, found ${meta.version}`);
  }
  if (!['htp', 'model2vec'].includes(meta.embeddingMode)) {
    fail(`meta.json embeddingMode must be 'htp' or 'model2vec' (got '${meta.embeddingMode}')`);
  }
  if (!Number.isFinite(meta.dimension) || meta.dimension <= 0) {
    fail('meta.json dimension must be a positive number');
  }
};

const validateNotes = (notes: NoteRecord[]) => {
  if (!Array.isArray(notes)) fail('notes.json must be an array');
  if (notes.length === 0) fail('notes.json is empty');

  for (const note of notes) {
    if (typeof note.path !== 'string' || note.path.length === 0) {
      fail('notes.json: each record must include a non-empty path');
    }
    if (typeof note.gist !== 'string') {
      fail(`notes.json: gist must be string (${note.path})`);
    }
    if (!Number.isFinite(note.mtime)) {
      fail(`notes.json: mtime must be number (${note.path})`);
    }
    if (typeof note.indexed !== 'boolean') {
      fail(`notes.json: indexed must be boolean (${note.path})`);
    }
    if (!note.fields || typeof note.fields !== 'object') {
      fail(`notes.json: fields must be object (${note.path})`);
    }
    for (const value of Object.values(note.fields)) {
      if (typeof value !== 'string' && !isStringArray(value)) {
        fail(`notes.json: fields values must be string or string[] (${note.path})`);
      }
    }
    if (note.tags && !isStringArray(note.tags)) {
      fail(`notes.json: tags must be string[] (${note.path})`);
    }
  }
};

const main = () => {
  const options = parseArgs(process.argv.slice(2));
  const vaultRoot = options.vaultRoot ?? process.cwd();
  const indexDir = options.indexDir ?? path.join(vaultRoot, '.obsidian/plugins/elysium/index');

  if (!fs.existsSync(indexDir)) {
    fail(`index directory not found: ${indexDir} (pass --index-dir or --vault)`);
  }

  const metaPath = path.join(indexDir, 'meta.json');
  const notesPath = path.join(indexDir, 'notes.json');
  const hnswPath = path.join(indexDir, 'hnsw.bin');

  if (!fs.existsSync(metaPath)) fail(`missing meta.json at ${metaPath}`);
  if (!fs.existsSync(notesPath)) fail(`missing notes.json at ${notesPath}`);

  if (!options.allowMissingHnsw && !fs.existsSync(hnswPath)) {
    fail(`missing hnsw.bin at ${hnswPath} (use --allow-missing-hnsw to ignore)`);
  }

  const meta = readJson<IndexMeta>(metaPath);
  const notes = readJson<NoteRecord[]>(notesPath);

  validateMeta(meta);
  validateNotes(notes);

  console.log(`[smoke-index-export] OK: ${notes.length} notes, version ${meta.version}`);
};

main();

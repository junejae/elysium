const DB_NAME = 'elysium';
const DB_VERSION = 2;

const STORE_NOTES = 'notes';
const STORE_INDEX = 'hnsw_index';
const STORE_META = 'metadata';

export interface NoteRecord {
  path: string;
  gist: string;
  mtime: number;
  indexed: boolean;
  type?: string;
  area?: string;
  tags?: string[];
}

export class IndexedDbStorage {
  private db: IDBDatabase | null = null;

  async open(): Promise<void> {
    return new Promise((resolve, reject) => {
      const request = indexedDB.open(DB_NAME, DB_VERSION);

      request.onupgradeneeded = (event) => {
        const db = (event.target as IDBOpenDBRequest).result;
        const oldVersion = event.oldVersion;

        if (!db.objectStoreNames.contains(STORE_NOTES)) {
          const noteStore = db.createObjectStore(STORE_NOTES, { keyPath: 'path' });
          noteStore.createIndex('mtime', 'mtime');
          noteStore.createIndex('indexed', 'indexed');
          noteStore.createIndex('type', 'type');
          noteStore.createIndex('area', 'area');
        } else if (oldVersion < 2) {
          const tx = (event.target as IDBOpenDBRequest).transaction!;
          const noteStore = tx.objectStore(STORE_NOTES);
          if (!noteStore.indexNames.contains('type')) {
            noteStore.createIndex('type', 'type');
          }
          if (!noteStore.indexNames.contains('area')) {
            noteStore.createIndex('area', 'area');
          }
        }

        if (!db.objectStoreNames.contains(STORE_INDEX)) {
          db.createObjectStore(STORE_INDEX, { keyPath: 'id' });
        }

        if (!db.objectStoreNames.contains(STORE_META)) {
          db.createObjectStore(STORE_META, { keyPath: 'key' });
        }
      };

      request.onsuccess = () => {
        this.db = request.result;
        resolve();
      };

      request.onerror = () => reject(request.error);
    });
  }

  async close(): Promise<void> {
    this.db?.close();
    this.db = null;
  }

  async saveNote(record: NoteRecord): Promise<void> {
    if (!this.db) throw new Error('Database not open');

    return new Promise((resolve, reject) => {
      const tx = this.db!.transaction(STORE_NOTES, 'readwrite');
      const store = tx.objectStore(STORE_NOTES);
      const request = store.put(record);

      request.onsuccess = () => resolve();
      request.onerror = () => reject(request.error);
    });
  }

  async getNote(path: string): Promise<NoteRecord | null> {
    if (!this.db) throw new Error('Database not open');

    return new Promise((resolve, reject) => {
      const tx = this.db!.transaction(STORE_NOTES, 'readonly');
      const store = tx.objectStore(STORE_NOTES);
      const request = store.get(path);

      request.onsuccess = () => resolve(request.result ?? null);
      request.onerror = () => reject(request.error);
    });
  }

  async deleteNote(path: string): Promise<void> {
    if (!this.db) throw new Error('Database not open');

    return new Promise((resolve, reject) => {
      const tx = this.db!.transaction(STORE_NOTES, 'readwrite');
      const store = tx.objectStore(STORE_NOTES);
      const request = store.delete(path);

      request.onsuccess = () => resolve();
      request.onerror = () => reject(request.error);
    });
  }

  async getAllNotes(): Promise<NoteRecord[]> {
    if (!this.db) throw new Error('Database not open');

    return new Promise((resolve, reject) => {
      const tx = this.db!.transaction(STORE_NOTES, 'readonly');
      const store = tx.objectStore(STORE_NOTES);
      const request = store.getAll();

      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error);
    });
  }

  async getNoteCount(): Promise<number> {
    if (!this.db) throw new Error('Database not open');

    return new Promise((resolve, reject) => {
      const tx = this.db!.transaction(STORE_NOTES, 'readonly');
      const store = tx.objectStore(STORE_NOTES);
      const request = store.count();

      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error);
    });
  }

  async saveHnswIndex(data: Uint8Array): Promise<void> {
    if (!this.db) throw new Error('Database not open');

    return new Promise((resolve, reject) => {
      const tx = this.db!.transaction(STORE_INDEX, 'readwrite');
      const store = tx.objectStore(STORE_INDEX);
      const request = store.put({ id: 'main', data, timestamp: Date.now() });

      request.onsuccess = () => resolve();
      request.onerror = () => reject(request.error);
    });
  }

  async loadHnswIndex(): Promise<Uint8Array | null> {
    if (!this.db) throw new Error('Database not open');

    return new Promise((resolve, reject) => {
      const tx = this.db!.transaction(STORE_INDEX, 'readonly');
      const store = tx.objectStore(STORE_INDEX);
      const request = store.get('main');

      request.onsuccess = () => {
        const result = request.result;
        resolve(result?.data ?? null);
      };
      request.onerror = () => reject(request.error);
    });
  }

  async saveMeta(key: string, value: unknown): Promise<void> {
    if (!this.db) throw new Error('Database not open');

    return new Promise((resolve, reject) => {
      const tx = this.db!.transaction(STORE_META, 'readwrite');
      const store = tx.objectStore(STORE_META);
      const request = store.put({ key, value });

      request.onsuccess = () => resolve();
      request.onerror = () => reject(request.error);
    });
  }

  async getMeta<T>(key: string): Promise<T | null> {
    if (!this.db) throw new Error('Database not open');

    return new Promise((resolve, reject) => {
      const tx = this.db!.transaction(STORE_META, 'readonly');
      const store = tx.objectStore(STORE_META);
      const request = store.get(key);

      request.onsuccess = () => {
        const result = request.result;
        resolve(result?.value ?? null);
      };
      request.onerror = () => reject(request.error);
    });
  }

  async clearAll(): Promise<void> {
    if (!this.db) throw new Error('Database not open');

    const stores = [STORE_NOTES, STORE_INDEX, STORE_META];
    
    for (const storeName of stores) {
      await new Promise<void>((resolve, reject) => {
        const tx = this.db!.transaction(storeName, 'readwrite');
        const store = tx.objectStore(storeName);
        const request = store.clear();

        request.onsuccess = () => resolve();
        request.onerror = () => reject(request.error);
      });
    }
  }
}

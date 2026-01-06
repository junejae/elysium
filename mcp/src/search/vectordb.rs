//! Vector database using SQLite
//!
//! Stores embeddings as BLOBs and computes similarity in Rust.
//! Can be upgraded to sqlite-vec for native vector operations later.

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

use super::embedding::{cosine_similarity, EMBEDDING_DIM};

/// Vector database for note embeddings
pub struct VectorDB {
    conn: Connection,
}

/// Note metadata stored alongside embeddings
#[derive(Debug, Clone)]
pub struct NoteRecord {
    pub id: String,
    pub path: String,
    pub title: String,
    pub gist: Option<String>,
    pub note_type: Option<String>,
    pub status: Option<String>,
    pub area: Option<String>,
    pub tags: Vec<String>,
    pub mtime: i64,
}

impl VectorDB {
    /// Open or create database at path
    pub fn open(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    /// Open in-memory database (for testing)
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    /// Initialize database schema
    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            -- Notes metadata
            CREATE TABLE IF NOT EXISTS notes (
                id TEXT PRIMARY KEY,
                path TEXT NOT NULL UNIQUE,
                title TEXT NOT NULL,
                gist TEXT,
                note_type TEXT,
                status TEXT,
                area TEXT,
                tags TEXT,  -- JSON array
                mtime INTEGER NOT NULL,
                indexed_at INTEGER NOT NULL
            );

            -- Embeddings (stored as BLOB for now, can migrate to sqlite-vec later)
            CREATE TABLE IF NOT EXISTS embeddings (
                note_id TEXT PRIMARY KEY,
                embedding BLOB NOT NULL,
                FOREIGN KEY (note_id) REFERENCES notes(id) ON DELETE CASCADE
            );

            -- Index metadata
            CREATE TABLE IF NOT EXISTS index_meta (
                key TEXT PRIMARY KEY,
                value TEXT
            );

            -- Indexes
            CREATE INDEX IF NOT EXISTS idx_notes_path ON notes(path);
            CREATE INDEX IF NOT EXISTS idx_notes_type ON notes(note_type);
            CREATE INDEX IF NOT EXISTS idx_notes_area ON notes(area);
            CREATE INDEX IF NOT EXISTS idx_notes_mtime ON notes(mtime);
            "#,
        )?;

        Ok(())
    }

    /// Insert or update note with embedding
    pub fn upsert_note(&self, note: &NoteRecord, embedding: &[f32]) -> Result<()> {
        let tags_json = serde_json::to_string(&note.tags)?;
        let embedding_blob = embedding_to_blob(embedding);
        let now = chrono::Utc::now().timestamp();

        self.conn.execute(
            r#"
            INSERT INTO notes (id, path, title, gist, note_type, status, area, tags, mtime, indexed_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT(id) DO UPDATE SET
                path = excluded.path,
                title = excluded.title,
                gist = excluded.gist,
                note_type = excluded.note_type,
                status = excluded.status,
                area = excluded.area,
                tags = excluded.tags,
                mtime = excluded.mtime,
                indexed_at = excluded.indexed_at
            "#,
            params![
                note.id,
                note.path,
                note.title,
                note.gist,
                note.note_type,
                note.status,
                note.area,
                tags_json,
                note.mtime,
                now,
            ],
        )?;

        self.conn.execute(
            r#"
            INSERT INTO embeddings (note_id, embedding)
            VALUES (?1, ?2)
            ON CONFLICT(note_id) DO UPDATE SET embedding = excluded.embedding
            "#,
            params![note.id, embedding_blob],
        )?;

        Ok(())
    }

    /// Delete note by ID
    pub fn delete_note(&self, id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM notes WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Get note by ID
    pub fn get_note(&self, id: &str) -> Result<Option<NoteRecord>> {
        let result = self
            .conn
            .query_row(
                "SELECT id, path, title, gist, note_type, status, area, tags, mtime FROM notes WHERE id = ?1",
                params![id],
                |row| {
                    let tags_json: String = row.get(7)?;
                    let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
                    Ok(NoteRecord {
                        id: row.get(0)?,
                        path: row.get(1)?,
                        title: row.get(2)?,
                        gist: row.get(3)?,
                        note_type: row.get(4)?,
                        status: row.get(5)?,
                        area: row.get(6)?,
                        tags,
                        mtime: row.get(8)?,
                    })
                },
            )
            .optional()?;

        Ok(result)
    }

    /// Search for similar notes using cosine similarity
    pub fn search(&self, query_embedding: &[f32], limit: usize) -> Result<Vec<(NoteRecord, f32)>> {
        // Load all embeddings and compute similarity in Rust
        // This is O(n) but fine for < 10,000 notes
        // Can be optimized with HNSW index or sqlite-vec later

        let mut stmt = self.conn.prepare(
            r#"
            SELECT n.id, n.path, n.title, n.gist, n.note_type, n.status, n.area, n.tags, n.mtime, e.embedding
            FROM notes n
            JOIN embeddings e ON n.id = e.note_id
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            let tags_json: String = row.get(7)?;
            let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
            let embedding_blob: Vec<u8> = row.get(9)?;

            Ok((
                NoteRecord {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    title: row.get(2)?,
                    gist: row.get(3)?,
                    note_type: row.get(4)?,
                    status: row.get(5)?,
                    area: row.get(6)?,
                    tags,
                    mtime: row.get(8)?,
                },
                embedding_blob,
            ))
        })?;

        let mut results: Vec<(NoteRecord, f32)> = Vec::new();

        for row_result in rows {
            let (note, embedding_blob) = row_result?;
            let embedding = blob_to_embedding(&embedding_blob);
            let similarity = cosine_similarity(query_embedding, &embedding);
            results.push((note, similarity));
        }

        // Sort by similarity descending
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);

        Ok(results)
    }

    /// Get index statistics
    pub fn get_stats(&self) -> Result<IndexStats> {
        let note_count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))?;

        let embedding_count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM embeddings", [], |row| row.get(0))?;

        let last_indexed: Option<i64> = self
            .conn
            .query_row(
                "SELECT MAX(indexed_at) FROM notes",
                [],
                |row| row.get(0),
            )
            .optional()?
            .flatten();

        Ok(IndexStats {
            note_count: note_count as usize,
            embedding_count: embedding_count as usize,
            last_indexed,
        })
    }

    /// Get all note IDs with their mtimes
    pub fn get_all_mtimes(&self) -> Result<Vec<(String, i64)>> {
        let mut stmt = self.conn.prepare("SELECT id, mtime FROM notes")?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    /// Set index metadata
    pub fn set_meta(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO index_meta (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// Get index metadata
    pub fn get_meta(&self, key: &str) -> Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT value FROM index_meta WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.into())
    }
}

/// Index statistics
#[derive(Debug)]
pub struct IndexStats {
    pub note_count: usize,
    pub embedding_count: usize,
    pub last_indexed: Option<i64>,
}

/// Convert f32 embedding to BLOB
fn embedding_to_blob(embedding: &[f32]) -> Vec<u8> {
    let mut blob = Vec::with_capacity(embedding.len() * 4);
    for &val in embedding {
        blob.extend_from_slice(&val.to_le_bytes());
    }
    blob
}

/// Convert BLOB to f32 embedding
fn blob_to_embedding(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blob_conversion() {
        let embedding = vec![1.0, 2.0, 3.0, -0.5];
        let blob = embedding_to_blob(&embedding);
        let recovered = blob_to_embedding(&blob);
        assert_eq!(embedding, recovered);
    }

    #[test]
    fn test_db_operations() -> Result<()> {
        let db = VectorDB::open_in_memory()?;

        let note = NoteRecord {
            id: "test-note".to_string(),
            path: "Notes/Test Note.md".to_string(),
            title: "Test Note".to_string(),
            gist: Some("This is a test note".to_string()),
            note_type: Some("note".to_string()),
            status: Some("active".to_string()),
            area: Some("tech".to_string()),
            tags: vec!["test".to_string()],
            mtime: 1704067200,
        };

        let embedding = vec![0.1; EMBEDDING_DIM];
        db.upsert_note(&note, &embedding)?;

        let retrieved = db.get_note("test-note")?;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().title, "Test Note");

        let stats = db.get_stats()?;
        assert_eq!(stats.note_count, 1);
        assert_eq!(stats.embedding_count, 1);

        Ok(())
    }
}

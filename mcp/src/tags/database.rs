//! Tag database for storing tag metadata and embeddings
//!
//! Uses SQLite for persistence with pre-computed Model2Vec embeddings.

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

use super::embedder::TagEmbedder;

/// A tag entry in the database
#[derive(Debug, Clone)]
pub struct TagEntry {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub embedding: Vec<f32>,
    pub aliases: Vec<String>,
    pub usage_count: i64,
}

/// Tag database manager
pub struct TagDatabase {
    conn: Connection,
}

impl TagDatabase {
    /// Open or create tag database
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open tag database: {}", path.display()))?;

        let db = Self { conn };
        db.init_schema()?;

        Ok(db)
    }

    /// Initialize database schema
    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS tags (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT UNIQUE NOT NULL,
                description TEXT NOT NULL,
                embedding BLOB NOT NULL,
                usage_count INTEGER DEFAULT 0,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS tag_aliases (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
                alias TEXT UNIQUE NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_tags_name ON tags(name);
            CREATE INDEX IF NOT EXISTS idx_aliases_alias ON tag_aliases(alias);
            "#,
        )?;

        Ok(())
    }

    /// Get all tags with their embeddings
    pub fn get_all_tags(&self) -> Result<Vec<TagEntry>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT t.id, t.name, t.description, t.embedding, t.usage_count,
                   GROUP_CONCAT(a.alias, ',') as aliases
            FROM tags t
            LEFT JOIN tag_aliases a ON t.id = a.tag_id
            GROUP BY t.id
            ORDER BY t.usage_count DESC
            "#,
        )?;

        let tags = stmt
            .query_map([], |row| {
                let embedding_blob: Vec<u8> = row.get(3)?;
                let embedding = bytes_to_embedding(&embedding_blob);
                let aliases_str: Option<String> = row.get(5)?;
                let aliases = aliases_str
                    .map(|s| s.split(',').map(String::from).collect())
                    .unwrap_or_default();

                Ok(TagEntry {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    embedding,
                    aliases,
                    usage_count: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(tags)
    }

    /// Get a tag by name
    pub fn get_tag(&self, name: &str) -> Result<Option<TagEntry>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT t.id, t.name, t.description, t.embedding, t.usage_count,
                   GROUP_CONCAT(a.alias, ',') as aliases
            FROM tags t
            LEFT JOIN tag_aliases a ON t.id = a.tag_id
            WHERE t.name = ?1
            GROUP BY t.id
            "#,
        )?;

        let tag = stmt
            .query_row([name], |row| {
                let embedding_blob: Vec<u8> = row.get(3)?;
                let embedding = bytes_to_embedding(&embedding_blob);
                let aliases_str: Option<String> = row.get(5)?;
                let aliases = aliases_str
                    .map(|s| s.split(',').map(String::from).collect())
                    .unwrap_or_default();

                Ok(TagEntry {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    embedding,
                    aliases,
                    usage_count: row.get(4)?,
                })
            })
            .optional()?;

        Ok(tag)
    }

    /// Add a new tag with auto-generated embedding
    pub fn add_tag(&self, name: &str, description: &str, embedder: &TagEmbedder) -> Result<i64> {
        // Generate embedding from description
        let embedding = embedder.embed(description)?;
        let embedding_blob = embedding_to_bytes(&embedding);

        self.conn.execute(
            "INSERT INTO tags (name, description, embedding) VALUES (?1, ?2, ?3)",
            params![name, description, embedding_blob],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Add a tag with pre-computed embedding
    pub fn add_tag_with_embedding(
        &self,
        name: &str,
        description: &str,
        embedding: &[f32],
    ) -> Result<i64> {
        let embedding_blob = embedding_to_bytes(embedding);

        self.conn.execute(
            "INSERT INTO tags (name, description, embedding) VALUES (?1, ?2, ?3)",
            params![name, description, embedding_blob],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Add an alias to a tag
    pub fn add_alias(&self, tag_name: &str, alias: &str) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO tag_aliases (tag_id, alias)
            SELECT id, ?2 FROM tags WHERE name = ?1
            "#,
            params![tag_name, alias],
        )?;

        Ok(())
    }

    /// Increment usage count for a tag
    pub fn increment_usage(&self, tag_name: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE tags SET usage_count = usage_count + 1, updated_at = CURRENT_TIMESTAMP WHERE name = ?1",
            [tag_name],
        )?;

        Ok(())
    }

    /// Get tag count
    pub fn tag_count(&self) -> Result<i64> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM tags", [], |row| row.get(0))?;

        Ok(count)
    }

    /// Check if database is empty (needs seeding)
    pub fn is_empty(&self) -> Result<bool> {
        Ok(self.tag_count()? == 0)
    }

    /// Find tag by name or alias
    pub fn find_tag(&self, name_or_alias: &str) -> Result<Option<TagEntry>> {
        // First try exact name match
        if let Some(tag) = self.get_tag(name_or_alias)? {
            return Ok(Some(tag));
        }

        // Try alias match
        let mut stmt = self.conn.prepare(
            r#"
            SELECT t.id, t.name, t.description, t.embedding, t.usage_count
            FROM tags t
            JOIN tag_aliases a ON t.id = a.tag_id
            WHERE a.alias = ?1
            "#,
        )?;

        let tag = stmt
            .query_row([name_or_alias], |row| {
                let embedding_blob: Vec<u8> = row.get(3)?;
                let embedding = bytes_to_embedding(&embedding_blob);

                Ok(TagEntry {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    embedding,
                    aliases: vec![],
                    usage_count: row.get(4)?,
                })
            })
            .optional()?;

        Ok(tag)
    }
}

/// Convert f32 vector to bytes for storage
fn embedding_to_bytes(embedding: &[f32]) -> Vec<u8> {
    embedding.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Convert bytes back to f32 vector
fn bytes_to_embedding(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| {
            let arr: [u8; 4] = chunk.try_into().unwrap();
            f32::from_le_bytes(arr)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::embedder::EMBEDDING_DIM;

    #[test]
    fn test_embedding_conversion() {
        let original = vec![0.1, 0.2, 0.3, -0.5];
        let bytes = embedding_to_bytes(&original);
        let recovered = bytes_to_embedding(&bytes);

        assert_eq!(original.len(), recovered.len());
        for (a, b) in original.iter().zip(recovered.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn test_database_basic() {
        let db = TagDatabase::open(Path::new(":memory:")).unwrap();

        assert!(db.is_empty().unwrap());

        // Add tag with manual embedding
        let fake_embedding = vec![0.0; EMBEDDING_DIM];
        db.add_tag_with_embedding("gpu", "GPU hardware and VRAM", &fake_embedding)
            .unwrap();

        assert!(!db.is_empty().unwrap());

        let tag = db.get_tag("gpu").unwrap().unwrap();
        assert_eq!(tag.name, "gpu");
        assert_eq!(tag.description, "GPU hardware and VRAM");
    }
}

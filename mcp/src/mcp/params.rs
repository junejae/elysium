//! Parameter structures for MCP tools

use schemars::JsonSchema;
use serde::Deserialize;

/// Parameters for vault_search tool
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchParams {
    /// Natural language search query (e.g., "GPU memory sharing methods")
    #[schemars(description = "Natural language search query")]
    pub query: String,
    /// Maximum number of results to return (default: 5)
    #[schemars(description = "Maximum number of results (default: 5)")]
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// Filter by note type (note, term, project, log, lesson)
    #[schemars(description = "Filter by type: note, term, project, log, lesson")]
    #[serde(default)]
    pub note_type: Option<String>,
    /// Filter by area (work, tech, life, career, learning, reference, defense, prosecutor, judge)
    #[schemars(
        description = "Filter by area: work, tech, life, career, learning, reference, defense, prosecutor, judge"
    )]
    #[serde(default)]
    pub area: Option<String>,
    /// Fields to include in output: "default" (title,path,gist), "standard" (+ type,status,area,tags), "all", or comma-separated list
    #[schemars(
        description = "Fields to include: 'default', 'standard', 'all', or comma-separated (e.g., 'title,gist,source')"
    )]
    #[serde(default)]
    pub fields: Option<String>,
    /// Search mode: "hybrid" (BM25 + semantic, default), "semantic" (HNSW only), "keyword" (BM25 only)
    #[schemars(description = "Search mode: 'hybrid' (default), 'semantic', 'keyword'")]
    #[serde(default)]
    pub search_mode: Option<String>,
}

pub fn default_limit() -> usize {
    5
}

/// Parameters for vault_get_note tool
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetNoteParams {
    /// Note title (e.g., "GPU 기술 허브")
    #[schemars(description = "Note title to retrieve")]
    pub note: String,
    /// Fields to include in metadata: "default" (title,path,gist), "standard" (+ type,status,area,tags), "all", or comma-separated list
    #[schemars(
        description = "Fields to include: 'default', 'standard', 'all', or comma-separated (e.g., 'title,gist,source')"
    )]
    #[serde(default)]
    pub fields: Option<String>,
}

/// Parameters for vault_list_notes tool
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListNotesParams {
    /// Filter by note type (note, term, project, log)
    #[schemars(description = "Filter by type: note, term, project, log")]
    #[serde(default)]
    pub note_type: Option<String>,
    /// Filter by area (work, tech, life, career, learning, reference)
    #[schemars(description = "Filter by area: work, tech, life, career, learning, reference")]
    #[serde(default)]
    pub area: Option<String>,
    /// Maximum number of results (default: 50)
    #[schemars(description = "Maximum results (default: 50)")]
    #[serde(default = "default_list_limit")]
    pub limit: usize,
    /// Fields to include in output: "default" (title,path,gist), "standard" (+ type,status,area,tags), "all", or comma-separated list
    #[schemars(
        description = "Fields to include: 'default', 'standard', 'all', or comma-separated (e.g., 'title,gist,source')"
    )]
    #[serde(default)]
    pub fields: Option<String>,
}

pub fn default_list_limit() -> usize {
    50
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RelatedParams {
    #[schemars(description = "Note title to find related notes for")]
    pub note: String,
    #[schemars(description = "Maximum number of results (default: 10)")]
    #[serde(default = "default_related_limit")]
    pub limit: usize,
    #[schemars(description = "Boost notes with same type as source")]
    #[serde(default)]
    #[allow(dead_code)]
    pub boost_type: bool,
    #[schemars(description = "Boost notes with same area as source")]
    #[serde(default)]
    #[allow(dead_code)]
    pub boost_area: bool,
}

pub fn default_related_limit() -> usize {
    10
}

/// Parameters for vault_audit tool
#[derive(Debug, Deserialize, JsonSchema)]
pub struct AuditParams {
    /// Quick mode: schema + wikilinks only
    #[schemars(description = "Quick mode: schema + wikilinks only")]
    #[serde(default)]
    pub quick: bool,

    /// Include detailed error messages
    #[schemars(description = "Include detailed error list per check")]
    #[serde(default)]
    pub verbose: bool,
}

/// Parameters for unified vault_save tool
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SaveParams {
    /// Note title (used as filename for create, or to find existing note)
    #[schemars(description = "Note title (filename for create, or search key for update/append)")]
    pub title: String,

    /// Note content (markdown body, or memo for inbox strategy)
    #[schemars(description = "Note content (markdown body)")]
    pub content: String,

    /// Save strategy: create, update, append, inbox, smart
    #[schemars(
        description = "Save strategy: 'create' (new note), 'update' (overwrite existing), 'append' (add to existing), 'inbox' (quick capture), 'smart' (auto-detect duplicates)"
    )]
    #[serde(default = "default_strategy")]
    pub strategy: String,

    /// Note type: note, term, project, log
    #[schemars(description = "Note type: note, term, project, log")]
    #[serde(default)]
    pub note_type: Option<String>,

    /// Note area: work, tech, life, career, learning, reference
    #[schemars(description = "Note area: work, tech, life, career, learning, reference")]
    #[serde(default)]
    pub area: Option<String>,

    /// Tags (comma-separated)
    #[schemars(description = "Tags (comma-separated, e.g., 'gpu, cuda, nvidia')")]
    #[serde(default)]
    pub tags: Option<String>,

    /// Gist summary (2-3 sentences for semantic search)
    #[schemars(description = "Gist summary (2-3 sentences for semantic search)")]
    #[serde(default)]
    pub gist: Option<String>,

    /// Source URLs (for web research notes, comma-separated)
    #[schemars(
        description = "Source URLs (comma-separated, e.g., 'https://a.com, https://b.com')"
    )]
    #[serde(default)]
    pub source: Option<String>,

    /// Similarity threshold for smart strategy (default: 0.7)
    #[schemars(description = "Similarity threshold for smart strategy (0.0-1.0, default: 0.7)")]
    #[serde(default = "default_similarity_threshold")]
    pub similarity_threshold: Option<f32>,

    /// Auto-generate tags based on gist/title (default: true)
    #[schemars(description = "Auto-generate tags using semantic matching (default: true)")]
    #[serde(default = "default_auto_tag")]
    pub auto_tag: bool,

    /// Maximum number of auto-generated tags (default: 5)
    #[schemars(description = "Maximum number of auto-generated tags (default: 5)")]
    #[serde(default = "default_tag_limit")]
    pub tag_limit: usize,

    /// Enable tag discovery from content keywords (not just DB match)
    #[schemars(description = "Enable tag discovery from content keywords (default: false)")]
    #[serde(default)]
    pub discover: bool,
}

pub fn default_auto_tag() -> bool {
    true
}

pub fn default_tag_limit() -> usize {
    5
}

pub fn default_strategy() -> String {
    "create".to_string()
}

/// Parameters for vault_tags_suggest tool
#[allow(dead_code)]
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TagsSuggestParams {
    /// Text to analyze for tag suggestions (gist or title)
    #[schemars(description = "Text to analyze for tag suggestions")]
    pub text: String,

    /// Maximum number of tag suggestions (default: 5)
    #[schemars(description = "Maximum number of suggestions (default: 5)")]
    #[serde(default = "default_tag_limit")]
    pub limit: usize,
}

/// Parameters for vault_tags_analyze tool
#[allow(dead_code)]
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TagsAnalyzeParams {
    /// Similarity threshold for merge suggestions (default: 0.7)
    #[schemars(description = "Similarity threshold for merge suggestions (0.0-1.0, default: 0.7)")]
    #[serde(default = "default_merge_threshold")]
    pub threshold: f32,
}

/// Parameters for vault_suggest_tags (advanced search based)
#[allow(dead_code)]
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SuggestTagsParams {
    /// Note title or path to analyze
    #[schemars(description = "Note title or path to find similar notes and suggest tags from")]
    pub note: String,

    /// Maximum number of tag suggestions (default: 5)
    #[schemars(description = "Maximum number of tag suggestions (default: 5)")]
    #[serde(default = "default_tag_limit")]
    pub limit: usize,

    /// Number of similar notes to analyze (default: 10)
    #[schemars(description = "Number of similar notes to analyze for tags (default: 10)")]
    #[serde(default = "default_similar_notes")]
    pub similar_count: usize,

    /// Minimum frequency threshold (default: 2)
    #[schemars(
        description = "Minimum number of occurrences for a tag to be suggested (default: 2)"
    )]
    #[serde(default = "default_min_frequency")]
    pub min_frequency: usize,
}

#[allow(dead_code)]
pub fn default_similar_notes() -> usize {
    10
}

#[allow(dead_code)]
pub fn default_min_frequency() -> usize {
    2
}

#[allow(dead_code)]
pub fn default_merge_threshold() -> f32 {
    0.7
}

pub fn default_similarity_threshold() -> Option<f32> {
    Some(0.7)
}

//! Tag matcher for suggesting tags based on semantic similarity
//!
//! Uses Model2Vec embeddings to match note gists to relevant tags.

use anyhow::Result;
use serde::Serialize;
use std::collections::HashSet;

use super::database::TagDatabase;
use super::embedder::TagEmbedder;
use super::keyword::KeywordExtractor;

/// A suggested tag with confidence score
#[derive(Debug, Clone, Serialize)]
pub struct TagSuggestion {
    pub tag: String,
    pub score: f32,
    pub reason: String,
}

/// Tag matcher combining keyword and semantic matching
pub struct TagMatcher {
    embedder: TagEmbedder,
    database: TagDatabase,
    /// Minimum similarity threshold for suggestions
    threshold: f32,
}

impl TagMatcher {
    /// Create a new tag matcher
    pub fn new(embedder: TagEmbedder, database: TagDatabase) -> Self {
        Self {
            embedder,
            database,
            threshold: 0.3, // Default threshold
        }
    }

    /// Set similarity threshold
    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.threshold = threshold;
        self
    }

    /// Suggest tags for given text (gist or title)
    pub fn suggest_tags(&self, text: &str, limit: usize) -> Result<Vec<TagSuggestion>> {
        // Get text embedding
        let text_embedding = self.embedder.embed(text)?;

        // Get all tags from database
        let tags = self.database.get_all_tags()?;

        // Calculate similarities
        let mut suggestions: Vec<TagSuggestion> = tags
            .iter()
            .map(|tag| {
                let score = TagEmbedder::cosine_similarity(&text_embedding, &tag.embedding);
                TagSuggestion {
                    tag: tag.name.clone(),
                    score,
                    reason: format!("Semantic match: {:.0}%", score * 100.0),
                }
            })
            .filter(|s| s.score >= self.threshold)
            .collect();

        // Sort by score descending
        suggestions.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

        // Limit results
        suggestions.truncate(limit);

        Ok(suggestions)
    }

    /// Hybrid suggestion: keyword + semantic
    pub fn suggest_tags_hybrid(&self, text: &str, limit: usize) -> Result<Vec<TagSuggestion>> {
        let mut suggestions = Vec::new();
        let text_lower = text.to_lowercase();

        // Get all tags
        let tags = self.database.get_all_tags()?;

        // Phase 1: Keyword matching (fast)
        for tag in &tags {
            // Check if tag name or alias appears in text
            if text_lower.contains(&tag.name) {
                suggestions.push(TagSuggestion {
                    tag: tag.name.clone(),
                    score: 1.0, // Perfect match
                    reason: "Keyword match".to_string(),
                });
                continue;
            }

            // Check aliases
            for alias in &tag.aliases {
                if text_lower.contains(alias) {
                    suggestions.push(TagSuggestion {
                        tag: tag.name.clone(),
                        score: 0.95,
                        reason: format!("Alias match: {}", alias),
                    });
                    break;
                }
            }
        }

        // Phase 2: Semantic matching
        let text_embedding = self.embedder.embed(text)?;

        for tag in &tags {
            // Skip if already matched by keyword
            if suggestions.iter().any(|s| s.tag == tag.name) {
                continue;
            }

            let score = TagEmbedder::cosine_similarity(&text_embedding, &tag.embedding);

            if score >= self.threshold {
                suggestions.push(TagSuggestion {
                    tag: tag.name.clone(),
                    score,
                    reason: format!("Semantic match: {:.0}%", score * 100.0),
                });
            }
        }

        // Sort by score descending
        suggestions.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

        // Limit results
        suggestions.truncate(limit);

        Ok(suggestions)
    }

    /// Find similar tags (for merge suggestions)
    pub fn find_similar_tags(&self, tag_name: &str, threshold: f32) -> Result<Vec<(String, f32)>> {
        let source_tag = match self.database.get_tag(tag_name)? {
            Some(t) => t,
            None => return Ok(vec![]),
        };

        let all_tags = self.database.get_all_tags()?;

        let mut similar: Vec<(String, f32)> = all_tags
            .iter()
            .filter(|t| t.name != tag_name)
            .map(|t| {
                let score = TagEmbedder::cosine_similarity(&source_tag.embedding, &t.embedding);
                (t.name.clone(), score)
            })
            .filter(|(_, score)| *score >= threshold)
            .collect();

        similar.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        Ok(similar)
    }

    /// Analyze tags and suggest merges
    pub fn analyze_for_merges(&self, threshold: f32) -> Result<Vec<MergeSuggestion>> {
        let tags = self.database.get_all_tags()?;
        let mut suggestions = Vec::new();
        let mut seen_pairs = std::collections::HashSet::new();

        for tag in &tags {
            let similar = self.find_similar_tags(&tag.name, threshold)?;

            for (other_name, score) in similar {
                // Create ordered pair to avoid duplicates
                let pair = if tag.name < other_name {
                    (tag.name.clone(), other_name.clone())
                } else {
                    (other_name.clone(), tag.name.clone())
                };

                if !seen_pairs.contains(&pair) {
                    seen_pairs.insert(pair.clone());

                    // Prefer the one with higher usage
                    let other_tag = self.database.get_tag(&other_name)?.unwrap();
                    let (keep, merge) = if tag.usage_count >= other_tag.usage_count {
                        (tag.name.clone(), other_name)
                    } else {
                        (other_name, tag.name.clone())
                    };

                    suggestions.push(MergeSuggestion {
                        keep,
                        merge,
                        similarity: score,
                    });
                }
            }
        }

        suggestions.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap());

        Ok(suggestions)
    }

    /// Get embedder reference
    pub fn embedder(&self) -> &TagEmbedder {
        &self.embedder
    }

    /// Get database reference
    pub fn database(&self) -> &TagDatabase {
        &self.database
    }

    /// Suggest tags with new tag discovery from content
    ///
    /// This method combines:
    /// 1. Existing tag DB matching (keyword + semantic)
    /// 2. New tag discovery from content keywords
    ///
    /// Returns tags with source indication (existing vs discovered)
    pub fn suggest_tags_with_discovery(
        &self,
        text: &str,
        limit: usize,
        keyword_extractor: Option<&KeywordExtractor>,
    ) -> Result<Vec<TagSuggestion>> {
        // First, get suggestions from existing tag DB
        let mut suggestions = self.suggest_tags_hybrid(text, limit)?;

        // If no keyword extractor provided, just return DB suggestions
        let extractor = match keyword_extractor {
            Some(e) => e,
            None => return Ok(suggestions),
        };

        // Extract keywords from content
        let keywords = extractor.extract_keywords(text, 10)?;

        // Add discovered keywords that aren't already in suggestions
        for keyword in keywords {
            let keyword_lower = keyword.token.to_lowercase();

            // Skip if already suggested from DB
            if suggestions.iter().any(|s| s.tag == keyword_lower) {
                continue;
            }

            // Skip very short keywords (less than 3 chars)
            if keyword_lower.len() < 3 {
                continue;
            }

            // Skip common words (stopwords)
            if is_stopword(&keyword_lower) {
                continue;
            }

            // Add as discovered tag with adjusted score
            // Discovered tags get slightly lower scores than DB matches
            let adjusted_score = keyword.score * 0.8;
            if adjusted_score >= self.threshold {
                suggestions.push(TagSuggestion {
                    tag: keyword_lower,
                    score: adjusted_score,
                    reason: format!("Discovered keyword: {:.0}%", keyword.score * 100.0),
                });
            }
        }

        // Re-sort by score
        suggestions.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

        // Limit results
        suggestions.truncate(limit);

        Ok(suggestions)
    }
}

/// Check if a word is a common stopword
fn is_stopword(word: &str) -> bool {
    // Check exact match first
    if STOPWORDS_SET.contains(word) {
        return true;
    }

    // Check Korean verb/adjective endings (words ending with these patterns)
    for suffix in KOREAN_VERB_SUFFIXES {
        if word.ends_with(suffix) && word.len() > suffix.len() + 3 {
            return true;
        }
    }

    // Check single-character Korean particle suffixes
    // These are common grammatical markers that shouldn't be part of tags
    for particle in KOREAN_PARTICLES {
        if word.ends_with(particle) && word.len() > particle.len() {
            // Make sure the base word (without particle) is at least 2 chars
            let base_len = word.len() - particle.len();
            if base_len >= 2 {
                return true;
            }
        }
    }

    false
}

/// Single-character Korean particles that get attached to words
const KOREAN_PARTICLES: &[&str] = &[
    // Subject/topic markers
    "가", "이", "는", "은", // Object markers
    "를", "을", // Other common particles
    "의", "에", "로", "로", "과", "와", "랑",
];

/// Korean verb/adjective suffixes that indicate this is likely a sentence fragment
const KOREAN_VERB_SUFFIXES: &[&str] = &[
    // Declarative endings
    "입니다",
    "습니다",
    "됩니다",
    "합니다",
    "있습니다",
    "이다",
    "한다",
    "된다",
    "있다",
    "없다",
    "이야",
    "야",
    "이에요",
    "에요",
    "예요",
    // Past tense
    "었다",
    "았다",
    "였다",
    "했다",
    "됐다",
    "었습니다",
    "았습니다",
    "였습니다",
    "했습니다",
    // Question endings
    "니까",
    "습니까",
    "는가",
    "인가",
    "나요",
    "까요",
    // Connective endings
    "하고",
    "하며",
    "하면",
    "해서",
    "하여",
    "하니",
    "으며",
    "으면",
    "으니",
    "어서",
    "아서",
    // Modifier endings
    "하는",
    "되는",
    "있는",
    "없는",
    "같은",
    // Other common endings
    "으로",
    "에서",
    "에게",
    "한테",
    "처럼",
    "같이",
];

lazy_static::lazy_static! {
    static ref STOPWORDS_SET: std::collections::HashSet<&'static str> = {
        let words: &[&str] = &[
            // ===== English stopwords =====
            // Articles & determiners
            "the", "a", "an", "this", "that", "these", "those",
            // Pronouns
            "i", "you", "he", "she", "it", "we", "they", "me", "him", "her", "us", "them",
            "my", "your", "his", "its", "our", "their", "mine", "yours", "ours", "theirs",
            "who", "whom", "whose", "which", "what", "whoever", "whatever",
            // Prepositions
            "in", "on", "at", "to", "for", "of", "with", "by", "from", "up", "down",
            "into", "onto", "upon", "out", "off", "over", "under", "above", "below",
            "between", "among", "through", "during", "before", "after", "about", "against",
            // Conjunctions
            "and", "or", "but", "nor", "so", "yet", "for", "because", "although", "though",
            "while", "whereas", "if", "unless", "until", "since", "when", "where", "whether",
            // Auxiliary verbs
            "is", "am", "are", "was", "were", "be", "been", "being",
            "have", "has", "had", "having", "do", "does", "did", "doing",
            "will", "would", "shall", "should", "may", "might", "must", "can", "could",
            // Common adverbs
            "very", "really", "just", "only", "also", "too", "even", "still", "already",
            "now", "then", "here", "there", "always", "never", "often", "sometimes",
            // Other common words
            "not", "no", "yes", "all", "any", "some", "each", "every", "both", "few", "more",
            "most", "other", "such", "own", "same", "than", "as", "how", "why",

            // ===== Korean stopwords (한국어 불용어) =====
            // Particles (조사)
            "이", "가", "은", "는", "을", "를", "의", "에", "에서", "으로", "로",
            "와", "과", "하고", "이랑", "랑", "도", "만", "부터", "까지", "에게", "한테",
            "께", "에게서", "한테서", "보다", "처럼", "같이", "대로", "만큼", "밖에",
            // Pronouns (대명사)
            "나", "저", "너", "당신", "그", "그녀", "우리", "저희", "너희", "그들",
            "이것", "그것", "저것", "여기", "거기", "저기", "어디", "누구", "무엇", "뭐",
            // Demonstratives (지시사)
            "이", "그", "저", "이런", "그런", "저런", "이렇게", "그렇게", "저렇게",
            // Conjunctions (접속사)
            "그리고", "그러나", "그래서", "하지만", "그런데", "따라서", "그러므로",
            "또한", "또", "및", "혹은", "또는",
            // Auxiliary/common verbs
            "하다", "되다", "있다", "없다", "이다", "아니다", "같다",
            "하는", "되는", "있는", "없는", "인", "인데",
            // Common nouns that are too generic
            "것", "수", "등", "때", "중", "후", "전", "내", "간", "측",
            "점", "개", "번", "분", "년", "월", "일", "시", "초",
            // Adverbs (부사)
            "매우", "아주", "너무", "정말", "진짜", "참", "꽤", "좀", "조금", "많이",
            "잘", "더", "덜", "가장", "제일", "특히", "바로", "곧", "이미", "아직",
            "항상", "자주", "가끔", "절대", "결코", "전혀", "거의",
            // Question words
            "왜", "어떻게", "얼마나", "언제", "어디서",
            // Numbers
            "하나", "둘", "셋", "넷", "다섯", "여섯", "일곱", "여덟", "아홉", "열",
            "일", "이", "삼", "사", "오", "육", "칠", "팔", "구", "십", "백", "천", "만",
            // Common action words (too generic for tags)
            "기반", "활용", "구현", "방법", "사용", "관련", "대한", "위한", "통한",
            "정리", "분석", "설명", "소개", "개요", "요약", "정의", "비교", "검토",
            "자동", "수동", "직접", "간접",
            // Common endings that slip through
            "합니다", "입니다", "됩니다", "있습니다", "없습니다",
            "하는", "되는", "있는", "없는", "같은", "다른",
            // File/document related
            "파일", "문서", "페이지", "내용", "정보", "데이터",
            // Time-related generic words
            "오늘", "내일", "어제", "지금", "나중", "최근", "현재",
            // Action-related generic words
            "보기", "하기", "되기", "만들기", "사용하기",
            // Dashboard/UI fragments (specific to our vault issue)
            "대시보드입니다", "탐색합니다", "노트를", "노트를",
        ];
        words.iter().cloned().collect()
    };
}

/// Suggestion to merge two similar tags
#[derive(Debug, Clone, Serialize)]
pub struct MergeSuggestion {
    pub keep: String,
    pub merge: String,
    pub similarity: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    #[ignore] // Requires model download
    fn test_matcher_basic() {
        let embedder = TagEmbedder::default_multilingual().unwrap();
        let db = TagDatabase::open(Path::new(":memory:")).unwrap();

        // Add some tags
        db.add_tag("gpu", "GPU hardware, VRAM, graphics card", &embedder)
            .unwrap();
        db.add_tag("cuda", "NVIDIA CUDA programming, GPU computing", &embedder)
            .unwrap();
        db.add_tag("llm", "Large language models, GPT, Claude", &embedder)
            .unwrap();

        let matcher = TagMatcher::new(embedder, db);

        // Test semantic matching
        let suggestions = matcher
            .suggest_tags("GPU memory optimization techniques", 3)
            .unwrap();

        println!("Suggestions: {:?}", suggestions);

        // gpu and cuda should be in top suggestions
        let tag_names: Vec<_> = suggestions.iter().map(|s| s.tag.as_str()).collect();
        assert!(tag_names.contains(&"gpu") || tag_names.contains(&"cuda"));
    }
}

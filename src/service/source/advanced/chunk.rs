//! Token-aware chunking of extracted content units.
//!
//! Chunks never cross extraction unit boundaries. Each chunk includes:
//! - Canonical locator from the first unit
//! - Ordered chunk ordinal within the document
//! - Token count estimate (character-based approximation)
//! - Text fingerprint for content-addressed identity

use crate::error::Result;

use super::extraction::ExtractedUnit;
use super::identity;

/// A searchable content chunk ready for embedding.
#[derive(Debug, Clone)]
pub struct Chunk {
    pub chunk_id: String,
    pub document_key: String,
    pub content_revision: i64,
    pub ordinal: u32,
    pub locator_json: String,
    pub locator_sort_key: String,
    pub text: String,
    pub text_fingerprint: String,
    pub token_count: u32,
}

/// Chunking configuration.
#[derive(Debug, Clone)]
pub struct ChunkConfig {
    /// Target token count per chunk (character-based approximation: 1 token ≈ 4 chars).
    pub target_tokens: u32,
    /// Overlap between consecutive chunks (in tokens).
    pub overlap_tokens: u32,
    /// Policy version for fingerprinting.
    pub policy_version: String,
}

impl Default for ChunkConfig {
    fn default() -> Self {
        Self { target_tokens: 384, overlap_tokens: 48, policy_version: "v1".to_string() }
    }
}

impl ChunkConfig {
    /// Approximate token count from character length.
    /// Conservative: assumes ~4 chars per token for mixed CJK/Latin text.
    fn estimate_tokens(&self, text: &str) -> u32 {
        (text.chars().count() as u32 / 4).max(1)
    }

    /// Target chars per chunk based on token target.
    fn target_chars(&self) -> usize {
        (self.target_tokens as usize * 4).max(64)
    }

    /// Overlap chars based on token overlap.
    fn overlap_chars(&self) -> usize {
        (self.overlap_tokens as usize * 4).max(16)
    }
}

/// Chunk extracted units into searchable fragments.
///
/// Chunks never cross unit boundaries. Each chunk contains one or more
/// complete units. Units larger than the target size are kept as single chunks.
pub fn chunk_document(
    units: &[ExtractedUnit],
    document_key: &str,
    content_revision: i64,
    config: &ChunkConfig,
) -> Result<Vec<Chunk>> {
    if units.is_empty() {
        return Ok(Vec::new());
    }

    let target = config.target_chars();
    let overlap = config.overlap_chars();
    let mut chunks = Vec::new();
    let mut current_text = String::new();
    let mut current_start_ordinal = units[0].ordinal;
    let mut current_unit_count = 0u32;

    for (i, unit) in units.iter().enumerate() {
        let unit_len = unit.text.chars().count();

        // If adding this unit would exceed the target and we already have content,
        // flush the current chunk.
        if !current_text.is_empty() && current_text.chars().count() + unit_len > target {
            chunks.push(build_chunk(
                document_key,
                content_revision,
                chunks.len() as u32,
                &current_text,
                &units[current_start_ordinal as usize..i],
                config,
            ));
            current_unit_count = 0;

            // Start new chunk with overlap from the previous one
            if !current_text.is_empty() {
                let overlap_start = current_text.chars().count().saturating_sub(overlap);
                let overlap_text: String = current_text.chars().skip(overlap_start).collect();
                current_text = overlap_text;
                current_start_ordinal = units[i.saturating_sub(current_unit_count as usize)].ordinal;
            } else {
                current_text.clear();
                current_start_ordinal = unit.ordinal;
            }
        }

        if current_unit_count > 0 {
            current_text.push('\n');
        }
        current_text.push_str(&unit.text);
        current_unit_count += 1;
    }

    // Flush the final chunk
    if !current_text.is_empty() {
        let start_idx = current_start_ordinal as usize;
        let end_idx = units.len();
        chunks.push(build_chunk(
            document_key,
            content_revision,
            chunks.len() as u32,
            &current_text,
            &units[start_idx..end_idx],
            config,
        ));
    }

    Ok(chunks)
}

fn build_chunk(
    document_key: &str,
    content_revision: i64,
    ordinal: u32,
    text: &str,
    units: &[ExtractedUnit],
    config: &ChunkConfig,
) -> Chunk {
    let text_fp = identity::raw_fingerprint(text.as_bytes());
    let locator_json = units.first().map(|u| u.locator_json.clone()).unwrap_or_default();
    let locator_sort_key = units.first().map(|u| u.locator_sort_key.clone()).unwrap_or_default();
    let token_count = config.estimate_tokens(text);

    let chunk_id = identity::chunk_id(document_key, content_revision, &locator_json, &config.policy_version, &text_fp);

    Chunk {
        chunk_id,
        document_key: document_key.to_string(),
        content_revision,
        ordinal,
        locator_json,
        locator_sort_key,
        text: text.to_string(),
        text_fingerprint: text_fp,
        token_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::source::advanced::extraction::ExtractedUnit;

    fn make_units(texts: &[&str]) -> Vec<ExtractedUnit> {
        texts
            .iter()
            .enumerate()
            .map(|(i, t)| ExtractedUnit {
                ordinal: i as u32,
                text: t.to_string(),
                locator_json: format!(r#"{{"kind":"text","start_line":{}}}"#, i + 1),
                locator_sort_key: format!("{:010}", i),
            })
            .collect()
    }

    #[test]
    fn empty_units_produces_no_chunks() {
        let chunks = chunk_document(&[], "dk", 1, &ChunkConfig::default()).unwrap();
        assert!(chunks.is_empty());
    }

    #[test]
    fn single_short_unit_is_one_chunk() {
        let units = make_units(&["Hello world"]);
        let chunks = chunk_document(&units, "dk", 1, &ChunkConfig::default()).unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].ordinal, 0);
        assert!(chunks[0].text.contains("Hello world"));
    }

    #[test]
    fn multiple_small_units_combine() {
        let units = make_units(&["First sentence.", "Second sentence.", "Third sentence.", "Fourth sentence."]);
        let chunks = chunk_document(&units, "dk", 1, &ChunkConfig::default()).unwrap();
        // All four short sentences should fit in one chunk
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].text.contains("First"));
        assert!(chunks[0].text.contains("Fourth"));
    }

    #[test]
    fn chunk_ids_are_deterministic() {
        let units = make_units(&["Hello world"]);
        let c1 = chunk_document(&units, "dk", 1, &ChunkConfig::default()).unwrap();
        let c2 = chunk_document(&units, "dk", 1, &ChunkConfig::default()).unwrap();
        assert_eq!(c1[0].chunk_id, c2[0].chunk_id);
    }

    #[test]
    fn chunk_ids_differ_by_content() {
        let u1 = make_units(&["Hello"]);
        let u2 = make_units(&["World"]);
        let c1 = chunk_document(&u1, "dk", 1, &ChunkConfig::default()).unwrap();
        let c2 = chunk_document(&u2, "dk", 1, &ChunkConfig::default()).unwrap();
        assert_ne!(c1[0].chunk_id, c2[0].chunk_id);
    }
}

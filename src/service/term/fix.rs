use std::path::Path;

use super::{parse_correction_attr, sort_terms_by_name, TermUpdate};
use crate::error::{MfError, Result};
use crate::model::term::{Correction, FixKind, MatchKind, Term};
use crate::service::index;

/// Modify an existing term's definition, aliases, tags, description, confidence,
/// or corrections.
pub fn fix_term(project_root: &Path, term_name: &str, update: TermUpdate<'_>) -> Result<Term> {
    if !update.has_legacy_flags()
        && !update.has_metadata_flags()
        && update.delete_aliases.is_empty()
        && update.delete_tags.is_empty()
        && update.add_corrections.is_empty()
        && update.delete_corrections.is_empty()
        && update.correction_matches.is_empty()
        && update.correction_fixes.is_empty()
        && update.correction_pinyins.is_empty()
    {
        return Err(MfError::usage(
            "at least one of --definition, --description, --confidence, --alias, --tag, --delete-alias, --delete-tag, --clear-description, --clear-confidence, --add-correction, --delete-correction, --correction-match, --correction-fix, --correction-pinyin must be provided",
            None,
        ));
    }

    super::validate_confidence(update.confidence)?;

    let mut index = index::load(project_root)?;

    let term_clone = {
        let terms = index.terms.as_mut().ok_or_else(|| {
            MfError::not_found(
                format!("term '{term_name}' not found"),
                Some("use `mf term list` or `mf term new`".to_string()),
            )
        })?;

        let pos = terms.iter().position(|t| t.term == term_name).ok_or_else(|| {
            MfError::not_found(
                format!("term '{term_name}' not found"),
                Some("use `mf term list` or `mf term new`".to_string()),
            )
        })?;

        if terms.iter().filter(|t| t.term == term_name).count() > 1 {
            return Err(MfError::IncompatibleSchema {
                path: project_root.join("mind-index.yaml"),
                found: format!("duplicate term '{term_name}'"),
                expected: vec!["unique terms".to_string()],
            });
        }

        let t = &mut terms[pos];
        apply_update(t, &update)?;
        t.clone()
    };

    if let Some(ref mut terms) = index.terms {
        sort_terms_by_name(terms);
    }
    index::save(project_root, &index)?;

    Ok(term_clone)
}

/// Apply a *value* correction attribute (e.g. `--correction-match`).
/// Returns an error if the original doesn't identify an existing correction.
fn update_correction_match(t: &mut Term, original: &str, value: &str) -> Result<()> {
    let kind: MatchKind = match value.to_lowercase().as_str() {
        "word" => MatchKind::Word,
        "substring" => MatchKind::Substring,
        "pinyin" => MatchKind::Pinyin,
        other => return Err(MfError::usage(format!("invalid match kind '{other}'"), None)),
    };
    let pos = super::find_correction_index(t, original)?;
    t.corrections[pos].r#match = kind;
    Ok(())
}

fn update_correction_fix(t: &mut Term, original: &str, value: &str) -> Result<()> {
    let kind: FixKind = match value.to_lowercase().as_str() {
        "required" => FixKind::Required,
        "suggested" => FixKind::Suggested,
        other => return Err(MfError::usage(format!("invalid fix kind '{other}'"), None)),
    };
    let pos = super::find_correction_index(t, original)?;
    t.corrections[pos].fix = kind;
    Ok(())
}

fn update_correction_pinyin(t: &mut Term, original: &str, value: &str) -> Result<()> {
    let pos = super::find_correction_index(t, original)?;
    t.corrections[pos].pinyin = if value.is_empty() { None } else { Some(value.to_string()) };
    Ok(())
}

fn delete_correction_by_original(t: &mut Term, original: &str) -> Result<()> {
    let pos = super::find_correction_index(t, original)?;
    t.corrections.remove(pos);
    Ok(())
}

fn add_correction_to_term(t: &mut Term, original: &str) -> Result<(Correction, bool)> {
    // Idempotent: if correction with this original already exists, return it.
    if let Some(existing) = t.corrections.iter().find(|c| c.original == original) {
        return Ok((existing.clone(), false));
    }
    let corr = Correction {
        original: original.to_string(),
        correct: String::new(), // placeholder; set later via correction update
        r#match: MatchKind::Word,
        fix: FixKind::Required,
        boundary: Default::default(),
        pinyin: None,
    };
    t.corrections.push(corr.clone());
    Ok((corr, true))
}

/// Apply a `TermUpdate` to a term entry in place.
pub(crate) fn apply_update(t: &mut Term, update: &TermUpdate<'_>) -> Result<()> {
    if let Some(def) = update.definition {
        t.definition = if def.is_empty() { None } else { Some(def.to_string()) };
    }
    if update.clear_description {
        t.description = None;
    } else if let Some(d) = update.description {
        t.description = if d.is_empty() { None } else { Some(d.to_string()) };
    }
    if update.clear_confidence {
        t.confidence = None;
    } else if let Some(c) = update.confidence {
        t.confidence = Some(c);
    }
    for alias in update.aliases {
        if !t.aliases.contains(alias) {
            t.aliases.push(alias.clone());
        }
    }
    for alias in update.delete_aliases {
        t.aliases.retain(|a| a != alias);
    }
    for tag in update.tags {
        if !t.tags.contains(tag) {
            t.tags.push(tag.clone());
        }
    }
    for tag in update.delete_tags {
        t.tags.retain(|t| t != tag);
    }

    // Correction mutations
    for original in update.add_corrections {
        add_correction_to_term(t, original)?;
    }
    for original in update.delete_corrections {
        delete_correction_by_original(t, original)?;
    }
    for raw in update.correction_matches {
        let attr = parse_correction_attr(raw, "correction-match")?;
        update_correction_match(t, &attr.original, &attr.value)?;
    }
    for raw in update.correction_fixes {
        let attr = parse_correction_attr(raw, "correction-fix")?;
        update_correction_fix(t, &attr.original, &attr.value)?;
    }
    for raw in update.correction_pinyins {
        let attr = parse_correction_attr(raw, "correction-pinyin")?;
        update_correction_pinyin(t, &attr.original, &attr.value)?;
    }
    Ok(())
}

use std::path::Path;

use super::{parse_correction_attr, sort_terms_by_name, TermUpdate};
use crate::error::{MfError, Result};
use crate::model::term::{validate_corrections, Boundary, Correction, FixKind, MatchKind, Term};
use crate::service::index;

/// Modify an existing term's definition, aliases, tags, description, confidence,
/// or corrections.
pub fn fix_term(project_root: &Path, term_name: &str, update: TermUpdate<'_>) -> Result<Term> {
    update.ensure_non_empty()?;

    super::validate_confidence(update.confidence)?;

    // Lenient load: a term whose corrections are already invalid must remain
    // repairable via `--delete-correction` / `--correction-match` (the reported
    // CLI self-repair deadlock).
    let mut index = index::load_lenient(project_root)?;

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
        validate_corrections(terms).map_err(|msg| MfError::usage(msg, None::<String>))?;
    }
    index::save(project_root, &index)?;

    Ok(term_clone)
}

/// Apply a *value* correction attribute (e.g. `--correction-match`).
/// Returns an error if the original doesn't identify an existing correction.
fn update_correction_match(t: &mut Term, original: &str, value: &str) -> Result<()> {
    let kind: MatchKind = match value.to_lowercase().as_str() {
        "word" => MatchKind::Word,
        "pinyin" => MatchKind::Pinyin,
        "substring" => MatchKind::Substring,
        other => return Err(MfError::usage(format!("invalid match kind '{other}'"), None)),
    };
    let pos = super::find_correction_index(t, original)?;
    t.corrections[pos].r#match = kind;
    if kind == MatchKind::Pinyin && t.corrections[pos].boundary == Boundary::Standalone {
        t.corrections[pos].boundary = Boundary::Loose;
    }
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

/// Add a correction from a raw `--add-correction` value of the form
/// `ORIGINAL[:CORRECT]`. When `:CORRECT` is omitted the correct text falls back
/// to the term's canonical name so the correction is never left with an empty,
/// no-op `correct` field.
fn add_correction_to_term(t: &mut Term, raw: &str) {
    let (original, correct) = match raw.split_once(':') {
        Some((o, c)) if !c.is_empty() => (o, c.to_string()),
        _ => (raw, t.term.clone()),
    };
    // Idempotent: skip when a correction with this original already exists.
    if t.corrections.iter().any(|c| c.original == original) {
        return;
    }
    t.corrections.push(Correction {
        original: original.to_string(),
        correct,
        r#match: MatchKind::Word,
        fix: FixKind::Required,
        boundary: Default::default(),
        pinyin: None,
    });
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
        add_correction_to_term(t, original);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::term::{Boundary, Correction, FixKind, MatchKind};

    fn synthetic_term() -> Term {
        Term {
            term: "Synthetic".into(),
            definition: Some("unchanged".into()),
            description: None,
            confidence: None,
            aliases: vec![],
            tags: vec![],
            corrections: vec![
                Correction {
                    original: "first".into(),
                    correct: "Synthetic".into(),
                    r#match: MatchKind::Word,
                    fix: FixKind::Required,
                    boundary: Boundary::Standalone,
                    pinyin: None,
                },
                Correction {
                    original: "second".into(),
                    correct: "Synthetic".into(),
                    r#match: MatchKind::Word,
                    fix: FixKind::Required,
                    boundary: Boundary::Standalone,
                    pinyin: None,
                },
            ],
        }
    }

    #[test]
    fn update_sets_exact_correction_match_without_touching_siblings() {
        let mut term = synthetic_term();
        let values = vec!["first:pinyin".to_string()];
        apply_update(&mut term, &TermUpdate { correction_matches: &values, ..Default::default() }).unwrap();
        assert_eq!(term.corrections[0].r#match, MatchKind::Pinyin);
        assert_eq!(term.corrections[1].r#match, MatchKind::Word);
        assert_eq!(term.definition.as_deref(), Some("unchanged"));
    }

    #[test]
    fn update_pinyin_and_delete_target_exact_original_only() {
        let mut term = synthetic_term();
        let pinyins = vec!["first:synthetic-reading".to_string()];
        let deletes = vec!["second".to_string()];
        apply_update(
            &mut term,
            &TermUpdate { correction_pinyins: &pinyins, delete_corrections: &deletes, ..Default::default() },
        )
        .unwrap();
        assert_eq!(term.corrections.len(), 1);
        assert_eq!(term.corrections[0].original, "first");
        assert_eq!(term.corrections[0].pinyin.as_deref(), Some("synthetic-reading"));
    }

    #[test]
    fn correction_match_to_pinyin_resets_standalone_boundary() {
        let mut term = synthetic_term();
        assert_eq!(term.corrections[0].boundary, Boundary::Standalone);
        let values = vec!["first:pinyin".to_string()];
        apply_update(&mut term, &TermUpdate { correction_matches: &values, ..Default::default() }).unwrap();
        assert_eq!(term.corrections[0].r#match, MatchKind::Pinyin);
        assert_eq!(term.corrections[0].boundary, Boundary::Loose);
        crate::model::term::validate_corrections(std::slice::from_ref(&term)).expect("normalized term must be valid");
    }

    #[test]
    fn add_correction_sets_correct_text_and_defaults_to_term_name() {
        let mut term = synthetic_term();
        let adds = vec!["typo:Synthetic".to_string(), "bare".to_string()];
        apply_update(&mut term, &TermUpdate { add_corrections: &adds, ..Default::default() }).unwrap();
        let explicit = term.corrections.iter().find(|c| c.original == "typo").unwrap();
        assert_eq!(explicit.correct, "Synthetic");
        let bare = term.corrections.iter().find(|c| c.original == "bare").unwrap();
        assert_eq!(bare.correct, "Synthetic", "bare add-correction falls back to the term name, never empty");
    }
}

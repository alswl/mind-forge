//! Effective date derivation for publish operations.
//!
//! Determines the publish date of an article from its origin
//! (filename prefix or template slot).

use chrono::NaiveDate;

use crate::error::{MfError, Result};
use crate::model::article::Article;
use crate::service::util::filename_date;

/// Origin of the effective date.
#[derive(Debug, Clone, PartialEq)]
pub enum EffectiveDateOrigin {
    /// Date parsed from template slot capture (US2).
    TemplateSlot,
    /// Date parsed from the leading YYYY-MM-DD prefix of the filename.
    FilenamePrefix,
}

/// An article's effective date and how it was determined.
#[derive(Debug, Clone)]
pub struct EffectiveDate {
    pub date: NaiveDate,
    pub origin: EffectiveDateOrigin,
}

/// Derive the effective date for an article.
///
/// For US1 (docs/-declared articles): parses the date from the leading
/// YYYY-MM-DD prefix of the article filename.
///
/// For US2 (generated articles): the date is captured from the template
/// slot match (added in T023).
pub fn for_article(article: &Article) -> Result<EffectiveDate> {
    // US2: TemplateOrigin branch added in T023
    if let Some(origin) = &article.template_origin
        && let Ok(date) = NaiveDate::parse_from_str(&origin.slot_value, "%Y-%m-%d")
    {
        return Ok(EffectiveDate { date, origin: EffectiveDateOrigin::TemplateSlot });
    }

    // US1: parse from filename prefix
    let basename = article.article_path.rsplit('/').next().unwrap_or(&article.article_path);
    let date = filename_date::parse_leading_date(basename).ok_or(MfError::NoEffectiveDate)?;
    Ok(EffectiveDate { date, origin: EffectiveDateOrigin::FilenamePrefix })
}

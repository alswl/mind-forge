//! Date extraction from filenames: parse a leading `YYYY-MM-DD` prefix.

use chrono::NaiveDate;

/// Parse the leading `YYYY-MM-DD` date from a filename basename (without extension).
///
/// Strips the file extension, takes the first 10 characters, and attempts
/// `NaiveDate::parse_from_str` with `%Y-%m-%d`. Returns `None` if the
/// prefix is not a valid date.
pub fn parse_leading_date(basename: &str) -> Option<NaiveDate> {
    let stem = basename.split('.').next().unwrap_or(basename);
    let prefix = stem.get(..10)?;
    NaiveDate::parse_from_str(prefix, "%Y-%m-%d").ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leading_date_extracted() {
        let date = parse_leading_date("2026-05-15-launch.md");
        assert_eq!(date, NaiveDate::from_ymd_opt(2026, 5, 15));
    }

    #[test]
    fn no_date_returns_none() {
        assert!(parse_leading_date("launch.md").is_none());
    }

    #[test]
    fn invalid_date_returns_none() {
        assert!(parse_leading_date("2026-13-40-bad.md").is_none());
    }

    #[test]
    fn short_string_returns_none() {
        assert!(parse_leading_date("abc.md").is_none());
    }
}

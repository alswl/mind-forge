use std::path::Path;

use chrono::Utc;

use super::validate_url;
use crate::error::{MfError, Result};
use crate::model::source::Source;
use crate::service::index;

/// Parameters for `update()`.
pub struct UpdateArgs<'a> {
    pub name: &'a str,
    pub rename: Option<&'a str>,
    pub url: Option<&'a str>,
}

/// Update a source by name. At least one of rename or url must be provided.
pub fn update(project_path: &Path, args: &UpdateArgs) -> Result<Source> {
    if args.rename.is_none() && args.url.is_none() {
        return Err(MfError::usage(
            "nothing to update: use --rename or --url",
            Some("pass --rename <NAME> or --url <URL> to modify the source".to_string()),
        ));
    }

    let mut index = index::load(project_path)?;
    let sources = index.sources.as_mut().ok_or_else(|| {
        MfError::usage(
            format!("source '{}' not found", args.name),
            Some("use `mf source list` to see available sources".to_string()),
        )
    })?;

    let idx = sources.iter().position(|s| s.name == args.name).ok_or_else(|| {
        MfError::usage(
            format!("source '{}' not found", args.name),
            Some("use `mf source list` to see available sources".to_string()),
        )
    })?;

    if let Some(new_name) = args.rename {
        if new_name != args.name && sources.iter().any(|s| s.name == new_name) {
            return Err(MfError::usage(
                format!("a source named '{new_name}' already exists"),
                Some("use a different --rename value".to_string()),
            ));
        }
    }

    if let Some(u) = args.url {
        validate_url(u)?;
    }

    let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let entry = &mut sources[idx];

    if let Some(new_name) = args.rename {
        entry.name = new_name.to_string();
    }
    if let Some(new_url) = args.url {
        entry.url = Some(new_url.to_string());
    }
    entry.updated_at = now.clone();

    let updated = entry.clone();

    sources.sort_by(|a, b| a.name.cmp(&b.name));

    index::save(project_path, &index)?;
    Ok(updated)
}

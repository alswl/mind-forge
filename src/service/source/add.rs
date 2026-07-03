use std::path::Path;
use std::path::PathBuf;

use chrono::Utc;

use super::{derive_name_from_path, infer_kind_from_path, validate_url};
use crate::error::{MfError, Result};
use crate::model::source::{FileKind, Source, SourceKind};
use crate::service::config as config_svc;
use crate::service::index;
use crate::service::util;
use crate::service::util::create_symlink;

/// The mode used when adding a source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddMode {
    Copy,
    Link,
    Url,
}

/// Outcome returned by the `add()` function.
pub struct AddOutcome {
    pub source: Source,
    pub mode: AddMode,
    pub replaced: bool,
}

/// Parameters for `add()`.
pub struct AddArgs<'a> {
    pub input: &'a str,
    pub name: Option<&'a str>,
    pub kind: Option<FileKind>,
    pub source_kind: Option<SourceKind>,
    pub link: bool,
    pub force: bool,
}

/// Internal enum for classifying `mf source add` input.
pub(crate) enum InputForm {
    Path,
    Url,
}

pub(crate) fn classify_input(input: &str) -> InputForm {
    if input.starts_with("http://") || input.starts_with("https://") {
        InputForm::Url
    } else {
        InputForm::Path
    }
}

enum UpsertSlot<'a> {
    New,
    Replace { idx: usize, prior: &'a Source },
}

/// Look up a source by name and determine whether we should create or replace.
fn locate_slot<'a>(sources: &'a [Source], name: &str, force: bool, conflict_marker: PathBuf) -> Result<UpsertSlot<'a>> {
    match sources.iter().position(|s| s.name == name) {
        None => Ok(UpsertSlot::New),
        Some(idx) if force => Ok(UpsertSlot::Replace { idx, prior: &sources[idx] }),
        Some(_) => Err(MfError::file_exists(conflict_marker)),
    }
}

fn replace_in_sources(sources: &mut Vec<Source>, idx: usize, source: Source) {
    sources.remove(idx);
    sources.push(source);
    sources.sort_by(|a, b| a.name.cmp(&b.name));
}

/// Add a source — dispatches to Path or URL branch based on input.
pub fn add(project_path: &Path, cwd: &Path, args: &AddArgs) -> Result<AddOutcome> {
    let form = classify_input(args.input);
    match form {
        InputForm::Url => add_url(project_path, args),
        InputForm::Path => add_path(project_path, cwd, args),
    }
}

fn add_url(project_path: &Path, args: &AddArgs) -> Result<AddOutcome> {
    validate_url(args.input)?;

    let name = args.name.map(|s| s.to_string()).ok_or_else(|| {
        MfError::usage("URL sources require an explicit --name", Some("pass --name <STRING>".to_string()))
    })?;

    let model_kind = match args.kind.clone() {
        Some(k) => match k {
            FileKind::Auto => FileKind::Web,
            FileKind::Rss => FileKind::Rss,
            FileKind::Web => FileKind::Web,
            FileKind::Pdf => {
                return Err(MfError::usage(
                    "cannot use --type pdf with a URL input",
                    Some("download the file first, then add the local path".to_string()),
                ))
            }
            FileKind::File => {
                return Err(MfError::usage(
                    "cannot use --type file with a URL input",
                    Some("download the file first, then add the local path".to_string()),
                ))
            }
        },
        None => FileKind::Web,
    };

    let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    let mut index = index::load(project_path)?;
    let sources = index.sources.get_or_insert_with(Vec::new);
    let slot = locate_slot(sources, &name, args.force, project_path.join("mind-index.yaml"))?;

    let (mode, source, replaced) = match slot {
        UpsertSlot::Replace { idx, prior } => {
            let source = Source {
                name: name.clone(),
                kind: model_kind,
                source_kind: args.source_kind.clone(),
                url: Some(args.input.to_string()),
                path: None,
                tags: prior.tags.clone(),
                added_at: prior.added_at.clone(),
                updated_at: now,
            };
            replace_in_sources(sources, idx, source.clone());
            (AddMode::Url, source, true)
        }
        UpsertSlot::New => {
            let source = Source {
                name: name.clone(),
                kind: model_kind,
                source_kind: args.source_kind.clone(),
                url: Some(args.input.to_string()),
                path: None,
                tags: vec![],
                added_at: now.clone(),
                updated_at: now,
            };
            sources.push(source.clone());
            sources.sort_by(|a, b| a.name.cmp(&b.name));
            (AddMode::Url, source, false)
        }
    };

    index::save(project_path, &index)?;
    Ok(AddOutcome { source, mode, replaced })
}

fn add_path(project_path: &Path, cwd: &Path, args: &AddArgs) -> Result<AddOutcome> {
    let source_path = cwd.join(args.input);
    let source_canonical = source_path.canonicalize().map_err(MfError::Io)?;

    let metadata = std::fs::metadata(&source_canonical).map_err(MfError::Io)?;
    if !metadata.is_file() {
        return Err(MfError::usage(
            format!("source path '{}' must be an existing regular file", args.input),
            None as Option<String>,
        ));
    }

    let layout = config_svc::effective_layout(project_path)?;
    let sources_dir = project_path.join(&layout.sources);
    if util::canonicalize_within(&sources_dir, &source_canonical).is_ok() {
        return Err(MfError::usage(
            format!("source file is already inside the project's {}/ directory", layout.sources),
            Some("use `mf source update <NAME>` to modify metadata".to_string()),
        ));
    }

    let model_kind = match args.kind.clone() {
        Some(k) => match k {
            FileKind::Auto => infer_kind_from_path(&source_canonical),
            FileKind::Pdf | FileKind::File => k,
            FileKind::Rss | FileKind::Web => {
                return Err(MfError::usage(
                    "cannot use --type rss or --type web with a local file input",
                    Some("pass an http(s):// URL".to_string()),
                ))
            }
        },
        None => infer_kind_from_path(&source_canonical),
    };

    let name = match args.name {
        Some(n) => n.to_string(),
        None => derive_name_from_path(&source_path)?,
    };

    let basename = source_canonical.file_name().map(|s| s.to_string_lossy().to_string()).ok_or_else(|| {
        MfError::usage(format!("cannot extract filename from '{}'", source_canonical.display()), None as Option<String>)
    })?;
    let kind_dir_name = model_kind.as_str();
    let kind_dir = sources_dir.join(kind_dir_name);
    let dest = kind_dir.join(&basename);

    std::fs::create_dir_all(&kind_dir).map_err(MfError::Io)?;

    let mut index = index::load(project_path)?;
    let sources = index.sources.get_or_insert_with(Vec::new);
    let slot = locate_slot(sources, &name, args.force, dest.clone())?;

    let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    let write_file = || -> Result<()> {
        if dest.exists() {
            std::fs::remove_file(&dest).map_err(MfError::Io)?;
        }
        if args.link {
            create_symlink(&source_canonical, &dest)
        } else {
            std::fs::copy(&source_canonical, &dest).map_err(MfError::Io).map(|_| ())
        }
    };

    let (mode, source, replaced) = match slot {
        UpsertSlot::Replace { idx, prior } => {
            let rel_path = util::rel_posix_path(project_path, &dest)?;
            let old_path = prior.path.clone();

            write_file()?;

            if let Some(ref old) = old_path {
                if *old != rel_path {
                    let _ = std::fs::remove_file(project_path.join(old));
                }
            }

            let source = Source {
                name: name.clone(),
                kind: model_kind,
                source_kind: args.source_kind.clone(),
                url: None,
                path: Some(rel_path),
                tags: prior.tags.clone(),
                added_at: prior.added_at.clone(),
                updated_at: now,
            };
            let mode = if args.link { AddMode::Link } else { AddMode::Copy };
            replace_in_sources(sources, idx, source.clone());
            (mode, source, true)
        }
        UpsertSlot::New => {
            let rel_path = util::rel_posix_path(project_path, &dest)?;

            match args.link {
                true => create_symlink(&source_canonical, &dest)?,
                false => {
                    std::fs::copy(&source_canonical, &dest).map_err(MfError::Io)?;
                }
            }

            let source = Source {
                name: name.clone(),
                kind: model_kind,
                source_kind: args.source_kind.clone(),
                url: None,
                path: Some(rel_path.clone()),
                tags: vec![],
                added_at: now.clone(),
                updated_at: now,
            };
            let mode = if args.link { AddMode::Link } else { AddMode::Copy };
            sources.push(source.clone());
            sources.sort_by(|a, b| a.name.cmp(&b.name));
            (mode, source, false)
        }
    };

    index::save(project_path, &index)?;

    Ok(AddOutcome { source, mode, replaced })
}

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
    Register,
}

/// Result of indexing a freshly added source into RAG (spec 069 #28).
pub struct IndexingSummary {
    /// Whether the source's content was indexed with chunks/embeddings.
    pub indexed: bool,
    /// Chunks persisted for the source this add (0 when skipped/degraded).
    pub chunks: u64,
    /// Set when indexing was skipped or degraded; carries a retry hint.
    pub warning: Option<String>,
    /// Backend that handled (or declined) indexing: "lance" or "legacy".
    pub backend: &'static str,
}

/// Outcome returned by the `add()` function.
pub struct AddOutcome {
    pub source: Source,
    pub mode: AddMode,
    pub replaced: bool,
    /// Lance-primary registration key (set only when Lance backend is active).
    pub registration_key: Option<String>,
    /// Whether the compatibility projection export degraded (set when Lance active).
    pub projection_degraded: bool,
    /// RAG indexing result (set only when the Lance backend indexes on add).
    pub indexing: Option<IndexingSummary>,
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

pub fn register_only(
    repo_root: &Path,
    project_path: &Path,
    cwd: &Path,
    args: &AddArgs,
    dry_run: bool,
) -> Result<AddOutcome> {
    if args.link {
        return Err(MfError::usage("--register-only cannot be combined with --link", None));
    }
    if args.force {
        return Err(MfError::usage("--register-only cannot be combined with --force", None));
    }
    if matches!(classify_input(args.input), InputForm::Url) {
        return Err(MfError::usage("--register-only requires a local file path", None));
    }

    let layout = config_svc::effective_layout(project_path)?;
    let sources_dir = project_path.join(&layout.sources);
    // Resolve project/sources/repo-relative or absolute inputs to an existing
    // file; a miss is a usage error (exit 2), never an internal Io error (#23).
    let source_path = util::path::resolve_source_input(args.input, project_path, &sources_dir, cwd, repo_root)?;
    let source_canonical = source_path.canonicalize().map_err(MfError::Io)?;
    if !std::fs::metadata(&source_canonical).map_err(MfError::Io)?.is_file() {
        return Err(MfError::usage(format!("source path '{}' must be an existing regular file", args.input), None));
    }
    util::canonicalize_within(&sources_dir, &source_canonical).map_err(|_| {
        MfError::usage(format!("--register-only path must be inside the project's {}/ directory", layout.sources), None)
    })?;

    let model_kind = match args.kind.clone() {
        Some(FileKind::Auto) | None => infer_kind_from_path(&source_canonical),
        Some(kind @ (FileKind::Pdf | FileKind::File)) => kind,
        Some(FileKind::Rss | FileKind::Web) => {
            return Err(MfError::usage("cannot use --file-kind rss or --file-kind web with a local file", None));
        }
    };
    let name = args.name.map(str::to_string).unwrap_or(derive_name_from_path(&source_path)?);
    let canonical_project = project_path.canonicalize().map_err(MfError::Io)?;
    let rel_path = util::rel_posix_path(&canonical_project, &source_canonical)?;
    let mut index = index::load(project_path)?;
    let sources = index.sources.get_or_insert_with(Vec::new);

    if let Some(existing) = sources.iter().find(|source| source.path.as_deref() == Some(rel_path.as_str())) {
        if existing.name != name {
            return Err(MfError::usage(
                format!("source path '{rel_path}' is already registered as '{}'", existing.name),
                Some("use the existing source name or update it explicitly".to_string()),
            ));
        }
        return Ok(AddOutcome {
            source: existing.clone(),
            mode: AddMode::Register,
            replaced: false,
            registration_key: None,
            projection_degraded: false,
            indexing: None,
        });
    }
    if sources.iter().any(|source| source.name == name) {
        return Err(MfError::file_exists(project_path.join("mind-index.yaml")));
    }

    let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let source = Source {
        name,
        kind: model_kind,
        source_kind: args.source_kind.clone(),
        url: None,
        path: Some(rel_path),
        tags: vec![],
        added_at: now.clone(),
        updated_at: now,
    };
    if !dry_run {
        sources.push(source.clone());
        sources.sort_by(|left, right| left.name.cmp(&right.name));
        index::save(project_path, &index)?;
    }
    Ok(AddOutcome {
        source,
        mode: AddMode::Register,
        replaced: false,
        registration_key: None,
        projection_degraded: false,
        indexing: None,
    })
}

/// Internal enum for classifying `mf source add` input.
pub(crate) enum InputForm {
    Path,
    Url,
}

pub(crate) fn classify_input(input: &str) -> InputForm {
    if input.starts_with("http://") || input.starts_with("https://") { InputForm::Url } else { InputForm::Path }
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
pub fn add(repo_root: &Path, project_path: &Path, cwd: &Path, args: &AddArgs) -> Result<AddOutcome> {
    let form = classify_input(args.input);
    match form {
        InputForm::Url => add_url(project_path, args),
        InputForm::Path => add_path(repo_root, project_path, cwd, args),
    }
}

/// Fetch a URL and persist it as a local source file under `sources/<kind>/`.
///
/// Returns the repo-relative local path and the raw bytes. The file is written
/// atomically (temp → rename) so a failure never leaves a half-written file.
pub(crate) fn fetch_url_to_local_file(
    url: &str,
    name: &str,
    kind: &str,
    project_path: &Path,
    max_bytes: u64,
    timeout_seconds: u32,
    max_redirects: u32,
) -> Result<(String, Vec<u8>)> {
    use crate::service::source::advanced::acquisition;

    let content = acquisition::acquire_http(url, max_bytes, timeout_seconds, max_redirects)?;
    let ext = file_extension_for_url(url, kind);
    let kind_dir = project_path.join("sources").join(kind);
    std::fs::create_dir_all(&kind_dir).map_err(MfError::Io)?;
    let dest = kind_dir.join(format!("{name}.{ext}"));
    let tmp = kind_dir.join(format!(".{name}.tmp"));
    // Write to a temp file then rename so a crash never leaves a half-written file.
    std::fs::write(&tmp, &content.raw_bytes).map_err(MfError::Io)?;
    std::fs::rename(&tmp, &dest).map_err(MfError::Io)?;
    let rel_path = util::rel_posix_path(project_path, &dest)?;
    Ok((rel_path, content.raw_bytes))
}

/// Derive a file extension from a URL path. Falls back to `html` for web and
/// `xml` for RSS when the path has no recognised extension.
fn file_extension_for_url<'a>(url: &'a str, kind: &str) -> &'a str {
    // Strip query and fragment before extracting the path suffix.
    let path_only = url.split('?').next().unwrap_or(url).split('#').next().unwrap_or(url);
    if let Some(ext) =
        std::path::Path::new(path_only).extension().and_then(|e| e.to_str()).filter(|e| !e.is_empty() && e.len() <= 10)
    {
        ext
    } else if kind == "rss" {
        "xml"
    } else {
        "html"
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
                ));
            }
            FileKind::File => {
                return Err(MfError::usage(
                    "cannot use --type file with a URL input",
                    Some("download the file first, then add the local path".to_string()),
                ));
            }
        },
        None => FileKind::Web,
    };

    let kind_str = model_kind.as_str();

    // Fetch the URL and persist as a local file (Bug B: all sources become
    // local files — `sync` no longer fetches URLs).
    let (rel_path, _raw_bytes) = fetch_url_to_local_file(
        args.input,
        &name,
        kind_str,
        project_path,
        64 * 1024 * 1024, // 64 MiB default
        30,               // 30 s timeout
        5,                // 5 redirects
    )?;

    let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    let mut index = index::load(project_path)?;
    let sources = index.sources.get_or_insert_with(Vec::new);
    let slot = locate_slot(sources, &name, args.force, project_path.join("mind-index.yaml"))?;

    let (mode, source, replaced) = match slot {
        UpsertSlot::Replace { idx, prior } => {
            // Clean up the previous local file if it differs.
            if let Some(ref old_path) = prior.path
                && *old_path != rel_path
            {
                let _ = std::fs::remove_file(project_path.join(old_path));
            }
            let source = Source {
                name: name.clone(),
                kind: model_kind,
                source_kind: args.source_kind.clone(),
                url: Some(args.input.to_string()),
                path: Some(rel_path),
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
                path: Some(rel_path),
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
    Ok(AddOutcome { source, mode, replaced, registration_key: None, projection_degraded: false, indexing: None })
}

fn add_path(repo_root: &Path, project_path: &Path, cwd: &Path, args: &AddArgs) -> Result<AddOutcome> {
    let layout = config_svc::effective_layout(project_path)?;
    let sources_dir = project_path.join(&layout.sources);
    // Resolve project/sources/repo-relative or absolute inputs to an existing
    // file; a miss is a usage error (exit 2), never an internal Io error (#23).
    let source_path = util::path::resolve_source_input(args.input, project_path, &sources_dir, cwd, repo_root)?;
    let source_canonical = source_path.canonicalize().map_err(MfError::Io)?;

    let metadata = std::fs::metadata(&source_canonical).map_err(MfError::Io)?;
    if !metadata.is_file() {
        return Err(MfError::usage(
            format!("source path '{}' must be an existing regular file", args.input),
            None as Option<String>,
        ));
    }

    if util::canonicalize_within(&sources_dir, &source_canonical).is_ok() {
        return Err(MfError::usage(
            format!("source file is already inside the project's {}/ directory", layout.sources),
            Some("pass --register-only to add the existing file to the source index".to_string()),
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
                ));
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

    // #25: refuse before any write if the computed destination already belongs
    // to a DIFFERENT source identity — otherwise a same-basename add silently
    // overwrites another source's file and misreports `replaced: false`.
    let dest_rel = util::rel_posix_path(project_path, &dest)?;
    if let Some(existing) = sources.iter().find(|s| s.path.as_deref() == Some(dest_rel.as_str()) && s.name != name) {
        return Err(MfError::usage(
            format!("destination '{dest_rel}' already holds source '{}'", existing.name),
            Some("choose a unique --name (or remove/rename the existing source first)".to_string()),
        ));
    }

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

            if let Some(ref old) = old_path
                && *old != rel_path
            {
                let _ = std::fs::remove_file(project_path.join(old));
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

            write_file()?;

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

    Ok(AddOutcome { source, mode, replaced, registration_key: None, projection_degraded: false, indexing: None })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_only_indexes_in_tree_file_without_touching_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        let source_dir = project.join("sources/file");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::write(project.join("mind.yaml"), "schema_version: '1'\n").unwrap();
        let file = source_dir.join("synthetic.md");
        let original = b"synthetic source bytes\n";
        std::fs::write(&file, original).unwrap();

        let input = file.to_string_lossy().to_string();
        let outcome = register_only(
            dir.path(),
            &project,
            dir.path(),
            &AddArgs { input: &input, name: None, kind: None, source_kind: None, link: false, force: false },
            false,
        )
        .unwrap();

        assert_eq!(outcome.mode, AddMode::Register);
        assert_eq!(std::fs::read(&file).unwrap(), original);
        let index = index::load(&project).unwrap();
        let sources = index.sources.unwrap();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].path.as_deref(), Some("sources/file/synthetic.md"));

        let repeated = register_only(
            dir.path(),
            &project,
            dir.path(),
            &AddArgs { input: &input, name: None, kind: None, source_kind: None, link: false, force: false },
            false,
        )
        .unwrap();
        assert!(!repeated.replaced);
        assert_eq!(index::load(&project).unwrap().sources.unwrap().len(), 1);
    }
}

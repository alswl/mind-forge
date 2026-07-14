use crate::model::asset::AssetKind;

pub use self::add::{AddArgs, add};
pub use self::clean::clean;
pub use self::index::reconcile;
pub use self::list::list;
pub use self::remove::remove_asset;
pub use self::rename::rename_asset;
pub use self::update::{set_publish_url, update_all, update_one};

// ── Extension-based asset kind inference ─────────────────────────────────────

const EXTENSION_MAP: &[(&str, AssetKind)] = &[
    ("png", AssetKind::Image),
    ("jpg", AssetKind::Image),
    ("jpeg", AssetKind::Image),
    ("gif", AssetKind::Image),
    ("webp", AssetKind::Image),
    ("svg", AssetKind::Image),
    ("bmp", AssetKind::Image),
    ("mp4", AssetKind::Video),
    ("mov", AssetKind::Video),
    ("webm", AssetKind::Video),
    ("mkv", AssetKind::Video),
    ("avi", AssetKind::Video),
    ("mp3", AssetKind::Audio),
    ("wav", AssetKind::Audio),
    ("flac", AssetKind::Audio),
    ("ogg", AssetKind::Audio),
    ("m4a", AssetKind::Audio),
];

pub(crate) fn infer_kind(extension: Option<&std::ffi::OsStr>) -> AssetKind {
    let ext = extension.and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase());
    match ext.as_deref() {
        Some(e) => EXTENSION_MAP.iter().find(|(k, _)| *k == e).map(|(_, kind)| *kind).unwrap_or(AssetKind::Other),
        None => AssetKind::Other,
    }
}

pub mod add;
pub mod clean;
pub mod index;
pub mod list;
pub mod remove;
pub mod rename;
pub mod update;

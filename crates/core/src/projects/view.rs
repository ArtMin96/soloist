//! The project display read-model: a projection of the durable [`ProjectRecord`].

use std::path::PathBuf;

use base64::Engine as _;
use serde::{Deserialize, Serialize};

use crate::ids::ProjectId;
use crate::ports::ProjectRecord;

/// The largest project icon the read-model will load — generous for a small avatar, but a
/// ceiling on what a stray `icon:` path can pull into memory.
const MAX_ICON_BYTES: u64 = 512 * 1024;

/// A project's display identity for the UI read model — a projection of the durable
/// [`ProjectRecord`]. Both fields are resolved here, the same way, so a consumer renders a
/// project the same way it renders any value: [`name`](Self::name) is the human label (the
/// `solo.yml` `name:` when set, else the folder name) and [`icon`](Self::icon) is the
/// `solo.yml` `icon:` loaded into a ready-to-render `data:` URL (`None` when absent,
/// unreadable, oversized, or not an image). The icon is not a separate lookup — it is a
/// field of the project, just like the name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectView {
    pub id: ProjectId,
    pub name: String,
    pub root: PathBuf,
    pub icon: Option<String>,
}

impl ProjectView {
    /// Projects a durable record into its display identity, resolving the name and the icon
    /// the same way — each from the record, into a value the UI renders directly.
    pub fn from_record(record: &ProjectRecord) -> Self {
        Self {
            id: record.id,
            name: display_name(record),
            root: record.root.clone(),
            icon: render_icon(record),
        }
    }
}

/// A project's display name: its `solo.yml` `name:` if set and non-blank, else the final
/// component of its (canonical, absolute) root path — falling back to the whole path only
/// for a root with no final component.
fn display_name(record: &ProjectRecord) -> String {
    record
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            record
                .root
                .file_name()
                .unwrap_or(record.root.as_os_str())
                .to_string_lossy()
                .into_owned()
        })
}

/// A project's display icon: its `solo.yml` `icon:` (resolved against the root) loaded into
/// a `data:` URL the UI renders directly, or `None` when there is none, it cannot be read,
/// it is too large, or it is not a known image type. Resolving it here — beside the name —
/// keeps the icon a plain field of the project rather than a separate request.
fn render_icon(record: &ProjectRecord) -> Option<String> {
    let path = record.root.join(record.icon.as_ref()?);
    // A relative `icon:` resolves against the root; an absolute one is taken as-is (the
    // behaviour of `PathBuf::join`).
    let mime = match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "bmp" => "image/bmp",
        _ => return None,
    };
    let meta = std::fs::metadata(&path).ok()?;
    if !meta.is_file() || meta.len() > MAX_ICON_BYTES {
        return None;
    }
    let bytes = std::fs::read(&path).ok()?;
    Some(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_name_prefers_config_name_then_falls_back_to_the_folder() {
        // A blank or absent name falls back to the root's final component; a real name wins.
        let blank = ProjectRecord {
            id: ProjectId::from_raw(1),
            root: PathBuf::from("/projects/storefront"),
            name: Some("   ".to_string()),
            icon: None,
        };
        assert_eq!(ProjectView::from_record(&blank).name, "storefront");

        let named = ProjectRecord {
            name: Some("Storefront".to_string()),
            ..blank.clone()
        };
        assert_eq!(ProjectView::from_record(&named).name, "Storefront");

        let absent = ProjectRecord {
            name: None,
            ..blank
        };
        assert_eq!(ProjectView::from_record(&absent).name, "storefront");
    }

    #[test]
    fn view_renders_a_known_image_icon_as_a_data_url() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(dir.path().join("icon.png"), b"\x89PNG fake bytes").expect("write icon");
        let record = ProjectRecord {
            id: ProjectId::from_raw(1),
            root: dir.path().to_path_buf(),
            name: None,
            icon: Some(PathBuf::from("icon.png")),
        };
        let icon = ProjectView::from_record(&record).icon.expect("icon");
        assert!(icon.starts_with("data:image/png;base64,"));
    }

    #[test]
    fn view_has_no_icon_when_absent_missing_or_not_an_image() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(dir.path().join("notes.txt"), b"x").expect("write notes");
        let base = ProjectRecord {
            id: ProjectId::from_raw(1),
            root: dir.path().to_path_buf(),
            name: None,
            icon: None,
        };
        // No icon configured.
        assert_eq!(ProjectView::from_record(&base).icon, None);
        // A non-image file is refused by the extension allow-list.
        let not_image = ProjectRecord {
            icon: Some(PathBuf::from("notes.txt")),
            ..base.clone()
        };
        assert_eq!(ProjectView::from_record(&not_image).icon, None);
        // A configured-but-missing file resolves to nothing rather than erroring.
        let missing = ProjectRecord {
            icon: Some(PathBuf::from("missing.png")),
            ..base
        };
        assert_eq!(ProjectView::from_record(&missing).icon, None);
    }

    #[test]
    fn view_has_no_icon_when_oversized() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("big.png"),
            vec![0u8; (MAX_ICON_BYTES + 1) as usize],
        )
        .expect("write big icon");
        let record = ProjectRecord {
            id: ProjectId::from_raw(1),
            root: dir.path().to_path_buf(),
            name: None,
            icon: Some(PathBuf::from("big.png")),
        };
        assert_eq!(ProjectView::from_record(&record).icon, None);
    }
}

//! Scene packs: inspect a manifest and show its permissions before any install.
//! Installation never happens without an explicit confirmation flag.

use std::io::Read;
use std::path::Path;
use std::process::Command;

use crate::config::Config;
use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PackError {
    #[error("cannot read pack manifest: {0}")]
    Read(String),
    #[error("cannot parse pack manifest: {0}")]
    Parse(String),
    #[error("git clone failed: {0}")]
    Git(String),
    #[error("install requires --confirm after reviewing the permission summary")]
    NeedsConfirm,
    #[error("copy failed: {0}")]
    Copy(String),
    #[error("invalid scenes_file name: {0}")]
    InvalidScenesFile(String),
    #[error("file too large: {0}")]
    TooLarge(String),
    #[error("revision mismatch: reviewed {expected}, installing {actual}")]
    RevisionMismatch { expected: String, actual: String },
    #[error("cannot read git revision: {0}")]
    Revision(String),
    #[error("unsafe pack file: {0}")]
    UnsafePackFile(String),
    #[error("digest mismatch: reviewed {expected}, installing {actual}")]
    DigestMismatch { expected: String, actual: String },
}

#[derive(Debug, Clone)]
pub struct PackReview {
    pub revision: String,
    pub digest: String,
    pub permissions: Vec<String>,
}

fn actual_permission(action: &crate::config::ActionSpec) -> String {
    match action.kind.as_str() {
        "omarchy" => {
            let argv = crate::actions::split_command(&action.run).unwrap_or_default();
            match (
                argv.first().map(String::as_str),
                argv.get(1).map(String::as_str),
            ) {
                (Some("theme"), Some("set" | "bg" | "refresh")) => "omarchy theme change".into(),
                (Some("bar"), Some("background")) => "omarchy bar background".into(),
                (Some("toggle"), Some("bar")) => "omarchy bar visibility".into(),
                _ => format!("omarchy {}", action.run),
            }
        }
        "hypr" => format!("hyprctl keyword {}", action.keyword),
        "notify" => format!("notifications dnd={:?}", action.dnd.unwrap_or(false)),
        "power" => format!("power profile {}", action.profile),
        "audio" => format!("audio sink {}", action.sink),
        "exec" => format!("exec (irreversible): {}", action.command),
        other => format!("unknown action: {other}"),
    }
}

pub fn revision(dir: &Path) -> Result<String, PackError> {
    let text = dir.to_string_lossy().to_string();
    let output = Command::new("git")
        .args(["-C", &text, "rev-parse", "HEAD"])
        .output()
        .map_err(|e| PackError::Revision(e.to_string()))?;
    if !output.status.success() {
        return Err(PackError::Revision("not a git checkout".into()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Resolve a pack file to a regular tracked file inside the checkout. A symlink
/// or a path escaping the tree would not be covered by the reviewed revision.
fn safe_pack_file(dir: &Path, name: &str) -> Result<std::path::PathBuf, PackError> {
    let source = dir.join(name);
    let meta =
        std::fs::symlink_metadata(&source).map_err(|e| PackError::UnsafePackFile(e.to_string()))?;
    if meta.file_type().is_symlink() || !meta.is_file() {
        return Err(PackError::UnsafePackFile(format!(
            "{name} is not a regular tracked file"
        )));
    }
    let root = std::fs::canonicalize(dir).map_err(|e| PackError::UnsafePackFile(e.to_string()))?;
    let resolved =
        std::fs::canonicalize(&source).map_err(|e| PackError::UnsafePackFile(e.to_string()))?;
    if !resolved.starts_with(&root) {
        return Err(PackError::UnsafePackFile(format!(
            "{name} resolves outside the checkout"
        )));
    }
    if !revision(dir).unwrap_or_default().is_empty() {
        let checkout = dir.to_string_lossy().to_string();
        let tracked = Command::new("git")
            .args(["-C", &checkout, "ls-files", "--error-unmatch", name])
            .output()
            .map_err(|e| PackError::UnsafePackFile(e.to_string()))?;
        if !tracked.status.success() {
            return Err(PackError::UnsafePackFile(format!(
                "{name} is not tracked by git"
            )));
        }
    }
    Ok(source)
}

/// Content hash of the two files that determine the pack's behaviour.
fn content_digest(dir: &Path, manifest: &PackManifest) -> Result<String, PackError> {
    let manifest_path = safe_pack_file(dir, "scene-pack.toml")?;
    let scenes_path = safe_pack_file(dir, &manifest.scenes_file())?;
    let a = manifest_path.to_string_lossy().to_string();
    let b = scenes_path.to_string_lossy().to_string();
    let output = Command::new("git")
        .args(["hash-object", &a, &b])
        .output()
        .map_err(|e| PackError::Revision(e.to_string()))?;
    if !output.status.success() {
        return Err(PackError::Revision("git hash-object failed".into()));
    }
    let hashes: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_string)
        .collect();
    Ok(hashes.join(":"))
}

/// Derive what the pack actually does from its scenes file, not from the
/// author-declared `permissions` block.
pub fn review(dir: &Path, manifest: &PackManifest) -> Result<PackReview, PackError> {
    let scenes_file = manifest.scenes_file();
    let _manifest_file = safe_pack_file(dir, "scene-pack.toml")?;
    let source = safe_pack_file(dir, &scenes_file)?;
    let config =
        Config::load(&source).map_err(|e| PackError::Parse(format!("{scenes_file}: {e}")))?;
    let mut permissions: Vec<String> = Vec::new();
    for scene in config.scene.values() {
        for action in &scene.actions {
            permissions.push(actual_permission(action));
        }
    }
    permissions.sort();
    permissions.dedup();
    let rev = revision(dir).unwrap_or_default();
    let digest = content_digest(dir, manifest)?;
    Ok(PackReview {
        revision: rev,
        digest,
        permissions,
    })
}

impl PackManifest {
    pub fn review_summary(&self, review: &PackReview) -> String {
        let mut out = format!(
            "Pack: {} {}\n{}\nRevision: {}\nDigest: {}\nPermissions derived from {}:\n",
            self.pack.name,
            self.pack.version,
            self.pack.description,
            if review.revision.is_empty() {
                "local files"
            } else {
                &review.revision
            },
            review.digest,
            self.scenes_file()
        );
        if review.permissions.is_empty() {
            out.push_str("  none\n");
        } else {
            for permission in &review.permissions {
                out.push_str(&format!("  - {permission}\n"));
            }
        }
        out.push_str("Author-declared permissions are informational only; the list above is authoritative.\n");
        out
    }
}

const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
const MAX_SCENES_BYTES: u64 = 1024 * 1024;

fn valid_scenes_file(name: &str) -> bool {
    !name.is_empty()
        && name.ends_with(".toml")
        && !name.starts_with('.')
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains("..")
}

#[derive(Debug, Clone, Deserialize)]
pub struct PackInfo {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub version: String,
    /// Preferred TOML location: inside [pack] (bare keys after a table belong
    /// to that table). Kept optional for older packs that used a top-level key.
    #[serde(default)]
    pub scenes_file: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Permission {
    pub kind: String,
    pub value: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PackManifest {
    pub pack: PackInfo,
    #[serde(default)]
    pub permissions: Vec<Permission>,
    /// Legacy top-level placement; `pack.scenes_file` wins when both exist.
    #[serde(default)]
    pub scenes_file: Option<String>,
}

fn default_scenes_file() -> String {
    "scenes.toml".to_string()
}

impl PackManifest {
    pub fn scenes_file(&self) -> String {
        self.pack
            .scenes_file
            .clone()
            .or_else(|| self.scenes_file.clone())
            .unwrap_or_else(default_scenes_file)
    }

    pub fn load(path: &Path) -> Result<Self, PackError> {
        let file = std::fs::File::open(path).map_err(|e| PackError::Read(e.to_string()))?;
        let size = file
            .metadata()
            .map(|m| m.len())
            .map_err(|e| PackError::Read(e.to_string()))?;
        if size > MAX_MANIFEST_BYTES {
            return Err(PackError::TooLarge("scene-pack.toml".into()));
        }
        let mut text = String::new();
        file.take(MAX_MANIFEST_BYTES)
            .read_to_string(&mut text)
            .map_err(|e| PackError::Read(e.to_string()))?;
        let manifest: Self = toml::from_str(&text).map_err(|e| PackError::Parse(e.to_string()))?;
        let scenes_file = manifest.scenes_file();
        if !valid_scenes_file(&scenes_file) {
            return Err(PackError::InvalidScenesFile(scenes_file));
        }
        Ok(manifest)
    }

    pub fn load_from_pack(dir: &Path) -> Result<Self, PackError> {
        let path = safe_pack_file(dir, "scene-pack.toml")?;
        Self::load(&path)
    }

    pub fn summary(&self) -> String {
        let mut out = format!(
            "Pack: {} {}\n{}\nPermissions:\n",
            self.pack.name, self.pack.version, self.pack.description
        );
        if self.permissions.is_empty() {
            out.push_str("  none declared\n");
        } else {
            for permission in &self.permissions {
                out.push_str(&format!(
                    "  - {} = {}  ({})\n",
                    permission.kind, permission.value, permission.description
                ));
            }
        }
        out.push_str(&format!("Scenes file: {}\n", self.scenes_file()));
        out
    }
}

/// Clone a pack to a temporary directory, load its manifest and return
/// (temp dir, manifest). The caller removes the temp dir after use.
pub fn fetch(url: &str) -> Result<(std::path::PathBuf, PackManifest), PackError> {
    let dir = std::env::temp_dir().join(format!(
        "scenes-pack-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or_default()
    ));
    let result = (|| -> Result<PackManifest, PackError> {
        let status = Command::new("git")
            .args(["clone", "--depth", "1", "--quiet", url])
            .arg(&dir)
            .status()
            .map_err(|e| PackError::Git(e.to_string()))?;
        if !status.success() {
            return Err(PackError::Git(format!("exit status {status}")));
        }
        PackManifest::load_from_pack(&dir)
    })();
    match result {
        Ok(manifest) => Ok((dir, manifest)),
        Err(err) => {
            let _ = std::fs::remove_dir_all(&dir);
            Err(err)
        }
    }
}

/// Install the pack's scenes file into the user config. Refuses to overwrite.
pub fn install_pack(
    dir: &Path,
    manifest: &PackManifest,
    config_dir: &Path,
    confirm: bool,
    expected_revision: Option<&str>,
    expected_digest: Option<&str>,
) -> Result<std::path::PathBuf, PackError> {
    if !confirm {
        return Err(PackError::NeedsConfirm);
    }
    if let Some(expected) = expected_revision {
        let actual = revision(dir)?;
        if actual != expected {
            return Err(PackError::RevisionMismatch {
                expected: expected.to_string(),
                actual,
            });
        }
    }
    let scenes_file = manifest.scenes_file();
    let _manifest_file = safe_pack_file(dir, "scene-pack.toml")?;
    let source = safe_pack_file(dir, &scenes_file)?;
    if let Some(expected) = expected_digest {
        let actual = content_digest(dir, manifest)?;
        if actual != expected {
            return Err(PackError::DigestMismatch {
                expected: expected.to_string(),
                actual,
            });
        }
    }
    let size = std::fs::metadata(&source)
        .map(|m| m.len())
        .map_err(|e| PackError::Copy(e.to_string()))?;
    if size > MAX_SCENES_BYTES {
        return Err(PackError::TooLarge(scenes_file));
    }
    let target = config_dir.join(&scenes_file);
    if target.exists() {
        return Err(PackError::Copy(format!(
            "{} already exists; remove it first",
            target.display()
        )));
    }
    std::fs::create_dir_all(config_dir).map_err(|e| PackError::Copy(e.to_string()))?;
    std::fs::copy(&source, &target).map_err(|e| PackError::Copy(e.to_string()))?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_manifest_and_summarises_permissions() {
        let dir = std::env::temp_dir().join(format!("scenes-pack-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("scene-pack.toml");
        std::fs::write(
            &file,
            "[pack]\nname = \"Focus Pack\"\nversion = \"1.0\"\ndescription = \"Deep work scenes\"\n\n[[permissions]]\nkind = \"theme\"\nvalue = \"Tokyo Night\"\ndescription = \"changes the theme\"\n",
        )
        .unwrap_or_default();
        let manifest = PackManifest::load(&file).unwrap_or_else(|e| panic!("{e}"));
        let summary = manifest.summary();
        assert!(summary.contains("Focus Pack"));
        assert!(summary.contains("theme = Tokyo Night"));
        let _ = std::fs::remove_file(file);
        let _ = std::fs::remove_dir(dir);
    }

    #[test]
    fn rejects_path_traversal_in_scenes_file() {
        let dir = std::env::temp_dir().join(format!("scenes-pack-bad-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("scene-pack.toml");
        std::fs::write(
            &file,
            "[pack]\nname = \"Bad\"\nversion = \"1\"\ndescription = \"\"\nscenes_file = \"../../etc/passwd.toml\"\n",
        )
        .unwrap_or_default();
        assert!(matches!(
            PackManifest::load(&file),
            Err(PackError::InvalidScenesFile(_))
        ));
        let _ = std::fs::remove_file(file);
        let _ = std::fs::remove_dir(dir);
    }

    #[test]
    fn accepts_legacy_top_level_scenes_file() {
        let dir = std::env::temp_dir().join(format!("scenes-pack-legacy-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("scene-pack.toml");
        std::fs::write(
            &file,
            "scenes_file = \"legacy.toml\"\n[pack]\nname = \"Legacy\"\nversion = \"1\"\ndescription = \"\"\n",
        )
        .unwrap_or_default();
        let manifest = PackManifest::load(&file).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(manifest.scenes_file(), "legacy.toml");
        let _ = std::fs::remove_file(file);
        let _ = std::fs::remove_dir(dir);
    }

    #[test]
    fn derives_permissions_from_actions() {
        let dir = std::env::temp_dir().join(format!("scenes-pack-review-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let scenes = dir.join("scenes.toml");
        std::fs::write(
            dir.join("scene-pack.toml"),
            "[pack]\nname = \"x\"\nversion = \"1\"\ndescription = \"\"\nscenes_file = \"scenes.toml\"\n",
        )
        .unwrap_or_default();
        std::fs::write(
            &scenes,
            "[scene.focus]\npriority = 50\n\n  [[scene.focus.when]]\n  window_class = [\"code\"]\n\n  [[scene.focus.do]]\nkind = \"omarchy\"\nrun = \"theme set Nord\"\n\n  [[scene.focus.do]]\nkind = \"hypr\"\nkeyword = \"animations:enabled\"\nvalue = \"0\"\n",
        )
        .unwrap_or_default();
        let manifest = PackManifest {
            pack: PackInfo {
                name: "x".into(),
                description: String::new(),
                version: String::new(),
                scenes_file: Some("scenes.toml".into()),
            },
            permissions: Vec::new(),
            scenes_file: None,
        };
        let review = review(&dir, &manifest).unwrap_or_else(|e| panic!("{e}"));
        assert!(
            review
                .permissions
                .iter()
                .any(|p| p.contains("omarchy theme change"))
        );
        assert!(
            review
                .permissions
                .iter()
                .any(|p| p.contains("animations:enabled"))
        );
        let _ = std::fs::remove_file(scenes);
        let _ = std::fs::remove_dir(dir);
    }

    #[test]
    fn rejects_symlinked_scenes_file() {
        use std::os::unix::fs::symlink;

        let dir = std::env::temp_dir().join(format!("scenes-pack-symlink-{}", std::process::id()));
        let outside =
            std::env::temp_dir().join(format!("scenes-pack-outside-{}.toml", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(&outside, "[scene.x]\npriority = 1\n").unwrap_or_default();
        std::fs::write(
            dir.join("scene-pack.toml"),
            "[pack]\nname = \"x\"\nversion = \"1\"\ndescription = \"\"\nscenes_file = \"scenes.toml\"\n",
        )
        .unwrap_or_default();
        let linked = dir.join("scenes.toml");
        let _ = std::fs::remove_file(&linked);
        symlink(&outside, &linked).unwrap_or_default();

        let manifest = PackManifest {
            pack: PackInfo {
                name: "x".into(),
                description: String::new(),
                version: String::new(),
                scenes_file: Some("scenes.toml".into()),
            },
            permissions: Vec::new(),
            scenes_file: None,
        };
        assert!(matches!(
            review(&dir, &manifest),
            Err(PackError::UnsafePackFile(_))
        ));
        assert!(matches!(
            install_pack(
                &dir,
                &manifest,
                std::path::Path::new("/tmp/nope"),
                true,
                None,
                None
            ),
            Err(PackError::UnsafePackFile(_))
        ));
        let _ = std::fs::remove_file(linked);
        let _ = std::fs::remove_file(outside);
        let _ = std::fs::remove_dir(dir);
    }

    #[test]
    fn rejects_symlinked_manifest() {
        use std::os::unix::fs::symlink;

        let dir = std::env::temp_dir().join(format!("scenes-pack-manifest-{}", std::process::id()));
        let outside = std::env::temp_dir().join(format!(
            "scenes-pack-manifest-out-{}.toml",
            std::process::id()
        ));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(
            &outside,
            "[pack]\nname = \"x\"\nversion = \"1\"\ndescription = \"\"\nscenes_file = \"scenes.toml\"\n",
        )
        .unwrap_or_default();
        let linked = dir.join("scene-pack.toml");
        let _ = std::fs::remove_file(&linked);
        symlink(&outside, &linked).unwrap_or_default();
        assert!(matches!(
            PackManifest::load_from_pack(&dir),
            Err(PackError::UnsafePackFile(_))
        ));
        let _ = std::fs::remove_file(linked);
        let _ = std::fs::remove_file(outside);
        let _ = std::fs::remove_dir(dir);
    }

    #[test]
    fn rejects_digest_mismatch() {
        let dir = std::env::temp_dir().join(format!("scenes-pack-digest-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(
            dir.join("scene-pack.toml"),
            "[pack]\nname = \"x\"\nversion = \"1\"\ndescription = \"\"\nscenes_file = \"scenes.toml\"\n",
        )
        .unwrap_or_default();
        std::fs::write(
            dir.join("scenes.toml"),
            "[scene.x]\npriority = 1\n\n  [[scene.x.when]]\n  window_class = [\"code\"]\n\n  [[scene.x.do]]\nkind = \"omarchy\"\nrun = \"theme set Nord\"\n",
        )
        .unwrap_or_default();
        let manifest = PackManifest::load_from_pack(&dir).unwrap_or_else(|e| panic!("{e}"));
        let result = install_pack(
            &dir,
            &manifest,
            std::path::Path::new("/tmp/nope-digest"),
            true,
            None,
            Some("bad-digest"),
        );
        assert!(matches!(result, Err(PackError::DigestMismatch { .. })));
        let _ = std::fs::remove_file(dir.join("scene-pack.toml"));
        let _ = std::fs::remove_file(dir.join("scenes.toml"));
        let _ = std::fs::remove_dir(dir);
    }

    #[test]
    fn refuses_install_without_confirmation() {
        let manifest = PackManifest {
            pack: PackInfo {
                name: "x".into(),
                description: String::new(),
                version: String::new(),
                scenes_file: None,
            },
            permissions: Vec::new(),
            scenes_file: None,
        };
        let result = install_pack(
            Path::new("/tmp"),
            &manifest,
            Path::new("/tmp/nope"),
            false,
            None,
            None,
        );
        assert!(matches!(result, Err(PackError::NeedsConfirm)));
    }
}

//! Scene packs: inspect a manifest and show its permissions before any install.
//! Installation never happens without an explicit confirmation flag.

use std::io::Read;
use std::path::Path;
use std::process::Command;

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
        PackManifest::load(&dir.join("scene-pack.toml"))
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
) -> Result<std::path::PathBuf, PackError> {
    if !confirm {
        return Err(PackError::NeedsConfirm);
    }
    let scenes_file = manifest.scenes_file();
    let source = dir.join(&scenes_file);
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
        let result = install_pack(Path::new("/tmp"), &manifest, Path::new("/tmp/nope"), false);
        assert!(matches!(result, Err(PackError::NeedsConfirm)));
    }
}

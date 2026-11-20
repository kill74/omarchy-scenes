//! Scene packs: inspect a manifest and show its permissions before any install.
//! Installation never happens without an explicit confirmation flag.

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
}

#[derive(Debug, Clone, Deserialize)]
pub struct PackInfo {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub version: String,
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
    #[serde(default = "default_scenes_file")]
    pub scenes_file: String,
}

fn default_scenes_file() -> String {
    "scenes.toml".to_string()
}

impl PackManifest {
    pub fn load(path: &Path) -> Result<Self, PackError> {
        let text = std::fs::read_to_string(path).map_err(|e| PackError::Read(e.to_string()))?;
        toml::from_str(&text).map_err(|e| PackError::Parse(e.to_string()))
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
        out.push_str(&format!("Scenes file: {}\n", self.scenes_file));
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
    let status = Command::new("git")
        .args(["clone", "--depth", "1", "--quiet", url])
        .arg(&dir)
        .status()
        .map_err(|e| PackError::Git(e.to_string()))?;
    if !status.success() {
        return Err(PackError::Git(format!("exit status {status}")));
    }
    let manifest = PackManifest::load(&dir.join("scene-pack.toml"))?;
    Ok((dir, manifest))
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
    let source = dir.join(&manifest.scenes_file);
    let target = config_dir.join(&manifest.scenes_file);
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
    fn refuses_install_without_confirmation() {
        let manifest = PackManifest {
            pack: PackInfo {
                name: "x".into(),
                description: String::new(),
                version: String::new(),
            },
            permissions: Vec::new(),
            scenes_file: "scenes.toml".into(),
        };
        let result = install_pack(Path::new("/tmp"), &manifest, Path::new("/tmp/nope"), false);
        assert!(matches!(result, Err(PackError::NeedsConfirm)));
    }
}

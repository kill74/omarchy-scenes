use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("cannot read config {path}: {source}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot parse config {path}: {source}")]
    Parse {
        path: String,
        #[source]
        source: toml::de::Error,
    },
    #[error("invalid scene '{scene}': {message}")]
    Invalid { scene: String, message: String },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub debounce_ms: u64,
    pub manual_override_pause: String,
    pub log_events: bool,
    #[serde(default)]
    pub signal_poll_ms: u64,
    #[serde(default)]
    pub notify_on_enter: bool,
    #[serde(default)]
    pub notify_on_leave: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            debounce_ms: 400,
            manual_override_pause: "30m".to_string(),
            log_events: true,
            signal_poll_ms: 5000,
            notify_on_enter: false,
            notify_on_leave: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub settings: Settings,
    pub scene: BTreeMap<String, Scene>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Scene {
    pub priority: i32,
    #[serde(default)]
    pub description: String,
    #[serde(default, rename = "when")]
    pub when: Vec<When>,
    #[serde(default, rename = "do")]
    pub actions: Vec<ActionSpec>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct When {
    #[serde(default)]
    pub window_class: Vec<String>,
    #[serde(default)]
    pub window_title: Vec<String>,
    #[serde(default)]
    pub workspace: Vec<String>,
    #[serde(default)]
    pub monitor_name: Vec<String>,
    #[serde(default)]
    pub git_repo: Vec<String>,
    #[serde(default)]
    pub cwd: Vec<String>,
    #[serde(default)]
    pub ssid: Vec<String>,
    #[serde(default)]
    pub audio_sink: Vec<String>,
    #[serde(default)]
    pub monitors_min: Option<usize>,
    #[serde(default)]
    pub battery_below: Option<u8>,
    #[serde(default)]
    pub power: Option<String>,
    #[serde(default)]
    pub time_range: Option<String>,
    #[serde(default)]
    pub monitors_max: Option<usize>,
    #[serde(default)]
    pub game: Option<bool>,
}

impl When {
    pub fn is_empty(&self) -> bool {
        self.window_class.is_empty()
            && self.window_title.is_empty()
            && self.workspace.is_empty()
            && self.monitor_name.is_empty()
            && self.git_repo.is_empty()
            && self.cwd.is_empty()
            && self.ssid.is_empty()
            && self.audio_sink.is_empty()
            && self.monitors_min.is_none()
            && self.battery_below.is_none()
            && self.power.is_none()
            && self.time_range.is_none()
            && self.monitors_max.is_none()
            && self.game.is_none()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ActionSpec {
    pub kind: String,
    #[serde(default)]
    pub run: String,
    #[serde(default)]
    pub keyword: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub dnd: Option<bool>,
    #[serde(default)]
    pub profile: String,
    #[serde(default)]
    pub sink: String,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub irreversible: bool,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.display().to_string(),
            source,
        })?;
        let config: Config = toml::from_str(&text).map_err(|source| ConfigError::Parse {
            path: path.display().to_string(),
            source,
        })?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.scene.is_empty() {
            return Err(ConfigError::Invalid {
                scene: "<config>".to_string(),
                message: "no [[scene.*]] sections found".to_string(),
            });
        }
        for (name, scene) in &self.scene {
            if scene.when.is_empty() {
                return Err(ConfigError::Invalid {
                    scene: name.clone(),
                    message: "needs at least one [[scene.<name>.when]] block".to_string(),
                });
            }
            if scene.actions.is_empty() {
                return Err(ConfigError::Invalid {
                    scene: name.clone(),
                    message: "needs at least one [[scene.<name>.do]] block".to_string(),
                });
            }
            for when in &scene.when {
                if when.is_empty() {
                    return Err(ConfigError::Invalid {
                        scene: name.clone(),
                        message: "a when block has no conditions".to_string(),
                    });
                }
            }
            for action in &scene.actions {
                let required = match action.kind.as_str() {
                    "omarchy" => "run",
                    "hypr" => "keyword",
                    "notify" => "dnd",
                    "power" => "profile",
                    "audio" => "sink",
                    "exec" => "command",
                    other => {
                        return Err(ConfigError::Invalid {
                            scene: name.clone(),
                            message: format!("unknown action kind '{other}'"),
                        });
                    }
                };
                let present = match required {
                    "run" => !action.run.is_empty(),
                    "keyword" => !action.keyword.is_empty(),
                    "dnd" => action.dnd.is_some(),
                    "profile" => !action.profile.is_empty(),
                    "sink" => !action.sink.is_empty(),
                    "command" => !action.command.is_empty(),
                    _ => false,
                };
                if !present {
                    return Err(ConfigError::Invalid {
                        scene: name.clone(),
                        message: format!("{} action needs '{required}'", action.kind),
                    });
                }
                if action.kind == "exec" && !action.irreversible {
                    return Err(ConfigError::Invalid {
                        scene: name.clone(),
                        message: "exec actions must set irreversible = true".to_string(),
                    });
                }
            }
        }
        Ok(())
    }
}

/// Minimal glob matcher: `*` matches any run of characters, `?` matches one.
/// Enough for the patterns the roadmap uses (`steam_app_*`, `~/dev/*`) and
/// deliberately dependency-free.
pub fn glob_match(pattern: &str, value: &str) -> bool {
    let p = pattern.as_bytes();
    let v = value.as_bytes();
    let (mut pi, mut vi) = (0usize, 0usize);
    let (mut star, mut mark) = (None::<usize>, 0usize);
    while vi < v.len() {
        if pi < p.len() && (p[pi] == b'?' || p[pi] == v[vi]) {
            pi += 1;
            vi += 1;
        } else if pi < p.len() && p[pi] == b'*' {
            star = Some(pi);
            mark = vi;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            vi = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == b'*' {
        pi += 1;
    }
    pi == p.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
[settings]
debounce_ms = 400
manual_override_pause = "30m"
log_events = true

[scene.focus]
priority = 50
description = "Deep work"

  [[scene.focus.when]]
  window_class = ["code", "kitty"]

  [[scene.focus.do]]
  kind = "omarchy"
  run = "theme set 'Tokyo Night'"

[scene.gaming]
priority = 80
  [[scene.gaming.when]]
  window_class = ["steam_app_*"]
  [[scene.gaming.do]]
  kind = "hypr"
  keyword = "animations:enabled"
  value = "0"
"#;

    #[test]
    fn parses_sample_config() {
        let cfg: Config = toml::from_str(SAMPLE).unwrap_or_else(|e| panic!("{e}"));
        cfg.validate().unwrap();
        assert_eq!(cfg.scene.len(), 2);
        assert_eq!(cfg.scene["focus"].priority, 50);
        assert_eq!(cfg.scene["focus"].actions[0].run, "theme set 'Tokyo Night'");
    }

    #[test]
    fn rejects_unknown_action_kind() {
        let bad = SAMPLE.replace("kind = \"hypr\"", "kind = \"teleport\"");
        let cfg: Config = toml::from_str(&bad).unwrap_or_else(|e| panic!("{e}"));
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn globs() {
        assert!(glob_match("steam_app_*", "steam_app_123"));
        assert!(glob_match("~/dev/*", "~/dev/omarchy"));
        assert!(glob_match("*code*", "vscode"));
        assert!(!glob_match("steam_app_*", "steam"));
        assert!(glob_match("???", "abc"));
    }
}

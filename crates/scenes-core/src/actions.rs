use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::config::ActionSpec;

#[derive(Debug, Error)]
pub enum ActionError {
    #[error("command '{program}' failed: {message}")]
    Command { program: String, message: String },
    #[error("unsafe omarchy command: {0}")]
    UnsafeOmarchy(String),
    #[error("unsupported action kind '{0}' in this phase")]
    Unsupported(String),
    #[error("state directory is unavailable")]
    NoStateDir,
    #[error("cannot parse hyprctl output: {0}")]
    BadHyprOutput(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Snapshot {
    None,
    Theme {
        slug: String,
        background: String,
        bar_background: String,
        bar_off: bool,
    },
    Hypr {
        key: String,
        value: String,
    },
    Dnd {
        quiet_text: String,
    },
}

pub trait ActionRunner: Send + Sync {
    fn apply(&self, action: &ActionSpec) -> Result<Snapshot, ActionError>;
    fn revert(&self, snapshot: &Snapshot) -> Result<(), ActionError>;
    fn describe(&self, action: &ActionSpec) -> String;
    fn notify(&self, _message: &str) -> Result<(), ActionError> {
        Ok(())
    }
}

fn run(program: &str, args: &[String]) -> Result<String, ActionError> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|source| ActionError::Command {
            program: program.to_string(),
            message: source.to_string(),
        })?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(ActionError::Command {
            program: program.to_string(),
            message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        })
    }
}

/// Run an Omarchy CLI command. The internal marker suppresses the manual
/// override hook, so a scene applying a theme does not pause itself.
fn omarchy(args: &[String]) -> Result<String, ActionError> {
    let output = Command::new("omarchy")
        .args(args)
        .env("SCENES_INTERNAL", "1")
        .output()
        .map_err(|source| ActionError::Command {
            program: "omarchy".into(),
            message: source.to_string(),
        })?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(ActionError::Command {
            program: "omarchy".into(),
            message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        })
    }
}

fn quiet_path() -> Result<PathBuf, ActionError> {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))
        .ok_or(ActionError::NoStateDir)
        .map(|base| base.join("omarchy/notifications/quiet.json"))
}

fn dnd_toggle() -> Result<(), ActionError> {
    run(
        "omarchy-shell",
        &["notifications".into(), "toggleDnd".into()],
    )?;
    Ok(())
}

/// Only commands that change desktop state we can restore need a theme snapshot.
fn omarchy_needs_snapshot(args: &[String]) -> bool {
    let first = args.first().map(String::as_str);
    let second = args.get(1).map(String::as_str);
    matches!(
        (first, second),
        (Some("theme"), Some("set" | "bg" | "refresh"))
            | (Some("bar"), Some("background"))
            | (Some("toggle"), Some("bar"))
    )
}

/// Split a command line into argv, honouring single and double quotes.
pub fn split_command(input: &str) -> Result<Vec<String>, ActionError> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut started = false;
    for ch in input.chars() {
        match quote {
            Some(q) => {
                if ch == q {
                    quote = None;
                } else {
                    current.push(ch);
                }
            }
            None => match ch {
                '\'' | '"' => {
                    quote = Some(ch);
                    started = true;
                }
                c if c.is_whitespace() => {
                    if started {
                        out.push(std::mem::take(&mut current));
                        started = false;
                    }
                }
                c => {
                    current.push(c);
                    started = true;
                }
            },
        }
    }
    if quote.is_some() {
        return Err(ActionError::UnsafeOmarchy("unclosed quote".into()));
    }
    if started {
        out.push(current);
    }
    Ok(out)
}

/// The real action runner. Only fixed binaries are spawned; arguments come
/// from the scene config but never through a shell, so no injection is possible.
pub struct CliRunner;

impl ActionRunner for CliRunner {
    fn apply(&self, action: &ActionSpec) -> Result<Snapshot, ActionError> {
        match action.kind.as_str() {
            "omarchy" => {
                let argv = split_command(&action.run)?;
                let program = argv.first().map(String::as_str).unwrap_or("");
                if program != "omarchy" {
                    return Err(ActionError::UnsafeOmarchy(action.run.clone()));
                }
                let rest = &argv[1..];
                let before = if omarchy_needs_snapshot(rest) {
                    self.snapshot_theme()?
                } else {
                    Snapshot::None
                };
                omarchy(rest)?;
                Ok(before)
            }
            "hypr" => {
                let key = action.keyword.as_str();
                let value = hyprctl_value(key)?;
                let args = vec!["keyword".to_string(), key.to_string(), action.value.clone()];
                run("hyprctl", &args)?;
                Ok(Snapshot::Hypr {
                    key: key.to_string(),
                    value,
                })
            }
            "notify" => {
                let path = quiet_path()?;
                let quiet_text = std::fs::read_to_string(&path).unwrap_or_default();
                let currently = quiet_text.contains("true");
                let desired = action.dnd.unwrap_or(false);
                if currently != desired {
                    dnd_toggle()?;
                }
                Ok(Snapshot::Dnd { quiet_text })
            }
            other => Err(ActionError::Unsupported(other.to_string())),
        }
    }

    fn revert(&self, snapshot: &Snapshot) -> Result<(), ActionError> {
        match snapshot {
            Snapshot::None => Ok(()),
            Snapshot::Theme {
                slug,
                background,
                bar_background,
                bar_off,
            } => {
                omarchy(&["theme".into(), "set".into(), slug.clone()])?;
                if !background.is_empty() {
                    omarchy(&[
                        "theme".into(),
                        "bg".into(),
                        "set".into(),
                        background.clone(),
                    ])?;
                }
                if !bar_background.is_empty() {
                    omarchy(&["bar".into(), "background".into(), bar_background.clone()])?;
                }
                let wanted = if *bar_off { "on" } else { "off" };
                omarchy(&["toggle".into(), "bar".into(), wanted.into()])?;
                Ok(())
            }
            Snapshot::Hypr { key, value } => {
                run("hyprctl", &["keyword".into(), key.clone(), value.clone()]).map(|_| ())
            }
            Snapshot::Dnd { quiet_text } => {
                let path = quiet_path()?;
                let wanted = quiet_text.contains("true");
                let current = std::fs::read_to_string(&path)
                    .unwrap_or_default()
                    .contains("true");
                if wanted != current {
                    dnd_toggle()?;
                }
                Ok(())
            }
        }
    }

    fn describe(&self, action: &ActionSpec) -> String {
        match action.kind.as_str() {
            "omarchy" => format!("omarchy {}", action.run),
            "hypr" => format!("hyprctl keyword {} {}", action.keyword, action.value),
            "notify" => format!("notifications dnd={:?}", action.dnd),
            other => other.to_string(),
        }
    }

    fn notify(&self, message: &str) -> Result<(), ActionError> {
        omarchy(&[
            "notification".into(),
            "send".into(),
            "Scenes".into(),
            message.into(),
        ])?;
        Ok(())
    }
}

impl CliRunner {
    fn snapshot_theme(&self) -> Result<Snapshot, ActionError> {
        let base = std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))
            .ok_or(ActionError::NoStateDir)?;
        let slug = std::fs::read_to_string(base.join("omarchy/current/theme.name"))
            .unwrap_or_default()
            .trim()
            .to_string();
        let background = std::fs::read_link(base.join("omarchy/current/background"))
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let bar_background = omarchy(&["bar".into(), "background".into()]).unwrap_or_default();
        let bar_off = base.join("omarchy/toggles/bar-off").exists();
        Ok(Snapshot::Theme {
            slug,
            background,
            bar_background,
            bar_off,
        })
    }
}

fn hyprctl_value(key: &str) -> Result<String, ActionError> {
    let output = Command::new("hyprctl")
        .args(["-j", "getoption", key])
        .output()
        .map_err(|source| ActionError::Command {
            program: "hyprctl".into(),
            message: source.to_string(),
        })?;
    if !output.status.success() {
        return Err(ActionError::Command {
            program: "hyprctl".into(),
            message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| ActionError::BadHyprOutput(e.to_string()))?;
    for field in ["str", "bool", "int", "float"] {
        if let Some(value) = json.get(field) {
            return Ok(match value {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            });
        }
    }
    Err(ActionError::BadHyprOutput(text.to_string()))
}

/// In-memory runner for tests: records what it was asked to do.
#[derive(Default)]
pub struct MockRunner {
    pub applied: Mutex<Vec<String>>,
    pub reverted: Mutex<Vec<String>>,
    pub notifications: Mutex<Vec<String>>,
}

impl ActionRunner for MockRunner {
    fn apply(&self, action: &ActionSpec) -> Result<Snapshot, ActionError> {
        self.applied
            .lock()
            .map_err(|_| ActionError::Unsupported("poisoned mock".into()))?
            .push(format!("{:?}", action));
        Ok(Snapshot::None)
    }

    fn revert(&self, snapshot: &Snapshot) -> Result<(), ActionError> {
        self.reverted
            .lock()
            .map_err(|_| ActionError::Unsupported("poisoned mock".into()))?
            .push(format!("{snapshot:?}"));
        Ok(())
    }

    fn describe(&self, action: &ActionSpec) -> String {
        format!("mock {:?}", action.kind)
    }

    fn notify(&self, message: &str) -> Result<(), ActionError> {
        self.notifications
            .lock()
            .map_err(|_| ActionError::Unsupported("poisoned mock".into()))?
            .push(message.to_string());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_quoted_commands() {
        let argv = split_command("theme set 'Tokyo Night'").unwrap_or_default();
        assert_eq!(argv, vec!["theme", "set", "Tokyo Night"]);
    }

    #[test]
    fn rejects_unclosed_quotes() {
        assert!(split_command("theme set 'Tokyo").is_err());
    }

    #[test]
    fn snapshots_only_state_changing_omarchy_commands() {
        let args = |items: &[&str]| items.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(omarchy_needs_snapshot(&args(&["theme", "set", "Nord"])));
        assert!(omarchy_needs_snapshot(&args(&[
            "bar",
            "background",
            "opaque"
        ])));
        assert!(omarchy_needs_snapshot(&args(&["toggle", "bar", "on"])));
        assert!(!omarchy_needs_snapshot(&args(&[
            "notification",
            "send",
            "hi"
        ])));
        assert!(!omarchy_needs_snapshot(&args(&["theme", "list"])));
    }

    #[test]
    fn mock_records_actions() {
        let mock = MockRunner::default();
        let action = ActionSpec {
            kind: "omarchy".into(),
            run: "theme set X".into(),
            keyword: String::new(),
            value: String::new(),
            dnd: None,
            profile: String::new(),
            sink: String::new(),
            command: String::new(),
            irreversible: false,
        };
        let _ = mock.apply(&action);
        assert_eq!(
            mock.applied.lock().unwrap_or_else(|e| e.into_inner()).len(),
            1
        );
    }
}

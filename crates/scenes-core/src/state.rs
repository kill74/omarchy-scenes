use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::actions::{ActionError, ActionRunner, Snapshot};
use crate::config::Config;
use crate::events_log::{EventRecord, EventSink};
use crate::rules::best_match;
use crate::signal::{Context, Signal};

#[derive(Debug, Error)]
pub enum ManagerError {
    #[error(transparent)]
    Action(#[from] ActionError),
    #[error("state file: {0}")]
    State(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Persisted {
    active_scene: Option<String>,
    snapshots: Vec<Snapshot>,
    paused_until: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManagerState {
    Idle,
    Evaluating,
    Debouncing,
    Applying,
    Active(String),
    Reverting,
    Paused,
    RollingBack,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

pub struct SceneManager<R: ActionRunner> {
    config: Config,
    runner: R,
    state: ManagerState,
    active: Option<String>,
    snapshots: Vec<Snapshot>,
    paused_until: Option<u64>,
    state_path: PathBuf,
    sink: Option<Arc<dyn EventSink>>,
    entered_at: Option<u64>,
    last_signals: String,
}

impl<R: ActionRunner> SceneManager<R> {
    pub fn new(config: Config, runner: R, state_path: &Path) -> Result<Self, ManagerError> {
        let mut manager = Self {
            config,
            runner,
            state: ManagerState::Idle,
            active: None,
            snapshots: Vec::new(),
            paused_until: None,
            state_path: state_path.to_path_buf(),
            sink: None,
            entered_at: None,
            last_signals: String::new(),
        };
        manager.recover()?;
        Ok(manager)
    }

    fn recover(&mut self) -> Result<(), ManagerError> {
        let Some(saved) = read_state(&self.state_path)? else {
            return Ok(());
        };
        for snapshot in saved.snapshots.iter().rev() {
            if let Err(err) = self.runner.revert(snapshot) {
                return Err(ManagerError::State(format!(
                    "recovery revert failed: {err}"
                )));
            }
        }
        let _ = std::fs::remove_file(&self.state_path);
        self.active = None;
        self.snapshots.clear();
        self.paused_until = saved.paused_until.filter(|until| *until > now_secs());
        self.state = if self.paused_until.is_some() {
            ManagerState::Paused
        } else {
            ManagerState::Idle
        };
        Ok(())
    }

    fn persist(&self) -> Result<(), ManagerError> {
        let data = Persisted {
            active_scene: self.active.clone(),
            snapshots: self.snapshots.clone(),
            paused_until: self.paused_until,
        };
        let text = serde_json::to_string(&data)
            .map_err(|e| ManagerError::State(format!("serialize: {e}")))?;
        if let Some(parent) = self.state_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| ManagerError::State(format!("mkdir: {e}")))?;
        }
        let tmp = self.state_path.with_extension("tmp");
        std::fs::write(&tmp, text).map_err(|e| ManagerError::State(format!("write: {e}")))?;
        std::fs::rename(&tmp, &self.state_path)
            .map_err(|e| ManagerError::State(format!("rename: {e}")))?;
        Ok(())
    }

    pub fn with_sink(mut self, sink: Arc<dyn EventSink>) -> Self {
        self.sink = Some(sink);
        self
    }

    fn log(&self, kind: &str, scene: &str, signals: &str, detail: &str) {
        if let Some(sink) = &self.sink
            && let Err(err) = sink.record(&EventRecord {
                kind: kind.to_string(),
                scene: scene.to_string(),
                signals: signals.to_string(),
                detail: detail.to_string(),
                ts: now_secs() as i64,
            })
        {
            eprintln!("scenes: event log write failed: {err}");
        }
    }

    /// Record a decision made outside signal handling (e.g. `scenes apply`).
    pub fn log_decision(&self, scene: &str, detail: &str) {
        self.log("decision", scene, &self.last_signals, detail);
    }

    pub fn state(&self) -> &ManagerState {
        &self.state
    }

    /// Publish the current state so consumers such as the bar widget can render
    /// while the daemon is running, even before the first scene transition.
    pub fn publish_state(&self) -> Result<(), ManagerError> {
        self.persist()
    }

    pub fn active_scene(&self) -> Option<&str> {
        self.active.as_deref()
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn handle_signal(&mut self, signal: &Signal, ctx: &Context) -> Result<(), ManagerError> {
        if let Some(until) = self.paused_until {
            if now_secs() < until {
                return Ok(());
            }
            self.paused_until = None;
            self.state = ManagerState::Idle;
        }

        self.last_signals = ctx.to_signal_json();
        let best = best_match(&self.config, ctx).map(|(name, _)| name.to_string());
        // A periodic tick that confirms the current scene is not a new decision.
        if best.as_deref() != self.active.as_deref() {
            self.log(
                "decision",
                best.as_deref().unwrap_or(""),
                &self.last_signals,
                "signal event",
            );
        }

        if matches!(signal, Signal::Manual) {
            return self.manual_override();
        }

        self.state = ManagerState::Evaluating;

        match best {
            None => {
                if self.active.is_some() {
                    self.revert_active()?;
                } else {
                    self.state = ManagerState::Idle;
                }
                Ok(())
            }
            Some(name) if self.active.as_deref() == Some(name.as_str()) => {
                self.state = ManagerState::Active(name);
                Ok(())
            }
            Some(name) => {
                if self.active.is_some() {
                    self.revert_active()?;
                }
                self.apply_scene(&name)
            }
        }
    }

    pub fn apply_scene(&mut self, name: &str) -> Result<(), ManagerError> {
        let scene = self
            .config
            .scene
            .get(name)
            .ok_or_else(|| ManagerError::State(format!("no scene named '{name}'")))?
            .clone();
        self.state = ManagerState::Applying;
        for action in &scene.actions {
            let snapshot = self.runner.apply(action)?;
            self.snapshots.push(snapshot);
            // Persist after each action so a crash mid-apply can still be undone.
            self.persist()?;
        }
        self.active = Some(name.to_string());
        self.entered_at = Some(now_secs());
        self.state = ManagerState::Active(name.to_string());
        self.log("enter", name, &self.last_signals.clone(), "applied");
        if self.config.settings.notify_on_enter {
            let _ = self.runner.notify(&format!("Entered scene {name}"));
        }
        self.persist()
    }

    pub fn revert_active(&mut self) -> Result<(), ManagerError> {
        self.state = ManagerState::Reverting;
        for snapshot in self.snapshots.iter().rev() {
            self.runner.revert(snapshot)?;
        }
        self.snapshots.clear();
        let leaving = self.active.take().unwrap_or_default();
        let seconds = self
            .entered_at
            .map(|start| now_secs().saturating_sub(start))
            .unwrap_or_default();
        self.entered_at = None;
        self.state = ManagerState::Idle;
        if !leaving.is_empty() {
            self.log("stay", &leaving, "", &seconds.to_string());
            self.log("leave", &leaving, "", "");
        }
        if self.config.settings.notify_on_leave && !leaving.is_empty() {
            let _ = self.runner.notify(&format!("Left scene {leaving}"));
        }
        self.persist()
    }

    pub fn manual_override(&mut self) -> Result<(), ManagerError> {
        if self.active.is_some() {
            self.revert_active()?;
        }
        let seconds = parse_duration(&self.config.settings.manual_override_pause);
        self.paused_until = Some(now_secs() + seconds);
        self.state = ManagerState::Paused;
        self.persist()
    }

    pub fn pause(&mut self, seconds: u64) -> Result<(), ManagerError> {
        if self.active.is_some() {
            self.revert_active()?;
        }
        self.paused_until = Some(now_secs() + seconds);
        self.state = ManagerState::Paused;
        self.persist()
    }

    pub fn resume(&mut self) -> Result<(), ManagerError> {
        self.paused_until = None;
        self.state = ManagerState::Idle;
        self.persist()
    }

    pub fn paused_remaining(&self) -> u64 {
        self.paused_until
            .map(|until| until.saturating_sub(now_secs()))
            .unwrap_or_default()
    }
}

fn parse_duration(text: &str) -> u64 {
    let text = text.trim();
    if let Some(value) = text.strip_suffix('m') {
        value.trim().parse::<u64>().unwrap_or(30) * 60
    } else if let Some(value) = text.strip_suffix('h') {
        value.trim().parse::<u64>().unwrap_or(1) * 3600
    } else {
        text.parse::<u64>().unwrap_or(1800)
    }
}

fn read_state(path: &Path) -> Result<Option<Persisted>, ManagerError> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(ManagerError::State(format!("read: {err}"))),
    };
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|e| ManagerError::State(format!("parse: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::MockRunner;
    use crate::config::{ActionSpec, Scene, When};
    use crate::events::HyprEvent;
    use crate::events_log::MockSink;
    use std::collections::BTreeMap;

    fn test_config() -> Config {
        let mut settings = Default::default();
        {
            let s: &mut crate::config::Settings = &mut settings;
            s.notify_on_enter = true;
            s.notify_on_leave = true;
        }
        let mut config = Config {
            settings,
            scene: BTreeMap::new(),
        };
        config.scene.insert(
            "focus".into(),
            Scene {
                priority: 50,
                description: "test".into(),
                when: vec![When {
                    window_class: vec!["code".into()],
                    ..Default::default()
                }],
                actions: vec![ActionSpec {
                    kind: "omarchy".into(),
                    run: "theme set X".into(),
                    keyword: String::new(),
                    value: String::new(),
                    dnd: None,
                    profile: String::new(),
                    sink: String::new(),
                    command: String::new(),
                    irreversible: false,
                }],
            },
        );
        config
    }

    fn temp_state(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("scenes-state-{name}-{}.json", std::process::id()))
    }

    #[test]
    fn applies_and_reverts_on_context_exit() {
        let path = temp_state("apply");
        let _ = std::fs::remove_file(&path);
        let mock = std::sync::Arc::new(MockRunner::default());
        let sink = std::sync::Arc::new(MockSink::default());
        let mut manager = SceneManager::new(test_config(), TestRunner(mock.clone()), &path)
            .unwrap_or_else(|e| panic!("{e}"))
            .with_sink(sink.clone());
        let mut ctx = Context {
            window_class: "code".into(),
            ..Default::default()
        };
        manager
            .handle_signal(
                &Signal::Hypr(HyprEvent::ActiveWindow {
                    class: "code".into(),
                    title: "x".into(),
                }),
                &ctx,
            )
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(manager.active_scene(), Some("focus"));
        assert_eq!(
            mock.applied.lock().unwrap_or_else(|e| e.into_inner()).len(),
            1
        );

        ctx.window_class = "foot".into();
        manager
            .handle_signal(
                &Signal::Hypr(HyprEvent::ActiveWindow {
                    class: "foot".into(),
                    title: "x".into(),
                }),
                &ctx,
            )
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(manager.active_scene(), None);
        assert_eq!(
            mock.reverted
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .len(),
            1
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn recovers_snapshot_after_crash() {
        let path = temp_state("crash");
        let data = r#"{"active_scene":"focus","snapshots":[{"Hypr":{"key":"animations:enabled","value":"1"}}],"paused_until":null}"#;
        std::fs::write(&path, data).unwrap_or_default();
        let mock = std::sync::Arc::new(MockRunner::default());
        let manager = SceneManager::new(test_config(), TestRunner(mock.clone()), &path)
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(manager.active_scene(), None);
        assert_eq!(
            mock.reverted
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .len(),
            1
        );
        assert!(!path.exists());
    }

    // Small wrapper so the mock can be shared with assertions in tests.
    struct TestRunner(std::sync::Arc<MockRunner>);
    impl ActionRunner for TestRunner {
        fn apply(&self, action: &ActionSpec) -> Result<Snapshot, ActionError> {
            self.0.apply(action)
        }
        fn revert(&self, snapshot: &Snapshot) -> Result<(), ActionError> {
            self.0.revert(snapshot)
        }
        fn describe(&self, action: &ActionSpec) -> String {
            self.0.describe(action)
        }
    }
}

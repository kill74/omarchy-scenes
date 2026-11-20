use crate::events::HyprEvent;

#[derive(Debug, Clone, Default)]
pub struct Context {
    pub window_class: String,
    pub window_title: String,
    pub workspace: String,
    pub monitor_name: String,
    pub monitor_count: usize,
    pub cwd: String,
    pub git_repo: String,
    pub power: String,
    pub battery: Option<u8>,
    pub ssid: String,
    pub audio_sink: String,
    pub minute_of_day: u32,
}

#[derive(Debug, Clone)]
pub enum Signal {
    Hypr(HyprEvent),
    Tick,
    MonitorCount(usize),
    Cwd(String),
    Power(String),
    Battery(u8),
    Ssid(String),
    AudioSink(String),
    Manual,
}

impl Context {
    pub fn apply(&mut self, signal: &Signal) {
        match signal {
            Signal::Hypr(event) => match event {
                HyprEvent::ActiveWindow { class, title } => {
                    self.window_class = class.clone();
                    self.window_title = title.clone();
                }
                HyprEvent::ActiveWindowV2 { .. } => {}
                HyprEvent::Workspace { id, .. } => self.workspace = id.clone(),
                HyprEvent::OpenWindow { class, .. } => {
                    if self.window_class.is_empty() {
                        self.window_class = class.clone();
                    }
                }
                HyprEvent::CloseWindow { .. } => {}
                HyprEvent::MonitorAdded { name } => {
                    self.monitor_name = name.clone();
                    self.monitor_count += 1;
                }
                HyprEvent::MonitorRemoved { .. } => {
                    self.monitor_count = self.monitor_count.saturating_sub(1);
                }
                HyprEvent::Other { .. } => {}
            },
            Signal::MonitorCount(count) => self.monitor_count = *count,
            Signal::Cwd(path) => {
                self.cwd = path.clone();
                self.git_repo = git_root(path);
            }
            Signal::Power(state) => self.power = state.clone(),
            Signal::Battery(level) => self.battery = Some(*level),
            Signal::Ssid(name) => self.ssid = name.clone(),
            Signal::AudioSink(name) => self.audio_sink = name.clone(),
            Signal::Tick | Signal::Manual => {}
        }
    }

    pub fn set_time(&mut self, hour: u32, minute: u32) {
        self.minute_of_day = hour.saturating_mul(60) + minute.min(59);
    }
}

/// Walk up from a directory looking for a `.git` marker. No git binary needed.
fn git_root(path: &str) -> String {
    let mut dir = std::path::Path::new(path).to_path_buf();
    loop {
        if dir.join(".git").exists() {
            return dir.to_string_lossy().to_string();
        }
        if !dir.pop() {
            return String::new();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn applies_window_and_workspace_signals() {
        let mut c = Context::default();
        c.apply(&Signal::Hypr(HyprEvent::Workspace {
            id: "3".into(),
            name: "3".into(),
        }));
        c.apply(&Signal::Hypr(HyprEvent::ActiveWindow {
            class: "code".into(),
            title: "main.rs".into(),
        }));
        assert_eq!(c.workspace, "3");
        assert_eq!(c.window_class, "code");
    }

    #[test]
    fn finds_git_root() {
        let base = std::env::temp_dir().join(format!("scenes-git-{}", std::process::id()));
        let nested = base.join("a/b");
        fs::create_dir_all(&nested).unwrap_or_default();
        fs::create_dir_all(base.join(".git")).unwrap_or_default();
        assert_eq!(
            git_root(nested.to_str().unwrap_or("")),
            base.to_string_lossy()
        );
        let _ = fs::remove_dir_all(base);
    }
}

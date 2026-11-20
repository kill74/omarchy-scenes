use std::path::PathBuf;
use std::sync::Arc;

use scenes_core::actions::CliRunner;
use scenes_core::config::Config;
use scenes_core::events::{self, HyprEvent};
use scenes_core::events_log::SqliteLog;
use scenes_core::signal::{Context, Signal};
use scenes_core::signals::{SignalSource, SystemSignals};
use scenes_core::state::{ManagerState, SceneManager};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Mutex;

#[derive(Debug, Deserialize)]
struct Request {
    method: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    seconds: u64,
}

#[derive(Debug, Serialize)]
struct Response {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl Response {
    fn ok(text: String) -> Self {
        Self {
            ok: true,
            result: Some(text),
            error: None,
        }
    }
    fn err(text: String) -> Self {
        Self {
            ok: false,
            result: None,
            error: Some(text),
        }
    }
}

type Shared = Arc<Mutex<SceneManager<CliRunner>>>;

fn state_dir() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("omarchy-scenes")
}

fn config_path() -> PathBuf {
    std::env::var_os("SCENES_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    std::env::var_os("HOME")
                        .map(|h| PathBuf::from(h).join(".config"))
                        .unwrap_or_else(|| PathBuf::from("/tmp"))
                })
                .join("omarchy/scenes/scenes.toml")
        })
}

fn socket2_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("SCENES_HYPR_SOCKET") {
        return Some(PathBuf::from(path));
    }
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from)?;
    let hypr = runtime.join("hypr");
    let mut found = None;
    if let Ok(entries) = std::fs::read_dir(hypr) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.contains(".socket2.sock") {
                found = Some(entry.path().join(".socket2.sock"));
            }
        }
    }
    found
}

fn ipc_path() -> PathBuf {
    std::env::var_os("SCENES_IPC_SOCKET")
        .map(PathBuf::from)
        .unwrap_or_else(|| state_dir().join("scenes.sock"))
}

fn context_snapshot(ctx: &mut Context) {
    if let Ok(out) = std::process::Command::new("date").args(["+%H %M"]).output() {
        let clock = String::from_utf8_lossy(&out.stdout).to_string();
        let parts: Vec<&str> = clock.split_whitespace().collect();
        if let (Some(h), Some(m)) = (parts.first(), parts.get(1)) {
            if let (Ok(hour), Ok(minute)) = (h.parse::<u32>(), m.parse::<u32>()) {
                ctx.set_time(hour, minute);
            }
        }
    }
    if let Ok(out) = std::process::Command::new("hyprctl")
        .args(["-j", "monitors"])
        .output()
    {
        if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
            if let Some(list) = value.as_array() {
                ctx.monitor_count = list.len();
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let config = match Config::load(&config_path()) {
        Ok(config) => config,
        Err(err) => {
            eprintln!("scenesd: {err}");
            std::process::exit(1);
        }
    };
    let debounce_ms = config.settings.debounce_ms;
    let signal_poll_ms = config.settings.signal_poll_ms;
    let state_dir = state_dir();
    if let Err(err) = std::fs::create_dir_all(&state_dir) {
        eprintln!("scenesd: cannot create state dir: {err}");
        std::process::exit(1);
    }
    let state_file = state_dir.join("state.json");
    let log_events = config.settings.log_events;
    let manager = match SceneManager::new(config, CliRunner, &state_file) {
        Ok(manager) => manager,
        Err(err) => {
            eprintln!("scenesd: recovery failed: {err}");
            std::process::exit(1);
        }
    };
    let manager = if log_events {
        match SqliteLog::open(&state_dir.join("events.db")) {
            Ok(log) => manager.with_sink(Arc::new(log)),
            Err(err) => {
                eprintln!("scenesd: event log unavailable: {err}");
                manager
            }
        }
    } else {
        manager
    };
    let shared: Shared = Arc::new(Mutex::new(manager));
    let ipc = ipc_path();
    let _ = std::fs::remove_file(&ipc);
    let listener = match UnixListener::bind(&ipc) {
        Ok(listener) => listener,
        Err(err) => {
            eprintln!("scenesd: cannot bind IPC socket: {err}");
            std::process::exit(1);
        }
    };

    let ipc_task = tokio::spawn(ipc_loop(listener, shared.clone()));
    let hypr_task = tokio::spawn(hypr_loop(shared.clone(), debounce_ms, signal_poll_ms));

    tokio::select! {
        _ = ipc_task => {}
        result = hypr_task => {
            if let Ok(Err(err)) = result {
                eprintln!("scenesd: hyprland listener stopped: {err}");
            }
        }
    }
}

async fn ipc_loop(listener: UnixListener, shared: Shared) {
    loop {
        let Ok((stream, _)) = listener.accept().await else {
            continue;
        };
        let shared = shared.clone();
        tokio::spawn(async move {
            if let Err(err) = handle_client(stream, shared).await {
                eprintln!("scenesd: ipc client: {err}");
            }
        });
    }
}

async fn handle_client(stream: UnixStream, shared: Shared) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    let read = reader.read_line(&mut line).await?;
    let response = if read == 0 {
        Response::err("empty request".into())
    } else {
        match serde_json::from_str::<Request>(&line) {
            Err(err) => Response::err(format!("bad request: {err}")),
            Ok(request) => handle_request(request, shared).await,
        }
    };
    let mut text = serde_json::to_string(&response).unwrap_or_else(|_| "{}".into());
    text.push('\n');
    reader.write_all(text.as_bytes()).await?;
    Ok(())
}

async fn handle_request(request: Request, shared: Shared) -> Response {
    let mut manager = shared.lock().await;
    let result = match request.method.as_str() {
        "status" => {
            let state = match manager.state() {
                ManagerState::Active(name) => format!("active:{name}"),
                other => format!("{other:?}"),
            };
            let active = manager.active_scene().unwrap_or("-").to_string();
            Ok(format!("{state} scene={active}"))
        }
        "list" => Ok(manager
            .config()
            .scene
            .keys()
            .cloned()
            .collect::<Vec<_>>()
            .join(" ")),
        "apply" => manager
            .apply_scene(&request.name)
            .map(|_| format!("applied {}", request.name))
            .map_err(|e| e.to_string()),
        "off" => manager
            .revert_active()
            .map(|_| "reverted".to_string())
            .map_err(|e| e.to_string()),
        "pause" => manager
            .pause(request.seconds)
            .map(|_| format!("paused {}s", request.seconds))
            .map_err(|e| e.to_string()),
        "resume" => manager
            .resume()
            .map(|_| "resumed".to_string())
            .map_err(|e| e.to_string()),
        "manual" => manager
            .handle_signal(&Signal::Manual, &Context::default())
            .map(|_| "manual override recorded".to_string())
            .map_err(|e| e.to_string()),
        other => Err(format!("unknown method '{other}'")),
    };
    match result {
        Ok(text) => Response::ok(text),
        Err(text) => Response::err(text),
    }
}

async fn hypr_loop(shared: Shared, debounce_ms: u64, signal_poll_ms: u64) -> Result<(), String> {
    let mut ctx = Context::default();
    let mut ticker = tokio::time::interval(std::time::Duration::from_secs(60));
    let mut poll_ticker =
        tokio::time::interval(std::time::Duration::from_millis(signal_poll_ms.max(1000)));
    loop {
        let socket = match socket2_path() {
            Some(path) if path.exists() => path,
            _ => {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                continue;
            }
        };
        let stream = match tokio::net::UnixStream::connect(&socket).await {
            Ok(stream) => stream,
            Err(err) => {
                eprintln!("scenesd: socket2 connect: {err}");
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                continue;
            }
        };
        let mut lines = BufReader::new(stream).lines();
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    context_snapshot(&mut ctx);
                    let mut manager = shared.lock().await;
                    let _ = manager.handle_signal(&Signal::Tick, &ctx);
                }
                _ = poll_ticker.tick() => {
                    let signals = SystemSignals;
                    let mut changes: Vec<Signal> = Vec::new();
                    if let Some(count) = signals.monitor_count() {
                        if count != ctx.monitor_count { changes.push(Signal::MonitorCount(count)); }
                    }
                    if let Some(ssid) = signals.wifi_ssid() {
                        if ssid != ctx.ssid { changes.push(Signal::Ssid(ssid)); }
                    }
                    if let Some(sink) = signals.audio_sink() {
                        if sink != ctx.audio_sink { changes.push(Signal::AudioSink(sink)); }
                    }
                    if let Some(power) = signals.power() {
                        if power != ctx.power { changes.push(Signal::Power(power)); }
                    }
                    if let Some(level) = signals.battery() {
                        if Some(level) != ctx.battery { changes.push(Signal::Battery(level)); }
                    }
                    let game = signals.game_active();
                    if game != ctx.game_active { changes.push(Signal::Game(game)); }
                    if let Some(cwd) = signals.focused_cwd() {
                        if cwd != ctx.cwd { changes.push(Signal::Cwd(cwd)); }
                    }
                    if !changes.is_empty() {
                        let mut manager = shared.lock().await;
                        for change in changes {
                            ctx.apply(&change);
                            if let Err(err) = manager.handle_signal(&change, &ctx) {
                                eprintln!("scenesd: {err}");
                            }
                        }
                    }
                }
                line = lines.next_line() => {
                    let Some(line) = line.ok().flatten() else { break };
                    let event = events::parse(&line);
                    if matches!(event, HyprEvent::Other { .. }) {
                        continue;
                    }
                    ctx.apply(&Signal::Hypr(event.clone()));
                    if debounce_ms > 0 {
                        tokio::time::sleep(std::time::Duration::from_millis(debounce_ms)).await;
                    }
                    let mut manager = shared.lock().await;
                    if let Err(err) = manager.handle_signal(&Signal::Hypr(event), &ctx) {
                        eprintln!("scenesd: {err}");
                    }
                }
            }
        }
        eprintln!("scenesd: socket2 closed; reconnecting");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_request_json() {
        let request: Request =
            serde_json::from_str(r#"{"method":"apply","name":"focus","seconds":0}"#)
                .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(request.method, "apply");
        assert_eq!(request.name, "focus");
    }
}

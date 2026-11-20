use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Serialize)]
struct Request {
    method: String,
    name: String,
    seconds: u64,
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

fn ipc_path() -> PathBuf {
    std::env::var_os("SCENES_IPC_SOCKET")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("XDG_STATE_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    std::env::var_os("HOME")
                        .map(|h| PathBuf::from(h).join(".local/state"))
                        .unwrap_or_else(|| PathBuf::from("/tmp"))
                })
                .join("omarchy-scenes/scenes.sock")
        })
}

fn parse_seconds(text: &str) -> u64 {
    let text = text.trim();
    if let Some(value) = text.strip_suffix('m') {
        value.trim().parse::<u64>().unwrap_or(30) * 60
    } else if let Some(value) = text.strip_suffix('h') {
        value.trim().parse::<u64>().unwrap_or(1) * 3600
    } else {
        text.parse::<u64>().unwrap_or(1800)
    }
}

fn usage() {
    println!(
        "scenes — context-aware desktop profiles\n\n\
         Usage: scenes <command> [args]\n\n\
         Commands:\n\
           list                 List configured scenes\n\
           status               Show the active scene and manager state\n\
           apply <name>         Apply a scene manually\n\
           off                  Leave the active scene and restore state\n\
           pause [duration]     Pause auto-switching (e.g. 30m, 1800)\n\
           resume               Resume auto-switching\n\
           check                Validate scenes.toml without the daemon\n\
           why | stats          Planned for Phase 4\n"
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(command) = args.first() else {
        usage();
        return;
    };

    if command == "check" {
        match scenes_core::config::Config::load(&config_path()) {
            Ok(config) => println!(
                "config OK: {} scenes, debounce {}ms",
                config.scene.len(),
                config.settings.debounce_ms
            ),
            Err(err) => {
                eprintln!("{err}");
                std::process::exit(1);
            }
        }
        return;
    }
    if command == "list" {
        match scenes_core::config::Config::load(&config_path()) {
            Ok(config) => {
                for name in config.scene.keys() {
                    println!("{name}");
                }
            }
            Err(err) => {
                eprintln!("{err}");
                std::process::exit(1);
            }
        }
        return;
    }
    if matches!(command.as_str(), "why" | "stats" | "suggest") {
        println!("{command} is planned for Phase 4");
        return;
    }

    let method = match command.as_str() {
        "status" | "apply" | "off" | "pause" | "resume" => command.clone(),
        _ => {
            usage();
            std::process::exit(2);
        }
    };
    let request = Request {
        method,
        name: args.get(1).cloned().unwrap_or_default(),
        seconds: args.get(1).map(|s| parse_seconds(s)).unwrap_or(1800),
    };
    let mut stream = match UnixStream::connect(ipc_path()) {
        Ok(stream) => stream,
        Err(err) => {
            eprintln!(
                "scenes: cannot reach daemon at {}: {err}",
                ipc_path().display()
            );
            eprintln!("start it with: systemctl --user start scenesd.service");
            std::process::exit(1);
        }
    };
    let mut line = serde_json::to_string(&request).unwrap_or_else(|_| "{}".into());
    line.push('\n');
    if let Err(err) = stream.write_all(line.as_bytes()) {
        eprintln!("scenes: {err}");
        std::process::exit(1);
    }
    let mut response = String::new();
    if let Err(err) = BufReader::new(stream).read_line(&mut response) {
        eprintln!("scenes: {err}");
        std::process::exit(1);
    }
    match serde_json::from_str::<serde_json::Value>(&response) {
        Ok(value) => {
            if let Some(result) = value.get("result").and_then(|v| v.as_str()) {
                println!("{result}");
            }
            if let Some(error) = value.get("error").and_then(|v| v.as_str()) {
                eprintln!("scenes: {error}");
                std::process::exit(1);
            }
        }
        Err(err) => {
            eprintln!("scenes: bad response: {err}");
            std::process::exit(1);
        }
    }
}

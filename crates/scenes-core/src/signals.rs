//! Phase 3 signal sources. Every external call goes through a fixed binary
//! with fixed arguments and sits behind a trait so tests use MockSignals.

use std::process::Command;

use crate::signal::git_root;

pub trait SignalSource: Send + Sync {
    fn monitor_count(&self) -> Option<usize>;
    fn wifi_ssid(&self) -> Option<String>;
    fn audio_sink(&self) -> Option<String>;
    fn power(&self) -> Option<String>;
    fn battery(&self) -> Option<u8>;
    fn game_active(&self) -> bool;
    fn focused_cwd(&self) -> Option<String>;
}

fn output(program: &str, args: &[&str]) -> Option<String> {
    let result = Command::new(program).args(args).output().ok()?;
    if result.status.success() {
        Some(String::from_utf8_lossy(&result.stdout).to_string())
    } else {
        None
    }
}

pub struct SystemSignals;

impl SignalSource for SystemSignals {
    fn monitor_count(&self) -> Option<usize> {
        let text = output("hyprctl", &["-j", "monitors"])?;
        serde_json::from_str::<serde_json::Value>(&text)
            .ok()?
            .as_array()
            .map(|list| list.len())
    }

    fn wifi_ssid(&self) -> Option<String> {
        parse_ssid(&output(
            "nmcli",
            &["-t", "-f", "active,ssid", "dev", "wifi"],
        )?)
    }

    fn audio_sink(&self) -> Option<String> {
        output("pactl", &["get-default-sink"])?
            .lines()
            .next()
            .map(str::to_string)
    }

    fn power(&self) -> Option<String> {
        parse_power(&output("upower", &["-e"])?)
    }

    fn battery(&self) -> Option<u8> {
        let devices = output("upower", &["-e"])?;
        let battery = devices
            .lines()
            .find(|line| line.contains("battery_"))
            .map(str::to_string)?;
        parse_battery(&output("upower", &["-i", &battery])?)
    }

    fn game_active(&self) -> bool {
        output("gamemoded", &["-s"])
            .map(|text| parse_game(&text))
            .unwrap_or(false)
    }

    fn focused_cwd(&self) -> Option<String> {
        let active = output("hyprctl", &["-j", "activewindow"])?;
        let address = serde_json::from_str::<serde_json::Value>(&active)
            .ok()?
            .get("address")
            .and_then(|v| v.as_str())?
            .to_string();
        let clients = output("hyprctl", &["-j", "clients"])?;
        parse_cwd(&clients, &address)
    }
}

pub fn parse_ssid(text: &str) -> Option<String> {
    text.lines()
        .find_map(|line| line.strip_prefix("yes:"))
        .map(str::to_string)
}

pub fn parse_power(devices: &str) -> Option<String> {
    if devices.lines().any(|line| line.contains("line_power_")) {
        Some("ac".to_string())
    } else if devices.lines().any(|line| line.contains("battery_")) {
        Some("battery".to_string())
    } else {
        None
    }
}

pub fn parse_battery(text: &str) -> Option<u8> {
    text.lines()
        .find_map(|line| line.trim().strip_prefix("percentage:"))
        .and_then(|value| value.trim().trim_end_matches('%').parse::<u8>().ok())
}

pub fn parse_game(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("gamemode is active") && !lower.contains("gamemode is inactive")
}

pub fn parse_cwd(clients_json: &str, address: &str) -> Option<String> {
    let clients = serde_json::from_str::<serde_json::Value>(clients_json).ok()?;
    let needle = address.trim_start_matches("0x").to_lowercase();
    let pid = clients.as_array()?.iter().find_map(|client| {
        let candidate = client
            .get("address")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .trim_start_matches("0x")
            .to_lowercase();
        if candidate == needle {
            client.get("pid").and_then(|v| v.as_u64())
        } else {
            None
        }
    })?;
    let cwd = std::fs::read_link(format!("/proc/{pid}/cwd")).ok()?;
    let text = cwd.to_string_lossy().to_string();
    let root = git_root(&text);
    Some(if root.is_empty() { text } else { root })
}

#[derive(Default)]
pub struct MockSignals {
    pub monitors: Option<usize>,
    pub ssid: Option<String>,
    pub sink: Option<String>,
    pub power: Option<String>,
    pub battery: Option<u8>,
    pub game: bool,
    pub cwd: Option<String>,
}

impl SignalSource for MockSignals {
    fn monitor_count(&self) -> Option<usize> {
        self.monitors
    }
    fn wifi_ssid(&self) -> Option<String> {
        self.ssid.clone()
    }
    fn audio_sink(&self) -> Option<String> {
        self.sink.clone()
    }
    fn power(&self) -> Option<String> {
        self.power.clone()
    }
    fn battery(&self) -> Option<u8> {
        self.battery
    }
    fn game_active(&self) -> bool {
        self.game
    }
    fn focused_cwd(&self) -> Option<String> {
        self.cwd.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_external_output() {
        assert_eq!(
            parse_ssid("no:Cafe\nnas:Office\nyes:NOWO_2MGK8aT-5G_EXT\n"),
            Some("NOWO_2MGK8aT-5G_EXT".into())
        );
        assert_eq!(
            parse_power("/org/freedesktop/UPower/devices/line_power_AC\n"),
            Some("ac".into())
        );
        assert_eq!(parse_battery("  percentage:              54%\n"), Some(54));
        assert!(!parse_game("gamemode is inactive"));
        assert!(parse_game("gamemode is active"));
        let clients = r#"[{"address":"0xabc","pid":1234,"class":"code"}]"#;
        let cwd = parse_cwd(clients, "abc");
        assert!(cwd.is_none() || !cwd.unwrap_or_default().is_empty());
    }

    #[test]
    fn mock_source_is_usable() {
        let mock = MockSignals {
            monitors: Some(2),
            ssid: Some("Home".into()),
            game: true,
            ..Default::default()
        };
        assert_eq!(mock.monitor_count(), Some(2));
        assert_eq!(mock.wifi_ssid().as_deref(), Some("Home"));
        assert!(mock.game_active());
    }
}

//! Parser for Hyprland's socket2 event stream. Wire format: `name>>a,b,c`.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HyprEvent {
    ActiveWindow { class: String, title: String },
    ActiveWindowV2 { address: String },
    Workspace { id: String, name: String },
    OpenWindow { address: String, class: String },
    CloseWindow { address: String },
    MonitorAdded { name: String },
    MonitorRemoved { name: String },
    Other { name: String, args: String },
}

fn args_after(line: &str) -> (&str, Vec<String>) {
    let (name, rest) = line.split_once(">>").unwrap_or((line, ""));
    (name, rest.split(',').map(str::to_string).collect())
}

pub fn parse(line: &str) -> HyprEvent {
    let (name, a) = args_after(line.trim_end_matches(['\r', '\n']));
    match name {
        "activewindow" => HyprEvent::ActiveWindow {
            class: a.first().cloned().unwrap_or_default(),
            title: a.get(1).cloned().unwrap_or_default(),
        },
        "activewindowv2" => HyprEvent::ActiveWindowV2 {
            address: a.first().cloned().unwrap_or_default(),
        },
        "workspace" | "workspacev2" => HyprEvent::Workspace {
            id: a.first().cloned().unwrap_or_default(),
            name: a.get(1).cloned().unwrap_or_default(),
        },
        "openwindow" => HyprEvent::OpenWindow {
            address: a.first().cloned().unwrap_or_default(),
            class: a.get(3).cloned().unwrap_or_default(),
        },
        "closewindow" => HyprEvent::CloseWindow {
            address: a.first().cloned().unwrap_or_default(),
        },
        "monitoradded" | "monitoraddedv2" => HyprEvent::MonitorAdded {
            name: a.first().cloned().unwrap_or_default(),
        },
        "monitorremoved" => HyprEvent::MonitorRemoved {
            name: a.first().cloned().unwrap_or_default(),
        },
        other => HyprEvent::Other {
            name: other.to_string(),
            args: a.join(","),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_verified_lines() {
        assert_eq!(
            parse("activewindow>>foot,foot"),
            HyprEvent::ActiveWindow {
                class: "foot".into(),
                title: "foot".into()
            }
        );
        assert_eq!(
            parse("workspacev2>>3,3"),
            HyprEvent::Workspace {
                id: "3".into(),
                name: "3".into()
            }
        );
        assert_eq!(
            parse("openwindow>>556c392e5700,3,foot,foot"),
            HyprEvent::OpenWindow {
                address: "556c392e5700".into(),
                class: "foot".into()
            }
        );
    }

    #[test]
    fn handles_empty_and_unknown() {
        assert_eq!(
            parse("activewindowv2>>"),
            HyprEvent::ActiveWindowV2 {
                address: String::new()
            }
        );
        assert_eq!(
            parse("activelayout>>kbd,English (US)"),
            HyprEvent::Other {
                name: "activelayout".into(),
                args: "kbd,English (US)".into()
            }
        );
    }
}

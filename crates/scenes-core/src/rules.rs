use crate::config::{Config, Scene, When, glob_match};
use crate::signal::Context;

fn any_match(patterns: &[String], value: &str) -> bool {
    let lower = value.to_lowercase();
    patterns
        .iter()
        .any(|p| glob_match(&p.to_lowercase(), &lower))
}

fn time_range_matches(range: &str, minute: u32) -> bool {
    let (from, to) = match range.split_once('-') {
        Some(pair) => pair,
        None => return false,
    };
    let parse = |s: &str| -> Option<u32> {
        let (h, m) = s.trim().split_once(':')?;
        Some(h.parse::<u32>().ok()?.checked_mul(60)? + m.parse::<u32>().ok()?)
    };
    let (start, end) = match (parse(from), parse(to)) {
        (Some(a), Some(b)) => (a, b),
        _ => return false,
    };
    if start <= end {
        minute >= start && minute < end
    } else {
        minute >= start || minute < end
    }
}

pub fn when_matches(when: &When, ctx: &Context) -> bool {
    if !when.window_class.is_empty() && !any_match(&when.window_class, &ctx.window_class) {
        return false;
    }
    if !when.window_title.is_empty() && !any_match(&when.window_title, &ctx.window_title) {
        return false;
    }
    if !when.workspace.is_empty() && !any_match(&when.workspace, &ctx.workspace) {
        return false;
    }
    if !when.monitor_name.is_empty() && !any_match(&when.monitor_name, &ctx.monitor_name) {
        return false;
    }
    if !when.git_repo.is_empty() && !any_match(&when.git_repo, &ctx.git_repo) {
        return false;
    }
    if !when.cwd.is_empty() && !any_match(&when.cwd, &ctx.cwd) {
        return false;
    }
    if !when.ssid.is_empty() && !any_match(&when.ssid, &ctx.ssid) {
        return false;
    }
    if !when.audio_sink.is_empty() && !any_match(&when.audio_sink, &ctx.audio_sink) {
        return false;
    }
    if let Some(min) = when.monitors_min
        && ctx.monitor_count < min
    {
        return false;
    }
    if let Some(max) = when.monitors_max
        && ctx.monitor_count > max
    {
        return false;
    }
    if let Some(wanted) = when.game
        && ctx.game_active != wanted
    {
        return false;
    }
    if let Some(level) = when.battery_below
        && ctx.battery.is_none_or(|b| b >= level)
    {
        return false;
    }
    if let Some(power) = &when.power
        && !ctx.power.eq_ignore_ascii_case(power)
    {
        return false;
    }
    if let Some(range) = &when.time_range
        && !time_range_matches(range, ctx.minute_of_day)
    {
        return false;
    }
    true
}

/// Highest priority wins; ties go to the last matching scene in config order.
pub fn best_match<'a>(config: &'a Config, ctx: &Context) -> Option<(&'a str, &'a Scene)> {
    let mut best: Option<(&str, &Scene)> = None;
    for (name, scene) in &config.scene {
        if !scene.when.iter().any(|when| when_matches(when, ctx)) {
            continue;
        }
        match best {
            None => best = Some((name, scene)),
            Some((_, current)) if scene.priority >= current.priority => best = Some((name, scene)),
            Some(_) => {}
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ActionSpec, When};
    use std::collections::BTreeMap;

    fn scene(priority: i32, when: When) -> Scene {
        Scene {
            priority,
            description: String::new(),
            when: vec![when],
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
        }
    }

    #[test]
    fn and_or_and_priority() {
        let mut config = Config {
            settings: Default::default(),
            scene: BTreeMap::new(),
        };
        config.scene.insert(
            "focus".into(),
            scene(
                50,
                When {
                    window_class: vec!["code".into()],
                    workspace: vec!["1".into()],
                    ..Default::default()
                },
            ),
        );
        config.scene.insert(
            "game".into(),
            scene(
                80,
                When {
                    window_class: vec!["steam_app_*".into()],
                    ..Default::default()
                },
            ),
        );
        let mut ctx = Context {
            window_class: "code".into(),
            workspace: "1".into(),
            ..Default::default()
        };
        assert_eq!(best_match(&config, &ctx).map(|(n, _)| n), Some("focus"));
        ctx.window_class = "steam_app_123".into();
        assert_eq!(best_match(&config, &ctx).map(|(n, _)| n), Some("game"));
        ctx.workspace = "9".into();
        assert_eq!(best_match(&config, &ctx).map(|(n, _)| n), Some("game"));
    }

    #[test]
    fn monitors_and_game_conditions() {
        let when = When {
            monitors_max: Some(1),
            game: Some(true),
            ..Default::default()
        };
        let ctx = Context {
            monitor_count: 2,
            game_active: true,
            ..Default::default()
        };
        assert!(!when_matches(&when, &ctx));
        let ctx = Context {
            monitor_count: 1,
            game_active: true,
            ..Default::default()
        };
        assert!(when_matches(&when, &ctx));
    }

    #[test]
    fn time_range_wraps_midnight() {
        assert!(time_range_matches("22:00-06:00", 23 * 60));
        assert!(time_range_matches("22:00-06:00", 60));
        assert!(!time_range_matches("22:00-06:00", 12 * 60));
    }
}

//! Hot-path benchmarks. Ignored by default so `cargo test` stays fast:
//!   cargo test --release -p scenes-core --test perf -- --ignored --nocapture

use std::hint::black_box;
use std::time::Instant;

use scenes_core::config::Config;
use scenes_core::events;
use scenes_core::rules::best_match;
use scenes_core::signal::Context;

fn config_text() -> String {
    let mut text = String::from("[settings]\nsignal_poll_ms = 5000\n\n");
    for i in 0..20 {
        text.push_str(&format!(
            "[scene.s{i}]\npriority = {}\n\n  [[scene.s{i}.when]]\n  window_class = [\"code*\", \"kitty\"]\n  workspace = [\"1\", \"2\", \"3\"]\n\n  [[scene.s{i}.do]]\nkind = \"omarchy\"\nrun = \"theme set Nord\"\n\n",
            i + 1
        ));
    }
    text
}

#[test]
#[ignore = "benchmark"]
fn event_parse_and_rule_match() {
    const N: usize = 100_000;
    let lines: Vec<String> = (0..N)
        .map(|i| format!("activewindow>>code-{i},main.rs"))
        .collect();

    let start = Instant::now();
    for line in &lines {
        let _ = black_box(events::parse(line));
    }
    let parse = start.elapsed();

    let config: Config = toml::from_str(&config_text()).unwrap_or_else(|e| panic!("{e}"));
    config.validate().unwrap_or_else(|e| panic!("{e}"));
    let ctx = Context {
        window_class: "code-42".into(),
        workspace: "2".into(),
        monitor_count: 2,
        ..Default::default()
    };
    let start = Instant::now();
    for _ in 0..N {
        let _ = black_box(best_match(&config, &ctx));
    }
    let matching = start.elapsed();

    println!(
        "event parse:  {N} lines in {parse:?} ({:.1?}/line)",
        parse / N as u32
    );
    println!(
        "rule match:   {N} evaluations in {matching:?} ({:.1?}/eval)",
        matching / N as u32
    );
    println!("scenes in config: {}", config.scene.len());
}

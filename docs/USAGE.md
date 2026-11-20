# Usage

## Signals

Hyprland socket2 provides `activewindowv2`, `workspacev2`, `openwindow`,
`closewindow`, `monitoradded`/`monitorremoved` and related events. Scenes also
polls, every `signal_poll_ms` (default 5000):

| Signal | Source |
|---|---|
| monitor count | `hyprctl -j monitors` |
| git repo / cwd | focused window PID → `/proc/<pid>/cwd` → nearest `.git` |
| power / battery | `upower -e`, `upower -i` |
| game | `gamemoded -s` |
| audio sink | `pactl get-default-sink` |
| Wi-Fi SSID | `nmcli -t -f active,ssid dev wifi` |

`scenes signals` prints the values this machine currently reports.

## Rules

```toml
[scene.focus]
priority = 50

  [[scene.focus.when]]
  window_class = ["code", "kitty"]   # OR within one field
  workspace = ["1"]                  # AND across fields in one block

  [[scene.focus.when]]
  git_repo = "~/dev/*"               # a second block: OR with the first
```

Globs support `*` and `?`. Other conditions: `window_title`, `monitor_name`,
`cwd`, `ssid`, `audio_sink`, `monitors_min`, `monitors_max`, `battery_below`,
`power`, `time_range` (`"22:00-06:00"` wraps midnight) and `game`.

## Actions

| kind | required field | snapshot / revert |
|---|---|---|
| `omarchy` | `run` | theme slug, wallpaper, bar background, bar visibility |
| `hypr` | `keyword`, `value` | previous keyword value via `hyprctl -j getoption` |
| `notify` | `dnd` | DND state from `quiet.json` |
| `power`, `audio` | `profile`, `sink` | reserved for future phases |
| `exec` | `command`, `irreversible = true` | no automatic revert |

## Recovery

The manager writes `state.json` after every action. On startup, any snapshot
found there is reverted before normal operation, so a `kill -9` cannot leave a
half-applied scene behind. Pausing state is persisted too.

Robustness notes: a second daemon refuses to start if one already owns the
socket, IPC requests are capped at 64 KiB, extended signal probes run on worker
threads, and scene-pack manifests/files are size-limited with `scenes_file`
restricted to a plain `.toml` filename.

The event log is `~/.local/state/omarchy-scenes/events.db`. `scenes why`,
`scenes stats` and `scenes suggest` read it directly; no daemon request needed.
Both signal decisions and manual `scenes apply` calls are recorded.

Hot paths have an ignored benchmark:
`cargo test --release -p scenes-core --test perf -- --ignored --nocapture`.

# Phase 0 — Recon results

Date: 2026-11-20. Host: Arch + Omarchy, Hyprland **0.56.2**, dual monitor
(`HDMI-A-1` 3440x1440@100, `HDMI-A-2` 1920x1080@75). All checks below were read-only
against the live install, except the documented benign workspace/focus changes used to
capture Hyprland events (original workspace restored).

---

## 1. Paths and ownership

| Purpose | Verified location | One-liner |
|---|---|---|
| Omarchy install (read-only reference) | `$OMARCHY_PATH` = `/usr/share/omarchy` | `echo "$OMARCHY_PATH" && ls "$OMARCHY_PATH"` |
| Non-existent path in the original brief | `~/.local/share/omarchy` — **does not exist** | `[ -d ~/.local/share/omarchy ] && echo yes || echo no` |
| User config (safe) | `~/.config/omarchy/**`, `~/.config/hypr/**` | `ls ~/.config/omarchy` |
| Own runtime state (new) | `~/.local/state/omarchy-scenes/` | `mkdir -p ~/.local/state/omarchy-scenes` (Phase 1) |
| Current theme name (slug) | `~/.local/state/omarchy/current/theme.name` (e.g. `command-deck`) | `cat ~/.local/state/omarchy/current/theme.name` |
| Current theme (display name) | via CLI | `omarchy theme current` → `Command Deck` |
| Full command inventory | `docs/commands.txt` (425 routes) | `omarchy commands --json \| python3 -c 'import json,sys;[print(c["route"]) for c in json.load(sys.stdin)["commands"]]'` |

**Important:** `omarchy commands --json` returns an **object** with `ok` and `commands`, not a list.

---

## 2. Integration points — exact commands and one-liners

### Theme (snapshot + revert)
- `omarchy theme set <name>` applies a theme; `omarchy theme list` lists them (e.g. **`Tokyo Night`**).
- `omarchy theme current` prints the **display** name; the slug is in `current/theme.name`.
- Side effects: rewrites themed configs and backgrounds, restarts terminals/shell, and resets
  bar/background styling to theme defaults. We observed a theme switch resetting
  `shell.json` bar background — **snapshot bar background too**.
- One-liners:
  ```bash
  omarchy theme set "Tokyo Night"
  omarchy theme current                                  # display name
  cat ~/.local/state/omarchy/current/theme.name          # slug for revert
  omarchy theme bg current                               # current wallpaper name
  omarchy bar background                                 # opaque | glassy | none
  ```
- Snapshot: theme slug + `omarchy theme bg current` + `omarchy bar background`.
- Revert: `omarchy theme set <slug>` then restore background/bar values if needed.
- Manual-override hook: `omarchy-hook theme-set <slug>` is called by `omarchy-theme-set`
  (source: `$(which omarchy-theme-set)`, line ~603). Install a watcher in
  `~/.config/omarchy/hooks/theme-set.d/`.

### Bar visibility (gaming scene)
- **No waybar in this install.** The bar is Quickshell.
- `omarchy toggle bar [toggle|on|off]` — help explicitly says **`on` hides the bar, `off` shows it**.
- One-liner:
  ```bash
  omarchy toggle bar on     # hide
  omarchy toggle bar off    # show (revert)
  ```

### Notifications / DND
- `omarchy notification send ...` (full arg list in `omarchy commands`).
- Two-word toggles have dedicated binaries: `omarchy-toggle-notification-silencing` **ignores
  extra args and always toggles**. The reliable DND route is shell IPC:
  ```bash
  omarchy-shell notifications toggleDnd
  omarchy-shell -q omarchy.indicators refresh
  cat ~/.local/state/omarchy/notifications/quiet.json     # {"dnd":false}
  ```
- `omarchy toggle enabled <single-token-flag>` checks a flag file (behaviour from
  `$(which omarchy-toggle)`).

### Menu extension
- File: `~/.config/omarchy/extensions/omarchy-menu.jsonc` (JSONC, comments allowed).
- Schema (from the file header): object keys are **ids**; dotted ids infer submenus; fields:
  `icon`, `label`, `action`, `target`, `provider`, `aliases`, `description`, `when`, `checked`.
- Invoke: `omarchy menu summon <route>` / `omarchy menu toggle <route>`, refresh with
  `omarchy menu refresh`.
- One-liner:
  ```bash
  omarchy menu summon appearance.theme
  ```
- Scenes Phase 2 can add a top-level `"scenes"` key with dotted children and summon by route.

### Hooks
- `omarchy hook install <type> <file>` installs into `~/.config/omarchy/hooks/<type>.d/`.
- `omarchy-hook <name> [args...]` runs `~/.config/omarchy/hooks/<name>` then every executable-ish
  file in `<name>.d/` via `bash`, passing args. `.sample` files are skipped.
- Confirmed `.d` dirs: `battery-low.d`, `font-set.d`, `post-boot.d`, `post-update.d`,
  `pre-refresh-pacman.d`, `theme-set.d`.
- Theme hook receives the slug: `omarchy-hook theme-set "$THEME_NAME"`.
- One-liner:
  ```bash
  omarchy hook install theme-set ./hooks/theme-set.d/50-scenes-override
  ```

### Bar widgets / shell plugins
- User plugins: `~/.config/omarchy/plugins/<plugin-id>/manifest.json` plus QML entry points.
  Manifest fields observed: `schemaVersion`, `id`, `name`, `version`, `author`, `description`,
  `kinds` (`bar-widget`), `entryPoints.barWidget`, `barWidget.displayName/category/allowMultiple`.
- Layout: `~/.config/omarchy/shell.json` → `bar.layout` sections
  `left`, `centerBegin`, `center`, `centerEnd`, `right`, `tray`.
- Commands: `omarchy plugin list|enable|disable|clone|validate`, `omarchy restart shell`,
  `omarchy-shell shell rescanPlugins`.
- One-liner:
  ```bash
  omarchy plugin validate ~/.config/omarchy/plugins/guilhermesales.commanddeck
  omarchy restart shell
  ```

### Hyprland IPC — socket2 events
- Socket directory: `${XDG_RUNTIME_DIR:-/run/user/1000}/hypr/<hash>/`.
- Event stream: `.socket2.sock` (read-only broadcast); command socket: `.socket.sock`.
- One-liner to locate (wildcard is required — hash changes per session):
  ```bash
  ls "${XDG_RUNTIME_DIR:-/run/user/1000}"/hypr/*/.socket2.sock
  ```
- Verified event names (captured live while opening/closing a foot window and switching workspace):
  ```
  windowtitle, windowtitlev2, createworkspace, createworkspacev2,
  activewindow, activewindowv2, workspace, workspacev2, openwindow,
  activelayout, closewindow, destroyworkspace, destroyworkspacev2
  ```
- Wire format: `event>>arg1,arg2`, newline-delimited. Examples:
  ```
  activewindowv2>>556c392e5700
  workspacev2>>3,3
  openwindow>>556c392e5700,3,foot,foot
  closewindow>>556c392e5700
  ```
- Note: dispatching a workspace switch with no window change produced no events in two of three
  attempts; the reliable capture included a real window open/close. Tests must use a mock
  socket with hand-written lines like the above.
- Full Hyprland docs URL used by this install for rules: `https://wiki.hypr.land/configuring/core/rules/window-rules/`.

### Focused window → git repo / cwd (Phase 3)
- `hyprctl clients -j` gives `address`, `class`, `pid` per window.
- `/proc/<pid>/cwd` gives the working directory; git root can be derived from it.
- Verified sample:
  ```
  com.mitchellh.ghostty addr=0x556c3ad26710 pid=861482 cwd=/home/guilhermesales
  ```
- One-liner:
  ```bash
  hyprctl clients -j | python3 -c 'import json,sys,os; c=json.load(sys.stdin)[0]; print(c["address"], c["class"], os.readlink("/proc/%d/cwd" % c["pid"]))'
  ```

### Power / game / audio / network signals (Phase 3)
| Signal | Tool | One-liner |
|---|---|---|
| Battery / AC | `upower` | `upower -e` → here only `DisplayDevice` (desktop, no battery) |
| GameMode | `gamemoded` | `gamemoded -s` → `gamemode is inactive` |
| Default audio sink | `pactl` | `pactl get-default-sink` |
| Wi-Fi SSID | `nmcli` | `nmcli -t -f active,ssid dev wifi` → `yes:NOWO_2MGK8aT-5G_EXT` |

`iwgetid` is **not** installed; use `nmcli`.

### Systemd user service
- User unit dir exists: `~/.config/systemd/user/` (already has several user units).
- Omarchy ships examples under `/usr/share/omarchy/default/systemd/user/` (read-only).
- One-liner: `systemctl --user status <unit>` (verification in Phase 1).

---

## 3. Wrong assumptions in the original roadmap

1. **`~/.local/share/omarchy/`** — does not exist. Use `$OMARCHY_PATH` = `/usr/share/omarchy` as the read-only reference.
2. **`omarchy toggle waybar`** — no waybar. Use `omarchy toggle bar [toggle|on|off]`, with inverted semantics (`on` hides, `off` shows).
3. **`omarchy commands --json` shape** — object with `ok`/`commands`, not a bare array.
4. **Hook invocation detail** — the theme-set hook is called as `omarchy-hook theme-set <slug>`; hooks are `.d` directories of `bash` scripts, and `.sample` files are skipped.
5. **SSID source** — `iwgetid` absent; `nmcli` is the available tool.
6. **Battery signal** — this is a desktop with no battery device; `battery_below` will be inert here unless a UPS/battery appears.
7. **Bar background is part of theme state** — a theme change resets shell.json bar styling, so scene snapshots must include it.

---

## 4. Phase 0 acceptance criterion

Per integration point above: exact command/file **and** a working one-liner. ✅
`docs/commands.txt` holds all 425 discovered `omarchy` routes for later lookup.
Stop here: no feature code, no new dependencies, no runtime state created.

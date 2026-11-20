# Omarchy Scenes

Context-aware desktop profiles for Omarchy. Scenes watches Hyprland events and
system signals, applies a named profile when a rule matches, and restores the
previous state when the context ends.

> Status: Phases 0–5 of `ROADMAP.md` are implemented. The daemon is intentionally
> conservative: no external network calls, no shell injection, and scene packs
> require an explicit `--confirm` after showing a permission summary.

## Install

```bash
git clone https://github.com/kill74/omarchy-scenes.git
cd omarchy-scenes
./install.sh
```

`install.sh` is idempotent. It builds the binaries, copies the default config only
when none exists, installs the user service and hooks, and can be removed with
`./install.sh --uninstall` (add `--purge-config` to delete config too).

For testing without touching systemd:

```bash
SCENES_SKIP_BUILD=1 SCENES_BIN_DIR="$PWD/target/debug" SCENES_SKIP_SYSTEMD=1 ./install.sh
```

## Quick start

```bash
scenes check              # validate ~/.config/omarchy/scenes/scenes.toml
scenes list               # list scenes
scenes status             # active scene and manager state
scenes apply focus        # apply one manually
scenes off                # leave and restore
scenes pause 30m          # pause auto-switching
scenes resume
scenes signals            # print current extended signals
scenes why                # last decision from the event log
scenes stats              # time spent per scene
scenes suggest            # draft scenes from observed decisions (never applied)
scenes pack inspect examples/focus-pack/scene-pack.toml
```

The Omarchy menu has a **Scenes** submenu, and the bar has a small status widget
that shows the active scene or a paused marker.

## Config

Config lives at `~/.config/omarchy/scenes/scenes.toml`. See
[`config/scenes.toml`](config/scenes.toml) for a complete example. Each
`[[scene.NAME.when]]` block is ANDed within itself; multiple blocks are ORed.
The highest `priority` wins. Every action records a snapshot so it can be
reverted.

Safety notes:
- Actions run fixed binaries (`omarchy`, `hyprctl`, `pactl`), never a shell.
- `exec` actions are validated by `scenes check` and must declare
  `irreversible = true`.
- A manual theme change pauses auto-switching for `manual_override_pause`.

More detail: [`docs/USAGE.md`](docs/USAGE.md),
[`docs/SCENE_PACKS.md`](docs/SCENE_PACKS.md), [`docs/RECON.md`](docs/RECON.md).

## Status of roadmap items

- Phases 0–4: complete and verified locally.
- Phase 5: packs, AUR PKGBUILD, CI and docs included.
  - The "suggest via OpenCode API" step is a local draft generator only; no
    external dependency or API call was added.
  - Omarchy plugin-marketplace submission is not included: Phase 0 confirmed
    the bar-widget plugin manifest, not the marketplace submission format.
  - No demo GIF is committed.

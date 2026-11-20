# Changelog

All notable changes to this project are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1] - 2026-11-20

### Fixed

- Scene-pack permissions are derived from actual scene actions, and installs are
  bound to the reviewed git revision with `--rev`.
- Menu uninstall preserves separators between surviving entries and validates
  JSONC before writing.

### Added

- `scenes log clear` removes local decision history.

## [0.1.0] - 2026-11-20

### Added

- Phase 0 recon with verified Omarchy commands and Hyprland socket2 events.
- Phase 1 Rust daemon, CLI, rule engine, snapshot/revert and crash recovery.
- Phase 2 Omarchy integration: menu entries, bar widget, hooks and notifications.
- Phase 3 extended signals: git/cwd, monitors, power/battery, GameMode, audio sink and Wi-Fi SSID.
- Phase 4 SQLite event log with `scenes why`, `scenes stats` and dry-run `scenes suggest`.
- Phase 5 scene packs, AUR PKGBUILD, CI and documentation.
- Bar widget plugin `io.github.kill74.scenes`, MIT license and marketplace preview.

### Changed

- Hardened IPC with a 64 KiB request limit and duplicate-daemon detection.
- Moved blocking signal probes onto worker threads.
- Restricted scene-pack `scenes_file` to a plain `.toml` filename with size limits.
- Documented `run` commands now accept both `theme set X` and `omarchy theme set X`.

### Fixed

- Scene-pack `scenes_file` was silently absorbed into a TOML table; both placements now resolve.
- `scenes why` now records manual `scenes apply` decisions.
- Bar widget path mismatch after the namespaced plugin ID change.

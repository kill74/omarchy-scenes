# Contributing

Thanks for helping improve Omarchy Scenes.

## Development setup

```bash
git clone https://github.com/kill74/omarchy-scenes.git
cd omarchy-scenes
cargo build --workspace
cargo test --workspace
```

The project is a Cargo workspace with three crates:

- `scenes-core` — config, rules, state machine, signals, actions, event log, packs
- `scenesd` — Hyprland socket2 listener, IPC server and signal polling
- `scenes` — CLI

## Before opening a pull request

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
omarchy plugin validate .
```

If you touch the bar widget, also run `qmllint -I "$OMARCHY_PATH/shell"
assets/scenes.status/Status.qml` where Qt tooling is available.

## Guidelines

- Keep changes small and focused; one behaviour per commit.
- Use Conventional Commits (`feat:`, `fix:`, `docs:`, `test:`, `refactor:`).
- Every new behaviour needs a test. Hyprland and `omarchy` calls go through
  traits so they can be mocked.
- No `unwrap()` outside tests; use typed errors with actionable messages.
- Ask before adding a dependency, any `exec` action, or writing outside
  `~/.config/omarchy/**` and `~/.local/state/omarchy-scenes/`.
- Never commit secrets, tokens, `.env` files or generated `target/` output.
- Update `README.md`, `docs/` and `CHANGELOG.md` when behaviour changes.

## Reporting bugs

Use the issue templates and include `scenes status`, `scenes why` and the
relevant `journalctl --user -u scenesd.service` output.

# Omarchy marketplace packaging

Phase 0 confirmed the **bar-widget plugin manifest** format used under
`~/.config/omarchy/plugins/<id>/manifest.json` (`schemaVersion`, `id`, `kinds`,
`entryPoints`, `barWidget`). It did **not** confirm a marketplace submission or
distribution format for a systemd daemon package.

Therefore no marketplace submission manifest is included. The Omarchy-native
parts that are confirmed are installed by `install.sh`:

- `scenes.status` bar widget plugin (manifest + QML)
- Scenes entries in `~/.config/omarchy/extensions/omarchy-menu.jsonc`
- theme-set and post-update hooks

If the marketplace later accepts daemon packages, the AUR PKGBUILD in
`../aur/PKGBUILD` is the closest packaging description to adapt.

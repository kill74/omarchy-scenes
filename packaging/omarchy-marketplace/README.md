# Omarchy marketplace packaging

This repository is now submission-shaped for [plugins.omarchy.org](https://plugins.omarchy.org):

- root `manifest.json` with id `io.github.kill74.scenes`, kind `bar-widget`
- entry point `assets/scenes.status/Status.qml`
- root `README.md` with install and removal instructions
- root `LICENSE` (MIT)
- root `preview.png`

`omarchy plugin validate .` must pass before submitting.

Suggested submission metadata:

- Category: `Productivity`
- Tags: `bar`, `hyprland`, `workspaces`

The plugin widget depends on the separately installed `scenesd` daemon. The
README documents that dependency, the optional signal tools and the fact that
no sudo/pkexec is used. If submitting this whole repository rather than a
dedicated plugin-only repo, expect the security baseline to report the
`installer`, `service-management` and `remote-build` capabilities for review;
they come from `install.sh`, the systemd unit and the Rust build.

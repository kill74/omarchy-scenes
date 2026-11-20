# Scene packs

A pack is a git repository containing `scene-pack.toml` and a scenes TOML file.

```toml
[pack]
name = "Focus Pack"
version = "1.0.0"
description = "Deep work scenes"
scenes_file = "scenes.toml"

[[permissions]]
kind = "theme"
value = "Tokyo Night"
description = "changes the active Omarchy theme"
```

`scenes_file` belongs inside `[pack]` (TOML bare keys after a table header are
part of that table). A legacy top-level `scenes_file` is still accepted, but
`pack.scenes_file` wins when both are present. It must be a plain `.toml`
filename with no path separators or `..`. Manifests are capped at 64 KiB and
scenes files at 1 MiB.

Inspect a local manifest:

```bash
scenes pack inspect examples/focus-pack/scene-pack.toml
```

Install from git (always shows the permission summary first, and installs
nothing without `--confirm`):

```bash
scenes pack install https://github.com/you/your-scene-pack
scenes pack install https://github.com/you/your-scene-pack --confirm
```

`--confirm` copies the pack's scenes file into
`~/.config/omarchy/scenes/`. It refuses to overwrite an existing file, so
remove or rename it first. The clone goes to a temporary directory and is
deleted afterwards.

Packs are not executed as code: no `exec` is run by the installer. The
`scenes check` validator still requires `irreversible = true` on any `exec`
action in the installed config.

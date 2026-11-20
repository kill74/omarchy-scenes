# Security Policy

## Supported versions

| Version | Supported |
|---|---|
| 0.1.x | Yes |
| < 0.1 | No |

## Reporting a vulnerability

Please report security issues privately through GitHub's
[private vulnerability reporting](https://github.com/kill74/omarchy-scenes/security/advisories/new)
rather than opening a public issue. Include reproduction steps, affected version
and the potential impact. You should receive an acknowledgement within seven
days; fixes are released as patch versions when feasible.

Do not include secrets, tokens or personal data in reports.

## Trust model

Omarchy shell plugins run unsandboxed with the user's permissions. This plugin is
a small bar widget; the reactive logic lives in a separate user daemon and
systemd user service. The project requires **no sudo and no pkexec**.

Runtime actions only spawn fixed binaries (`omarchy`, `hyprctl`, `pactl`,
`nmcli`, `upower`, `gamemoded`, `omarchy-shell`, `python3`, `git`). Scene config
is never passed to a shell. Scene packs are cloned to temporary directories,
show a permission summary and require explicit `--confirm`; they are never
executed by the installer.

The marketplace listing is not a security review. Review the source before
installing.

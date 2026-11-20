#!/usr/bin/env bash
# Idempotent installer for Omarchy Scenes.
#   install.sh             install binaries, config and user service
#   install.sh --uninstall stop, disable and remove (config kept unless --purge-config)
# Test overrides: SCENES_BIN_DIR, SCENES_SKIP_BUILD, SCENES_SKIP_SYSTEMD, SCENES_PREFIX.
set -euo pipefail

root=$(cd "$(dirname "$0")" && pwd)
prefix=${SCENES_PREFIX:-$HOME/.local}
config=${XDG_CONFIG_HOME:-$HOME/.config}/omarchy/scenes
state=${XDG_STATE_HOME:-$HOME/.local/state}/omarchy-scenes
unit=${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user/scenesd.service

if [[ ${1:-} == --uninstall ]]; then
	if [[ ${SCENES_SKIP_SYSTEMD:-0} != 1 ]] && command -v systemctl >/dev/null; then
		systemctl --user disable --now scenesd.service >/dev/null 2>&1 || true
	fi
	rm -f "$unit" "$prefix/bin/scenesd" "$prefix/bin/scenes" "$state/scenes.sock" "$state/state.json"
	rmdir "$state" 2>/dev/null || true
	if [[ ${2:-} == --purge-config ]]; then
		rm -rf "$config"
	fi
	echo "Scenes uninstalled (config kept at $config unless --purge-config was used)."
	exit 0
fi

if [[ ${SCENES_SKIP_BUILD:-0} != 1 ]]; then
	(cd "$root" && cargo build --release)
	bindir="$root/target/release"
else
	bindir=${SCENES_BIN_DIR:-$root/target/release}
fi

install -Dm755 "$bindir/scenesd" "$prefix/bin/scenesd"
install -Dm755 "$bindir/scenes" "$prefix/bin/scenes"
install -Dm644 "$root/systemd/scenesd.service" "$unit"
mkdir -p "$state"

if [[ -f $config/scenes.toml ]]; then
	echo "Keeping existing config at $config/scenes.toml"
else
	install -Dm644 "$root/config/scenes.toml" "$config/scenes.toml"
fi

if [[ ${SCENES_SKIP_SYSTEMD:-0} == 1 ]]; then
	echo "SCENES_SKIP_SYSTEMD=1: service not enabled."
else
	systemctl --user daemon-reload
	systemctl --user enable --now scenesd.service
fi

echo "Scenes installed: $prefix/bin/scenes, config $config/scenes.toml"

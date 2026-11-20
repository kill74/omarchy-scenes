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
theme_hook=${XDG_CONFIG_HOME:-$HOME/.config}/omarchy/hooks/theme-set.d/50-scenes-manual
update_hook=${XDG_CONFIG_HOME:-$HOME/.config}/omarchy/hooks/post-update.d/50-scenes-validate
status_plugin=${XDG_CONFIG_HOME:-$HOME/.config}/omarchy/plugins/io.github.kill74.scenes
legacy_plugin=${XDG_CONFIG_HOME:-$HOME/.config}/omarchy/plugins/scenes.status
menu_file=${XDG_CONFIG_HOME:-$HOME/.config}/omarchy/extensions/omarchy-menu.jsonc

if [[ ${1:-} == --uninstall ]]; then
	if [[ ${SCENES_SKIP_SYSTEMD:-0} != 1 ]] && command -v systemctl >/dev/null; then
		systemctl --user disable --now scenesd.service >/dev/null 2>&1 || true
	fi
	rm -f "$unit" "$prefix/bin/scenesd" "$prefix/bin/scenes" "$state/scenes.sock" "$state/state.json"
	rm -f "$theme_hook" "$update_hook"
	rm -rf "$status_plugin" "$legacy_plugin"
	if [[ -f $menu_file ]]; then
		python3 "$root/scripts/install-menu.py" remove "$menu_file" || true
	fi
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
install -Dm755 "$root/hooks/theme-set.d/50-scenes-manual" "$theme_hook"
install -Dm755 "$root/hooks/post-update.d/50-scenes-validate" "$update_hook"
mkdir -p "$status_plugin"
install -Dm644 "$root/manifest.json" "$status_plugin/manifest.json"
install -Dm644 "$root/assets/scenes.status/Status.qml" "$status_plugin/assets/scenes.status/Status.qml"
if [[ -f $menu_file ]]; then
	python3 "$root/scripts/install-menu.py" install "$menu_file" "$root/assets/menu-scenes.jsonc"
fi
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

#!/usr/bin/env python3
"""Insert or remove the Scenes block in omarchy-menu.jsonc, preserving user entries."""

import pathlib
import sys

BEGIN = "// scenes:begin"
END = "// scenes:end"


def main():
    if len(sys.argv) < 3:
        print(
            "usage: install-menu.py <install|remove> <menu.jsonc> [snippet.jsonc]",
            file=sys.stderr,
        )
        return 2
    action, menu_arg = sys.argv[1], pathlib.Path(sys.argv[2])
    text = menu_arg.read_text(encoding="utf-8")
    lines = text.splitlines()

    if action == "remove":
        starts = [i for i, line in enumerate(lines) if BEGIN in line]
        if not starts:
            return 0
        start = starts[0]
        end = next((i for i in range(start, len(lines)) if END in lines[i]), None)
        if end is None:
            return 0
        # Drop a trailing comma on the preceding entry, if any.
        before = start - 1
        while before >= 0 and not lines[before].strip():
            before -= 1
        if before >= 0 and lines[before].rstrip().endswith(","):
            lines[before] = lines[before].rstrip()[:-1]
        del lines[start : end + 1]
        menu_arg.write_text("\n".join(lines) + "\n", encoding="utf-8")
        return 0

    if BEGIN in text:
        print("Scenes menu entries already present; keeping them.")
        return 0
    if len(sys.argv) < 4:
        print("snippet path required for install", file=sys.stderr)
        return 2
    snippet = pathlib.Path(sys.argv[3]).read_text(encoding="utf-8").rstrip()
    idx = text.rfind("}")
    if idx < 0:
        print("menu file has no closing brace", file=sys.stderr)
        return 1
    before = text[:idx].rstrip()
    comma = "" if before.endswith(",") else ","
    updated = before + comma + "\n" + snippet + "\n" + text[idx:]
    menu_arg.write_text(updated, encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

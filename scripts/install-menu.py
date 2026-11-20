#!/usr/bin/env python3
"""Insert or remove the Scenes block in omarchy-menu.jsonc, preserving user entries."""

import json
import pathlib
import re
import sys

BEGIN = "// scenes:begin"
END = "// scenes:end"


def strip_comments(text):
    """Remove // and /* */ comments without touching URLs inside strings."""
    out = []
    i = 0
    in_string = False
    escaped = False
    while i < len(text):
        ch = text[i]
        nxt = text[i + 1] if i + 1 < len(text) else ""
        if in_string:
            out.append(ch)
            if escaped:
                escaped = False
            elif ch == "\\":
                escaped = True
            elif ch == '"':
                in_string = False
            i += 1
            continue
        if ch == '"':
            in_string = True
            out.append(ch)
            i += 1
            continue
        if ch == "/" and nxt == "/":
            while i < len(text) and text[i] != "\n":
                i += 1
            continue
        if ch == "/" and nxt == "*":
            i += 2
            while i + 1 < len(text) and not (text[i] == "*" and text[i + 1] == "/"):
                i += 1
            i += 2
            continue
        out.append(ch)
        i += 1
    return "".join(out)


def validate_jsonc(text):
    """Return an error string if the JSONC would not parse, else None."""
    stripped = strip_comments(text)
    stripped = re.sub(r",(\s*[}\]])", r"\1", stripped)
    try:
        json.loads(stripped)
    except ValueError as exc:
        return str(exc)
    return None


def remove_block(lines):
    starts = [i for i, line in enumerate(lines) if BEGIN in line]
    if not starts:
        return None
    start = starts[0]
    end = next((i for i in range(start, len(lines)) if END in lines[i]), None)
    if end is None:
        return None

    # A surviving entry below the block means the preceding comma must stay.
    following = next(
        (
            line.strip()
            for line in lines[end + 1 :]
            if line.strip() and not line.strip().startswith("//")
        ),
        "",
    )
    has_following_entry = following not in ("", "}")

    before = start - 1
    while before >= 0 and not lines[before].strip():
        before -= 1
    if before >= 0:
        trimmed = lines[before].rstrip()
        if not has_following_entry and trimmed.endswith(","):
            lines[before] = trimmed[:-1]
        elif has_following_entry and not trimmed.endswith(","):
            lines[before] = trimmed + ","

    del lines[start : end + 1]
    if start < len(lines) and not lines[start].strip():
        del lines[start]
    return lines


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
        updated_lines = remove_block(lines)
        if updated_lines is None:
            return 0
        updated = "\n".join(updated_lines) + "\n"
        error = validate_jsonc(updated)
        if error:
            print(f"refusing to write invalid JSONC: {error}", file=sys.stderr)
            return 1
        menu_arg.write_text(updated, encoding="utf-8")
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
    error = validate_jsonc(updated)
    if error:
        print(f"refusing to write invalid JSONC: {error}", file=sys.stderr)
        return 1
    menu_arg.write_text(updated, encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

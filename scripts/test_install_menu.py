#!/usr/bin/env python3
"""Tests for install-menu.py. Run with: python3 scripts/test_install_menu.py"""

import importlib.util
import pathlib
import sys
import tempfile
import unittest

SCRIPT = pathlib.Path(__file__).with_name("install-menu.py")
spec = importlib.util.spec_from_file_location("install_menu", SCRIPT)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

SNIPPET = """  // scenes:begin
  "scenes": {"icon": "x", "label": "Scenes"},
  "scenes.off": {"icon": "y", "label": "Off", "action": "scenes off"}
  // scenes:end"""

WITH_FOLLOWING = (
    """{
  // a comment
  "a": {"label": "A"},
%s,
  "b": {"label": "B", "action": "https://example.com/a//b"}
}
"""
    % SNIPPET
)

LAST = (
    """{
  "a": {"label": "A"},
%s
}
"""
    % SNIPPET
)


class InstallMenuTests(unittest.TestCase):
    def run_script(self, action, text, snippet=None):
        with tempfile.TemporaryDirectory() as tmp:
            menu = pathlib.Path(tmp) / "menu.jsonc"
            menu.write_text(text, encoding="utf-8")
            argv = ["install-menu.py", action, str(menu)]
            if snippet:
                snippet_path = pathlib.Path(tmp) / "snippet.jsonc"
                snippet_path.write_text(snippet, encoding="utf-8")
                argv.append(str(snippet_path))
            original = sys.argv
            sys.argv = argv
            try:
                code = module.main()
            finally:
                sys.argv = original
            return code, menu.read_text(encoding="utf-8")

    def test_remove_with_following_entry(self):
        code, text = self.run_script("remove", WITH_FOLLOWING)
        self.assertEqual(code, 0)
        self.assertIsNone(module.validate_jsonc(text))
        self.assertIn('"b":', text)
        self.assertNotIn("scenes:begin", text)

    def test_remove_last_entry(self):
        code, text = self.run_script("remove", LAST)
        self.assertEqual(code, 0)
        self.assertIsNone(module.validate_jsonc(text))
        self.assertNotIn('"a": {"label": "A"},', text)

    def test_install_validates(self):
        base = '{\n  "a": {"label": "A"}\n}\n'
        code, text = self.run_script("install", base, SNIPPET)
        self.assertEqual(code, 0)
        self.assertIsNone(module.validate_jsonc(text))
        self.assertIn("scenes:begin", text)

    def test_urls_are_not_comments(self):
        self.assertIsNone(
            module.validate_jsonc(
                '{"url": "https://example.com/a//b", "note": "/* not a comment */"}'
            )
        )


if __name__ == "__main__":
    unittest.main()

#!/usr/bin/env python3
"""Exercise release-note placement without building, committing, or publishing."""

import os
from pathlib import Path
import subprocess
import tempfile
import unittest


RELEASE_SCRIPT = Path(__file__).resolve().with_name("release.sh")


class ReleaseNotesTest(unittest.TestCase):
    def test_new_header_owns_unreleased_notes_before_historical_sections(self):
        for separator in ("", "---\n\n"):
            with self.subTest(separator=separator), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                files = {
                    "Cargo.toml": '[workspace.package]\nversion = "0.3.4"\n',
                    "Cargo.lock": "# No build is performed by this fixture.\n",
                    "addons/stage/plugin.cfg": 'version="0.3.4"\n',
                    "addons/director/plugin.cfg": 'version="0.3.4"\n',
                    "client-plugins/claude/.claude-plugin/plugin.json": '{"version": "0.3.4"}\n',
                    "client-plugins/codex/.codex-plugin/plugin.json": '{"version": "0.3.4"}\n',
                    "site/guide/installation.md": '"version": "0.3.4"\n',
                    "site/api/wire-format.md": '"version": "0.3.4"\n',
                    "site/changelog.md": (
                        "# Changelog\n\n## [Unreleased]\n\n### Stage\n- Current change\n\n"
                        + separator
                        + "## [0.3.4] — 2026-08-15\n\n- Historical change\n\n---\n\n"
                        + "[Unreleased]: https://github.com/nklisch/theatre/compare/v0.3.4...HEAD\n"
                    ),
                }
                for name, contents in files.items():
                    path = root / name
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_text(contents)
                tools = root / "fake-tools"
                tools.mkdir()
                # Resolve these commands only inside the subprocess. In particular,
                # a fixture must never create a real tag or contact a Git remote.
                for name in ("cargo", "git"):
                    tool = tools / name
                    tool.write_text("#!/bin/sh\nexit 0\n")
                    tool.chmod(0o755)
                subprocess.run(
                    ["bash", str(RELEASE_SCRIPT), "minor"],
                    cwd=root,
                    env={**os.environ, "PATH": f"{tools}{os.pathsep}{os.environ['PATH']}"},
                    check=True,
                    capture_output=True,
                    text=True,
                    timeout=15,
                )
                notes = (root / "site/changelog.md").read_text()
                self.assertLess(notes.index("## [Unreleased]"), notes.index("## [0.4.0]"))
                self.assertLess(notes.index("## [0.4.0]"), notes.index("- Current change"))
                self.assertLess(notes.index("- Current change"), notes.index("## [0.3.4]"))
                self.assertIn("## [0.3.4] — 2026-08-15\n\n- Historical change", notes)
                self.assertIn("compare/v0.4.0...HEAD", notes)
                self.assertIn("releases/tag/v0.4.0", notes)


if __name__ == "__main__":
    unittest.main()

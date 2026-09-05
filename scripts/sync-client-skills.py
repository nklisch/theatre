#!/usr/bin/env python3
"""Synchronize native client plugin skills from Theatre's canonical skill tree."""

from __future__ import annotations

import argparse
import re
import shutil
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SKILLS = ("theatre-stage", "theatre-director")
PLUGIN_ROOTS = (ROOT / "client-plugins" / "claude", ROOT / "client-plugins" / "codex")
LINK_PATTERN = re.compile(r"\[[^]]*]\(([^)]+)\)")


def files_under(root: Path) -> dict[Path, bytes]:
    return {
        path.relative_to(root): path.read_bytes()
        for path in sorted(root.rglob("*"))
        if path.is_file()
    }


def check_copy(source: Path, destination: Path) -> list[str]:
    errors: list[str] = []
    expected = files_under(source)
    actual = files_under(destination) if destination.is_dir() else {}
    if expected.keys() != actual.keys():
        missing = sorted(expected.keys() - actual.keys())
        extra = sorted(actual.keys() - expected.keys())
        if missing:
            errors.append(f"{destination}: missing {', '.join(map(str, missing))}")
        if extra:
            errors.append(f"{destination}: extra {', '.join(map(str, extra))}")
    for relative in expected.keys() & actual.keys():
        if expected[relative] != actual[relative]:
            errors.append(f"{destination / relative}: differs from canonical source")
    return errors


def check_links(plugin_root: Path) -> list[str]:
    errors: list[str] = []
    resolved_root = plugin_root.resolve()
    for document in plugin_root.joinpath("skills").rglob("*.md"):
        for target in LINK_PATTERN.findall(document.read_text(encoding="utf-8")):
            path_text = target.split("#", 1)[0]
            if not path_text or "://" in path_text or path_text.startswith("mailto:"):
                continue
            resolved = (document.parent / path_text).resolve()
            if not resolved.is_relative_to(resolved_root):
                errors.append(f"{document}: link escapes plugin root: {target}")
            elif not resolved.exists():
                errors.append(f"{document}: unresolved link: {target}")
    return errors


def synchronize() -> None:
    for plugin_root in PLUGIN_ROOTS:
        skills_root = plugin_root / "skills"
        skills_root.mkdir(parents=True, exist_ok=True)
        for skill in SKILLS:
            destination = skills_root / skill
            shutil.rmtree(destination, ignore_errors=True)
            shutil.copytree(ROOT / ".agents" / "skills" / skill, destination)


def check() -> list[str]:
    errors: list[str] = []
    for plugin_root in PLUGIN_ROOTS:
        for skill in SKILLS:
            errors.extend(
                check_copy(
                    ROOT / ".agents" / "skills" / skill,
                    plugin_root / "skills" / skill,
                )
            )
        errors.extend(check_links(plugin_root))
    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="report stale or incomplete copies without changing files",
    )
    args = parser.parse_args()

    if not args.check:
        synchronize()

    errors = check()
    if errors:
        print("Client skill synchronization failed:", file=sys.stderr)
        for error in errors:
            print(f"  - {error}", file=sys.stderr)
        return 1

    action = "verified" if args.check else "synchronized"
    print(f"Client skill copies {action} from .agents/skills.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

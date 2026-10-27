#!/usr/bin/env python3
"""Rewrite Rust import paths across repository source files.

Usage:
    python rewrite_imports.py <rules_file.json> [root_dir]
    python rewrite_imports.py --replace "old::path" "new::path" [root_dir]
"""

import json
import os
import sys
from pathlib import Path


def rewrite_file(path: Path, mapping: dict[str, str]) -> bool:
    try:
        text = path.read_text(encoding="utf-8")
    except UnicodeDecodeError:
        return False
    original = text
    for old, new in mapping.items():
        text = text.replace(old, new)
    if text != original:
        path.write_text(text, encoding="utf-8")
        return True
    return False


def main() -> int:
    args = sys.argv[1:]
    if not args:
        print(__doc__)
        return 1

    mapping = {}
    root_dir = "."

    if args[0] == "--replace":
        if len(args) < 3:
            print("Error: --replace requires <old> and <new>", file=sys.stderr)
            return 1
        mapping[args[1]] = args[2]
        if len(args) >= 4:
            root_dir = args[3]
    else:
        rules_path = Path(args[0])
        if not rules_path.is_file():
            print(f"Error: rules file not found: {rules_path}", file=sys.stderr)
            return 1
        with open(rules_path, "r", encoding="utf-8") as f:
            mapping = json.load(f)
        if len(args) >= 2:
            root_dir = args[1]

    root = Path(root_dir)
    changed_count = 0
    for path in root.rglob("*.rs"):
        # Skip target and .tools directories
        parts = path.parts
        if "target" in parts or ".tools" in parts or ".venv" in parts:
            continue
        if rewrite_file(path, mapping):
            print(f"Updated {path}")
            changed_count += 1

    print(f"Total files updated: {changed_count}")
    return 0


if __name__ == "__main__":
    sys.exit(main())

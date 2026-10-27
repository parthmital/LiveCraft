"""Apply the quick-xml 0.36 -> 0.42 API changes to crates/livecraft-core/src/als.rs.

Purpose: quick-xml 0.42 exposes attribute keys, attribute values, and element names as `str`
instead of bytes. These rules adapt the existing parser without changing its behaviour.
Validated on 2026-10-27: after these rules, `cargo clippy -D warnings` is clean and all tests pass.
Inputs: path to als.rs.
Outputs: rewrites the file in place and prints the number of replacements per rule.
Run (from the repository root):
    ./.venv/Scripts/python plans/livecraft-1.0/tools/quick_xml_042_migrate.py crates/livecraft-core/src/als.rs
"""

import re
import sys

RULES = [
    (
        r"String::from_utf8_lossy\(&attr\.value\)",
        "String::from_utf8_lossy(attr.value.as_bytes())",
    ),
    (
        r"std::str::from_utf8\(&attr\.value\)",
        "std::str::from_utf8(attr.value.as_bytes())",
    ),
    (
        r"String::from_utf8_lossy\(attr\.key\.as_ref\(\)\)",
        "String::from_utf8_lossy(attr.key.as_ref().as_bytes())",
    ),
    (
        r"String::from_utf8_lossy\(e\.name\(\)\.as_ref\(\)\)",
        "String::from_utf8_lossy(e.name().as_ref().as_bytes())",
    ),
    (r'attr\.key\.as_ref\(\) == b"([A-Za-z]+)"', r'attr.key.as_ref() == "\1"'),
    (r'(?m)^(\s*)b"([A-Za-z]+)" =>', r'\1"\2" =>'),
]


def main():
    if len(sys.argv) != 2:
        sys.exit("usage: quick_xml_042_migrate.py <path/to/als.rs>")
    path = sys.argv[1]
    with open(path, encoding="utf-8") as handle:
        text = handle.read()
    for pattern, replacement in RULES:
        text, count = re.subn(pattern, replacement, text)
        print(f"{count:4d}  {pattern}")
    with open(path, "w", encoding="utf-8", newline="\n") as handle:
        handle.write(text)


if __name__ == "__main__":
    main()

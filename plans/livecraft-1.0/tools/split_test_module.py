"""Move an inline `#[cfg(test)] mod tests { ... }` block that ends a Rust file into its own file.

Purpose: shrink god files mechanically, with no visibility changes (the new file is still a child module).
Inputs: path to a Rust source file whose last item is `#[cfg(test)]` followed by `mod tests {`.
Outputs: rewrites the file to end with `#[cfg(test)]\nmod tests;` and writes the block body to
`<dir>/tests.rs` (for lib.rs, main.rs, mod.rs) or `<dir>/<stem>/tests.rs` (for other files).
Run (from the repository root):
    ./.venv/Scripts/python plans/livecraft-1.0/tools/split_test_module.py crates/livecraft-core/src/lib.rs
Then run `cargo fmt --all`.
"""

import os
import sys


def main():
    if len(sys.argv) != 2:
        sys.exit("usage: split_test_module.py <path/to/file.rs>")
    path = sys.argv[1]
    with open(path, encoding="utf-8") as handle:
        lines = handle.read().splitlines()

    start = None
    for index in range(len(lines) - 1):
        if (
            lines[index].strip() == "#[cfg(test)]"
            and lines[index + 1].strip() == "mod tests {"
        ):
            start = index
    if start is None:
        sys.exit(f"{path}: no '#[cfg(test)]' + 'mod tests {{' pair found")

    end = len(lines) - 1
    while end > start and lines[end].strip() == "":
        end -= 1
    if lines[end].strip() != "}":
        sys.exit(
            f"{path}: the test module is not the last item in the file; move it by hand"
        )

    body = lines[start + 2 : end]
    dedented = [line[4:] if line.startswith("    ") else line for line in body]

    directory, filename = os.path.split(path)
    stem = os.path.splitext(filename)[0]
    if stem in ("lib", "main", "mod"):
        target = os.path.join(directory, "tests.rs")
    else:
        target = os.path.join(directory, stem, "tests.rs")
    if os.path.exists(target):
        sys.exit(f"{target} already exists; refusing to overwrite")
    os.makedirs(os.path.dirname(target), exist_ok=True)

    with open(target, "w", encoding="utf-8", newline="\n") as handle:
        handle.write("\n".join(dedented).rstrip() + "\n")
    remaining = lines[:start] + ["#[cfg(test)]", "mod tests;"]
    with open(path, "w", encoding="utf-8", newline="\n") as handle:
        handle.write("\n".join(remaining) + "\n")
    print(f"moved {len(body)} lines from {path} to {target}")


if __name__ == "__main__":
    main()

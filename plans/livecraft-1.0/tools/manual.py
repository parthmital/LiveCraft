"""Query and render the Ableton Live 12 reference manual PDF.

Purpose: let agents read manual text and render manual pages to PNG without improvising PDF code.
Inputs: docs/ableton-live-12-reference-manual.pdf (override with --pdf).
Outputs: text on stdout; PNG files for the `render` command.
Run (from the repository root, with the repo-local virtual environment):
    ./.venv/Scripts/python plans/livecraft-1.0/tools/manual.py toc --depth 2
    ./.venv/Scripts/python plans/livecraft-1.0/tools/manual.py toc --filter "Auto Filter"
    ./.venv/Scripts/python plans/livecraft-1.0/tools/manual.py page 554
    ./.venv/Scripts/python plans/livecraft-1.0/tools/manual.py range 554 561
    ./.venv/Scripts/python plans/livecraft-1.0/tools/manual.py search "Follow Action" --max 20
    ./.venv/Scripts/python plans/livecraft-1.0/tools/manual.py section "29.2 Auto Filter"
    ./.venv/Scripts/python plans/livecraft-1.0/tools/manual.py render 35 36 --out .agent-local/reference/manual --dpi 200
On macOS or Linux use ./.venv/bin/python instead of ./.venv/Scripts/python.
"""

import argparse
import os
import sys

try:
    import fitz  # PyMuPDF
except ImportError:
    sys.exit(
        "PyMuPDF is missing. Run: python -m venv .venv, then "
        "./.venv/Scripts/python -m pip install -r plans/livecraft-1.0/tools/requirements.txt"
    )

DEFAULT_PDF = os.path.join("docs", "ableton-live-12-reference-manual.pdf")


def open_doc(path):
    if not os.path.exists(path):
        sys.exit(
            f"Manual not found at {path}. It is Git-ignored; ask the repository owner for it."
        )
    return fitz.open(path)


def section_bounds(doc, title):
    """Return (title, first_page, last_page) for every TOC entry whose title contains `title`."""
    toc = doc.get_toc()
    out = []
    for i, (level, entry, page) in enumerate(toc):
        if title.lower() not in entry.lower():
            continue
        last = doc.page_count
        for later_level, _, later_page in toc[i + 1 :]:
            if later_level <= level:
                last = max(page, later_page - 1)
                break
        out.append((entry, page, last))
    return out


def print_pages(doc, first, last):
    first = max(1, first)
    last = min(doc.page_count, last)
    for number in range(first, last + 1):
        print(f"--- page {number} of {doc.page_count} ---")
        print(doc[number - 1].get_text())


def cmd_toc(doc, args):
    for level, entry, page in doc.get_toc():
        if args.filter and args.filter.lower() not in entry.lower():
            continue
        if not args.filter and level > args.depth:
            continue
        print(f"{'  ' * (level - 1)}{entry} (p. {page})")


def cmd_search(doc, args):
    needle = args.text.lower()
    hits = 0
    for index in range(doc.page_count):
        lines = doc[index].get_text().splitlines()
        for line_no, line in enumerate(lines):
            if needle in line.lower():
                hits += 1
                context = " | ".join(lines[max(0, line_no - 1) : line_no + 2])
                print(f"p. {index + 1}: {context}")
                break
        if hits >= args.max:
            print(f"(stopped after {args.max} pages; raise --max for more)")
            return
    if hits == 0:
        print(f"No match for {args.text!r}.")


def cmd_section(doc, args):
    matches = section_bounds(doc, args.title)
    if not matches:
        sys.exit(f"No TOC entry contains {args.title!r}. Try: toc --filter <word>")
    for entry, first, last in matches[: args.limit]:
        print(f"=== {entry} (pages {first} to {last}) ===")
        print_pages(doc, first, last)


def cmd_render(doc, args):
    os.makedirs(args.out, exist_ok=True)
    zoom = args.dpi / 72.0
    for number in range(args.first, args.last + 1):
        if not 1 <= number <= doc.page_count:
            sys.exit(f"Page {number} is outside 1..{doc.page_count}")
        pixmap = doc[number - 1].get_pixmap(matrix=fitz.Matrix(zoom, zoom), alpha=False)
        target = os.path.join(args.out, f"manual-p{number:04d}.png")
        pixmap.save(target)
        print(target)


def main():
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--pdf", default=DEFAULT_PDF)
    sub = parser.add_subparsers(dest="command", required=True)

    p = sub.add_parser("toc", help="print the table of contents")
    p.add_argument("--depth", type=int, default=2)
    p.add_argument("--filter")
    p.set_defaults(func=cmd_toc)

    p = sub.add_parser("page", help="print one page")
    p.add_argument("number", type=int)
    p.set_defaults(func=lambda d, a: print_pages(d, a.number, a.number))

    p = sub.add_parser("range", help="print an inclusive page range")
    p.add_argument("first", type=int)
    p.add_argument("last", type=int)
    p.set_defaults(func=lambda d, a: print_pages(d, a.first, a.last))

    p = sub.add_parser("search", help="list pages containing text")
    p.add_argument("text")
    p.add_argument("--max", type=int, default=15)
    p.set_defaults(func=cmd_search)

    p = sub.add_parser("section", help="print a whole TOC section")
    p.add_argument("title")
    p.add_argument("--limit", type=int, default=1)
    p.set_defaults(func=cmd_section)

    p = sub.add_parser(
        "render",
        help="render pages to PNG (manual screenshots are tiled, so render, never extract)",
    )
    p.add_argument("first", type=int)
    p.add_argument("last", type=int)
    p.add_argument("--out", default=os.path.join(".agent-local", "reference", "manual"))
    p.add_argument("--dpi", type=int, default=200)
    p.set_defaults(func=cmd_render)

    args = parser.parse_args()
    args.func(open_doc(args.pdf), args)


if __name__ == "__main__":
    main()

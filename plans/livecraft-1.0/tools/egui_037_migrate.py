"""Apply the mechanical part of the egui 0.29 -> 0.37 migration to crates/livecraft-gui/src.

Purpose: rewrite renamed egui APIs deterministically so the executor only hand-edits what the task lists.
Inputs: the .rs files under the directory given as the first argument.
Outputs: rewrites those files in place and prints a count per rule.
Run (from the repository root):
    ./.venv/Scripts/python plans/livecraft-1.0/tools/egui_037_migrate.py crates/livecraft-gui/src
Rules (egui CHANGELOG 0.30 to 0.37):
    Rounding::same(<float>)            -> CornerRadius::same(<rounded u8>)
    Rounding (type or import)          -> CornerRadius
    .rounding( / .rounding =           -> .corner_radius( / .corner_radius =
    Margin::same(<float>)              -> Margin::same(<rounded i8>)
    rect_stroke(a, b, c)               -> rect_stroke(a, b, c, eframe::egui::StrokeKind::Middle)
    TopBottomPanel::top/bottom, SidePanel::left/right -> Panel::top/bottom/left/right
    .exact_height( / .exact_width(    -> .exact_size(   (only in chains that start with a Panel)
    .show(ctx, |ui|                    -> .show(ui, |ui|   (panels now take the parent Ui)
    SelectableLabel::new(              -> Button::selectable(
    ui.id()                            -> ui.scope_id()
    allocate_new_ui(                   -> scope_builder(
    ctx.style()                        -> ctx.global_style()
    ctx.set_style(                     -> ctx.set_global_style(
"""

import os
import re
import sys


def round_literal(match, prefix):
    value = float(match.group(1))
    return f"{prefix}::same({int(round(value))})"


def add_stroke_kind(text):
    """Append a StrokeKind argument to every rect_stroke( call, using balanced parentheses."""
    out = []
    index = 0
    count = 0
    needle = "rect_stroke("
    while True:
        found = text.find(needle, index)
        if found < 0:
            out.append(text[index:])
            break
        start = found + len(needle)
        depth = 1
        cursor = start
        while depth:
            char = text[cursor]
            if char == "(":
                depth += 1
            elif char == ")":
                depth -= 1
            cursor += 1
        close = cursor - 1
        args = text[start:close]
        if "StrokeKind" in args:
            out.append(text[index:cursor])
        else:
            stripped = args.rstrip()
            trailing = args[len(stripped) :]
            if stripped.endswith(","):
                new_args = stripped + " eframe::egui::StrokeKind::Middle," + trailing
            else:
                new_args = stripped + ", eframe::egui::StrokeKind::Middle" + trailing
            out.append(text[index:start] + new_args + ")")
            count += 1
        index = cursor
    return "".join(out), count


SIMPLE_RULES = [
    (
        re.compile(r"\bRounding::same\(\s*([0-9]+(?:\.[0-9]+)?)\s*\)"),
        lambda m: round_literal(m, "CornerRadius"),
    ),
    (
        re.compile(r"\bMargin::same\(\s*([0-9]+\.[0-9]+)\s*\)"),
        lambda m: round_literal(m, "Margin"),
    ),
    (re.compile(r"\bRounding\b"), lambda m: "CornerRadius"),
    (re.compile(r"\.rounding\("), lambda m: ".corner_radius("),
    (re.compile(r"\.rounding\s*="), lambda m: ".corner_radius ="),
    (
        re.compile(r"\bTopBottomPanel::(top|bottom)\("),
        lambda m: f"Panel::{m.group(1)}(",
    ),
    (re.compile(r"\bSidePanel::(left|right)\("), lambda m: f"Panel::{m.group(1)}("),
    (re.compile(r"\.show\(ctx, \|ui\|"), lambda m: ".show(ui, |ui|"),
    (re.compile(r"\bSelectableLabel::new\("), lambda m: "Button::selectable("),
    (re.compile(r"\bui\.id\(\)"), lambda m: "ui.scope_id()"),
    (re.compile(r"\ballocate_new_ui\("), lambda m: "scope_builder("),
    (re.compile(r"\bctx\.style\(\)"), lambda m: "ctx.global_style()"),
    (re.compile(r"\bctx\.set_style\("), lambda m: "ctx.set_global_style("),
]

PANEL_SIZE = re.compile(
    r"(Panel::(?:top|bottom|left|right)\([^)]*\)\s*)\.exact_(?:height|width)\("
)


def migrate(path, totals):
    with open(path, encoding="utf-8") as handle:
        text = handle.read()
    original = text
    for pattern, replace in SIMPLE_RULES:
        text, n = pattern.subn(replace, text)
        totals[pattern.pattern] = totals.get(pattern.pattern, 0) + n
    text, n = PANEL_SIZE.subn(lambda m: m.group(1) + ".exact_size(", text)
    totals["Panel exact_size"] = totals.get("Panel exact_size", 0) + n
    text, n = add_stroke_kind(text)
    totals["rect_stroke StrokeKind"] = totals.get("rect_stroke StrokeKind", 0) + n
    if text != original:
        with open(path, "w", encoding="utf-8", newline="\n") as handle:
            handle.write(text)


def main():
    if len(sys.argv) != 2:
        sys.exit("usage: egui_037_migrate.py <directory>")
    totals = {}
    for root, _, files in os.walk(sys.argv[1]):
        for name in sorted(files):
            if name.endswith(".rs"):
                migrate(os.path.join(root, name), totals)
    for rule, count in totals.items():
        print(f"{count:4d}  {rule}")


if __name__ == "__main__":
    main()

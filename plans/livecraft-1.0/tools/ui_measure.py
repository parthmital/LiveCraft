"""Measure RGB colours, run-length scanlines, and tight ink bounding boxes in PNG captures.

Purpose: provide deterministic pixel measurements for UI theme tokens and layout metrics
described in ``plans/livecraft-1.0/reference/UI-FIDELITY.md``.
Inputs: a PNG image plus either a JSON measurement spec (``--spec``) or direct CLI
subcommands (``color``, ``runs``, ``text-bounds``).
Outputs: structured JSON written to ``-o`` / ``--output`` or stdout.

Run examples (from repository root with repo-local venv):
    ./.venv/Scripts/python plans/livecraft-1.0/tools/ui_measure.py --image shot.png --spec spec.json -o out.json
    ./.venv/Scripts/python plans/livecraft-1.0/tools/ui_measure.py color shot.png --points 10,20 30,40
    ./.venv/Scripts/python plans/livecraft-1.0/tools/ui_measure.py runs shot.png --line 0,12,199,12
    ./.venv/Scripts/python plans/livecraft-1.0/tools/ui_measure.py text-bounds shot.png --rect 10,10,120,24
"""

from __future__ import annotations

import argparse
import json
import os
import sys
from typing import Any

import numpy as np
from PIL import Image

DEFAULT_INK_THRESHOLD = 4


def load_rgb_array(image_path: str) -> np.ndarray:
    """Load an image file as an ``(H, W, 3)`` ``uint8`` NumPy array in RGB order."""
    if not os.path.exists(image_path):
        raise FileNotFoundError(f"Image not found: {image_path}")
    with Image.open(image_path) as image:
        rgb_image = image.convert("RGB")
        return np.asarray(rgb_image, dtype=np.uint8)


def rgb_to_hex(rgb: list[int] | tuple[int, int, int]) -> str:
    """Format an ``[r, g, b]`` sequence as lowercase ``#rrggbb`` hex."""
    return f"#{int(rgb[0]):02x}{int(rgb[1]):02x}{int(rgb[2]):02x}"


def parse_point_arg(value: str) -> tuple[int, int]:
    """Parse an ``x,y`` coordinate string into integer ``(x, y)``."""
    parts = [part.strip() for part in value.split(",")]
    if len(parts) != 2:
        raise argparse.ArgumentTypeError(f"Expected 'x,y', got {value!r}")
    try:
        return int(parts[0]), int(parts[1])
    except ValueError as exc:
        raise argparse.ArgumentTypeError(f"Invalid integer coordinate in {value!r}") from exc


def parse_line_arg(value: str) -> tuple[int, int, int, int]:
    """Parse an ``x1,y1,x2,y2`` scanline string into four integers."""
    parts = [part.strip() for part in value.split(",")]
    if len(parts) != 4:
        raise argparse.ArgumentTypeError(f"Expected 'x1,y1,x2,y2', got {value!r}")
    try:
        return int(parts[0]), int(parts[1]), int(parts[2]), int(parts[3])
    except ValueError as exc:
        raise argparse.ArgumentTypeError(f"Invalid integer coordinate in {value!r}") from exc


def parse_rect_arg(value: str) -> tuple[int, int, int, int]:
    """Parse an ``x,y,w,h`` rectangle string into four integers."""
    parts = [part.strip() for part in value.split(",")]
    if len(parts) != 4:
        raise argparse.ArgumentTypeError(f"Expected 'x,y,w,h', got {value!r}")
    try:
        return int(parts[0]), int(parts[1]), int(parts[2]), int(parts[3])
    except ValueError as exc:
        raise argparse.ArgumentTypeError(f"Invalid integer coordinate in {value!r}") from exc


def parse_rgb_arg(value: str) -> tuple[int, int, int]:
    """Parse an ``r,g,b`` colour string into integer ``(r, g, b)``."""
    parts = [part.strip() for part in value.split(",")]
    if len(parts) != 3:
        raise argparse.ArgumentTypeError(f"Expected 'r,g,b', got {value!r}")
    try:
        channels = (int(parts[0]), int(parts[1]), int(parts[2]))
    except ValueError as exc:
        raise argparse.ArgumentTypeError(f"Invalid integer colour channel in {value!r}") from exc
    for channel in channels:
        if not 0 <= channel <= 255:
            raise argparse.ArgumentTypeError(f"RGB channel out of range 0..255 in {value!r}")
    return channels


def sample_color(rgb_array: np.ndarray, x: int, y: int) -> list[int]:
    """Sample exact ``[r, g, b]`` colour at pixel coordinate ``(x, y)``."""
    height, width = rgb_array.shape[:2]
    if not (0 <= x < width and 0 <= y < height):
        raise ValueError(f"Point ({x}, {y}) is outside image bounds ({width}x{height})")
    pixel = rgb_array[y, x]
    return [int(pixel[0]), int(pixel[1]), int(pixel[2])]


def measure_runs(
    rgb_array: np.ndarray,
    x1: int,
    y1: int,
    x2: int,
    y2: int,
    tolerance: int = 0,
) -> tuple[str, list[tuple[list[int], int, int]]]:
    """Measure contiguous colour run segments along an inclusive horizontal or vertical scanline.

    Returns ``(axis, segments)`` where ``axis`` is ``"horizontal"`` or ``"vertical"`` and
    each segment is ``([r, g, b], start_coord, length)``.
    """
    height, width = rgb_array.shape[:2]
    if tolerance < 0:
        raise ValueError(f"Tolerance must be >= 0, got {tolerance}")
    if y1 != y2 and x1 != x2:
        raise ValueError(
            f"Scanline ({x1}, {y1}) -> ({x2}, {y2}) must be strictly horizontal or vertical"
        )
    if x2 < x1 or y2 < y1:
        raise ValueError(
            f"Scanline endpoints must satisfy x1 <= x2 and y1 <= y2, got ({x1}, {y1}, {x2}, {y2})"
        )
    if not (0 <= x1 < width and 0 <= x2 < width and 0 <= y1 < height and 0 <= y2 < height):
        raise ValueError(
            f"Scanline ({x1}, {y1}, {x2}, {y2}) is outside image bounds ({width}x{height})"
        )

    if y1 == y2:
        axis = "horizontal"
        pixels = rgb_array[y1, x1 : x2 + 1]
        base_coord = x1
    else:
        axis = "vertical"
        pixels = rgb_array[y1 : y2 + 1, x1]
        base_coord = y1

    segments: list[tuple[list[int], int, int]] = []
    if len(pixels) == 0:
        return axis, segments

    current_rgb = [int(pixels[0, 0]), int(pixels[0, 1]), int(pixels[0, 2])]
    run_start = base_coord
    run_len = 1

    for offset in range(1, len(pixels)):
        px = pixels[offset]
        px_rgb = [int(px[0]), int(px[1]), int(px[2])]
        if tolerance == 0:
            is_match = px_rgb == current_rgb
        else:
            is_match = (
                abs(px_rgb[0] - current_rgb[0]) <= tolerance
                and abs(px_rgb[1] - current_rgb[1]) <= tolerance
                and abs(px_rgb[2] - current_rgb[2]) <= tolerance
            )
        if is_match:
            run_len += 1
        else:
            segments.append((current_rgb, run_start, run_len))
            current_rgb = px_rgb
            run_start = base_coord + offset
            run_len = 1

    segments.append((current_rgb, run_start, run_len))
    return axis, segments


def estimate_border_background_rgb(region_rgb: np.ndarray) -> list[int]:
    """Estimate the most common RGB colour along the 1-pixel outer border of a region."""
    height, width = region_rgb.shape[:2]
    if height <= 2 or width <= 2:
        samples = region_rgb.reshape(-1, 3)
    else:
        top = region_rgb[0, :]
        bottom = region_rgb[-1, :]
        left = region_rgb[1:-1, 0]
        right = region_rgb[1:-1, -1]
        samples = np.concatenate([top, bottom, left, right], axis=0)

    unique_colors, counts = np.unique(samples, axis=0, return_counts=True)
    best_index = int(np.argmax(counts))
    best = unique_colors[best_index]
    return [int(best[0]), int(best[1]), int(best[2])]


def measure_text_bounds(
    rgb_array: np.ndarray,
    x: int,
    y: int,
    w: int,
    h: int,
    bg_rgb: tuple[int, int, int] | list[int] | None = None,
    threshold: int = DEFAULT_INK_THRESHOLD,
) -> dict[str, Any]:
    """Compute tight ink bounding box ``[x, y, w, h]`` inside region ``(x, y, w, h)``."""
    height, width = rgb_array.shape[:2]
    if w <= 0 or h <= 0:
        raise ValueError(f"Rectangle width and height must be positive, got w={w}, h={h}")
    if x < 0 or y < 0 or x + w > width or y + h > height:
        raise ValueError(
            f"Rectangle ({x}, {y}, {w}, {h}) exceeds image bounds ({width}x{height})"
        )
    if threshold < 0:
        raise ValueError(f"Threshold must be >= 0, got {threshold}")

    region = rgb_array[y : y + h, x : x + w]
    resolved_bg = (
        [int(bg_rgb[0]), int(bg_rgb[1]), int(bg_rgb[2])]
        if bg_rgb is not None
        else estimate_border_background_rgb(region)
    )

    bg_array = np.array(resolved_bg, dtype=np.int16)
    diff = np.abs(region.astype(np.int16) - bg_array)
    ink_mask = np.max(diff, axis=2) > threshold
    ink_pixel_count = int(np.count_nonzero(ink_mask))

    if ink_pixel_count == 0:
        return {
            "rect": [int(x), int(y), int(w), int(h)],
            "bbox": None,
            "relative_bbox": None,
            "bg_rgb": resolved_bg,
            "ink_pixel_count": 0,
        }

    ys, xs = np.nonzero(ink_mask)
    rel_x = int(xs.min())
    rel_y = int(ys.min())
    rel_w = int(xs.max()) - rel_x + 1
    rel_h = int(ys.max()) - rel_y + 1

    return {
        "rect": [int(x), int(y), int(w), int(h)],
        "bbox": [int(x + rel_x), int(y + rel_y), rel_w, rel_h],
        "relative_bbox": [rel_x, rel_y, rel_w, rel_h],
        "bg_rgb": resolved_bg,
        "ink_pixel_count": ink_pixel_count,
    }


def _unpack_point_entry(entry: dict[str, Any]) -> tuple[str | None, int, int]:
    name = entry.get("name")
    if "point" in entry:
        pt = entry["point"]
        if len(pt) != 2:
            raise ValueError(f"Color probe 'point' must have 2 elements: {entry!r}")
        return name, int(pt[0]), int(pt[1])
    if "x" in entry and "y" in entry:
        return name, int(entry["x"]), int(entry["y"])
    raise ValueError(f"Color probe must contain 'x'/'y' or 'point': {entry!r}")


def _unpack_run_entry(entry: dict[str, Any]) -> tuple[str | None, int, int, int, int, int]:
    name = entry.get("name")
    tolerance = int(entry.get("tolerance", 0))
    if "line" in entry:
        coords = entry["line"]
        if len(coords) != 4:
            raise ValueError(f"Run probe 'line' must have 4 elements: {entry!r}")
        return name, int(coords[0]), int(coords[1]), int(coords[2]), int(coords[3]), tolerance
    if all(key in entry for key in ("x1", "y1", "x2", "y2")):
        return (
            name,
            int(entry["x1"]),
            int(entry["y1"]),
            int(entry["x2"]),
            int(entry["y2"]),
            tolerance,
        )
    raise ValueError(f"Run probe must contain 'line' or 'x1,y1,x2,y2': {entry!r}")


def _unpack_text_entry(
    entry: dict[str, Any],
) -> tuple[str | None, int, int, int, int, list[int] | None, int]:
    name = entry.get("name")
    bg_raw = entry.get("bg")
    bg_rgb = [int(bg_raw[0]), int(bg_raw[1]), int(bg_raw[2])] if bg_raw is not None else None
    threshold = int(entry.get("threshold", DEFAULT_INK_THRESHOLD))
    if "rect" in entry:
        rect = entry["rect"]
        if len(rect) != 4:
            raise ValueError(f"Text bounds 'rect' must have 4 elements: {entry!r}")
        return name, int(rect[0]), int(rect[1]), int(rect[2]), int(rect[3]), bg_rgb, threshold
    if all(key in entry for key in ("x", "y", "w", "h")):
        return (
            name,
            int(entry["x"]),
            int(entry["y"]),
            int(entry["w"]),
            int(entry["h"]),
            bg_rgb,
            threshold,
        )
    raise ValueError(f"Text bounds probe must contain 'rect' or 'x,y,w,h': {entry!r}")


def measure_spec(image_path: str, spec: dict[str, Any]) -> dict[str, Any]:
    """Run all colour, run-length, and text-bounds measurements defined in ``spec``."""
    rgb_array = load_rgb_array(image_path)
    height, width = rgb_array.shape[:2]

    color_results: list[dict[str, Any]] = []
    for entry in spec.get("colors", []):
        name, x, y = _unpack_point_entry(entry)
        rgb = sample_color(rgb_array, x, y)
        item: dict[str, Any] = {"x": x, "y": y, "rgb": rgb, "hex": rgb_to_hex(rgb)}
        if name is not None:
            item["name"] = name
        color_results.append(item)

    run_results: list[dict[str, Any]] = []
    for entry in spec.get("runs", []):
        name, x1, y1, x2, y2, tolerance = _unpack_run_entry(entry)
        axis, segments = measure_runs(rgb_array, x1, y1, x2, y2, tolerance=tolerance)
        item = {
            "line": [x1, y1, x2, y2],
            "axis": axis,
            "segments": [[seg_rgb, start, length] for seg_rgb, start, length in segments],
        }
        if name is not None:
            item["name"] = name
        run_results.append(item)

    text_entries = spec.get("text_bounds", spec.get("text-bounds", []))
    text_results: list[dict[str, Any]] = []
    for entry in text_entries:
        name, x, y, w, h, bg_rgb, threshold = _unpack_text_entry(entry)
        result = measure_text_bounds(
            rgb_array, x, y, w, h, bg_rgb=bg_rgb, threshold=threshold
        )
        if name is not None:
            result["name"] = name
        text_results.append(result)

    return {
        "image": image_path,
        "width": int(width),
        "height": int(height),
        "colors": color_results,
        "runs": run_results,
        "text_bounds": text_results,
    }


def write_json_output(payload: dict[str, Any], output_path: str | None) -> None:
    """Write formatted JSON to ``output_path`` or stdout when ``output_path`` is omitted."""
    rendered = json.dumps(payload, indent=2)
    if output_path:
        parent = os.path.dirname(output_path)
        if parent:
            os.makedirs(parent, exist_ok=True)
        with open(output_path, "w", encoding="utf-8") as handle:
            handle.write(rendered + "\n")
    else:
        print(rendered)


def _cmd_spec(args: argparse.Namespace) -> int:
    with open(args.spec, "r", encoding="utf-8") as handle:
        spec = json.load(handle)
    payload = measure_spec(args.image, spec)
    write_json_output(payload, args.output)
    return 0


def _cmd_color(args: argparse.Namespace) -> int:
    rgb_array = load_rgb_array(args.image)
    height, width = rgb_array.shape[:2]
    colors = []
    for x, y in args.points:
        rgb = sample_color(rgb_array, x, y)
        colors.append({"x": x, "y": y, "rgb": rgb, "hex": rgb_to_hex(rgb)})
    payload = {
        "image": args.image,
        "width": int(width),
        "height": int(height),
        "colors": colors,
    }
    write_json_output(payload, args.output)
    return 0


def _cmd_runs(args: argparse.Namespace) -> int:
    rgb_array = load_rgb_array(args.image)
    height, width = rgb_array.shape[:2]
    x1, y1, x2, y2 = args.line
    axis, segments = measure_runs(
        rgb_array, x1, y1, x2, y2, tolerance=args.tolerance
    )
    payload = {
        "image": args.image,
        "width": int(width),
        "height": int(height),
        "line": [x1, y1, x2, y2],
        "axis": axis,
        "segments": [[seg_rgb, start, length] for seg_rgb, start, length in segments],
    }
    write_json_output(payload, args.output)
    return 0


def _cmd_text_bounds(args: argparse.Namespace) -> int:
    rgb_array = load_rgb_array(args.image)
    height, width = rgb_array.shape[:2]
    x, y, w, h = args.rect
    result = measure_text_bounds(
        rgb_array, x, y, w, h, bg_rgb=args.bg, threshold=args.threshold
    )
    payload = {
        "image": args.image,
        "width": int(width),
        "height": int(height),
        **result,
    }
    write_json_output(payload, args.output)
    return 0


def _build_spec_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="ui_measure.py",
        description=__doc__.splitlines()[0],
    )
    parser.add_argument("--image", required=True, help="Path to PNG capture")
    parser.add_argument("--spec", required=True, help="Path to JSON measurement spec")
    parser.add_argument("-o", "--output", help="Optional output JSON path")
    parser.set_defaults(func=_cmd_spec)
    return parser


def _build_subcommand_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="ui_measure.py",
        description=(
            "Measure RGB colours, run lengths, and text bounding boxes in PNG captures.\n"
            "Use either `--image <png> --spec <json> [-o out.json]` or a subcommand."
        ),
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    p_color = subparsers.add_parser("color", help="Sample RGB colour(s) at coordinates")
    p_color.add_argument("image", help="Path to PNG capture")
    p_color.add_argument(
        "--points",
        type=parse_point_arg,
        nargs="+",
        action="extend",
        required=True,
        help="Coordinate(s) as x,y",
    )
    p_color.add_argument("-o", "--output", help="Optional output JSON path")
    p_color.set_defaults(func=_cmd_color)

    p_runs = subparsers.add_parser("runs", help="Measure run-length segments along a line")
    p_runs.add_argument("image", help="Path to PNG capture")
    p_runs.add_argument(
        "--line",
        type=parse_line_arg,
        required=True,
        help="Scanline endpoints as x1,y1,x2,y2 (inclusive)",
    )
    p_runs.add_argument(
        "--tolerance",
        type=int,
        default=0,
        help="Max per-channel delta for matching pixels in a run (default: 0)",
    )
    p_runs.add_argument("-o", "--output", help="Optional output JSON path")
    p_runs.set_defaults(func=_cmd_runs)

    p_text = subparsers.add_parser(
        "text-bounds", help="Measure tight ink bounding box inside a rectangle"
    )
    p_text.add_argument("image", help="Path to PNG capture")
    p_text.add_argument(
        "--rect",
        type=parse_rect_arg,
        required=True,
        help="Search region as x,y,w,h",
    )
    p_text.add_argument(
        "--bg",
        type=parse_rgb_arg,
        default=None,
        help="Optional background colour r,g,b (auto-detected from border if omitted)",
    )
    p_text.add_argument(
        "--threshold",
        type=int,
        default=DEFAULT_INK_THRESHOLD,
        help=f"Per-channel contrast threshold above background (default: {DEFAULT_INK_THRESHOLD})",
    )
    p_text.add_argument("-o", "--output", help="Optional output JSON path")
    p_text.set_defaults(func=_cmd_text_bounds)

    return parser


def main(argv: list[str] | None = None) -> int:
    """CLI entrypoint supporting both ``--image/--spec`` and direct helper subcommands."""
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")
    raw_args = list(sys.argv[1:] if argv is None else argv)

    if raw_args and raw_args[0] in {"color", "runs", "text-bounds"}:
        parser = _build_subcommand_parser()
    elif "--spec" in raw_args or "--image" in raw_args:
        parser = _build_spec_parser()
    else:
        parser = _build_subcommand_parser()

    args = parser.parse_args(raw_args)
    try:
        return int(args.func(args))
    except (FileNotFoundError, ValueError, json.JSONDecodeError) as exc:
        print(f"Error: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())

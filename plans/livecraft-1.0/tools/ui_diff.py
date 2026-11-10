"""Compare LiveCraft UI captures against Live reference captures region by region.

Purpose: implement the reference parity thresholds in
``plans/livecraft-1.0/reference/UI-FIDELITY.md#comparison-and-thresholds`` and
``plans/livecraft-1.0/reference/QUALITY-GATES.md``.
Inputs: ``plans/livecraft-1.0/reference/capture-list.md`` (or a JSON region spec),
Live reference PNGs in ``.agent-local/reference/live/``, and LiveCraft PNGs.
Outputs: ``report.json``, ``index.html``, and PNG diff heatmaps inside
``.agent-local/output/ui-diff/`` (or ``--out-dir``).

Run examples (from repository root with repo-local venv):
    ./.venv/Scripts/python plans/livecraft-1.0/tools/ui_diff.py
    ./.venv/Scripts/python plans/livecraft-1.0/tools/ui_diff.py batch --require-references
    ./.venv/Scripts/python plans/livecraft-1.0/tools/ui_diff.py compare-pair --live ref.png --craft craft.png --spec regions.json
"""

from __future__ import annotations

import argparse
import html
import json
import os
import re
import sys
from typing import Any

import numpy as np
from PIL import Image

DEFAULT_CAPTURE_LIST = os.path.join("plans", "livecraft-1.0", "reference", "capture-list.md")
DEFAULT_REF_DIR = os.path.join(".agent-local", "reference", "live")
DEFAULT_CRAFT_DIR = os.path.join(".agent-local", "output", "ui-captures")
DEFAULT_OUT_DIR = os.path.join(".agent-local", "output", "ui-diff")

MAX_EDGE_DELTA_PX = 1
MAX_SOLID_CHANNEL_DELTA = 2
MIN_SSIM_DEFAULT = 0.97
MIN_SSIM_TEXT = 0.95
MAX_TEXT_SHIFT_PX = 1
MIN_TEXT_CORRELATION = 0.5

EDGE_CONTRAST_THRESHOLD = 4
INK_CONTRAST_THRESHOLD = 4
SOLID_PTP_TOLERANCE = 2
SSIM_WINDOW_SIZE = 7
SSIM_SIGMA = 1.5
SSIM_C1 = (0.01 * 255.0) ** 2
SSIM_C2 = (0.03 * 255.0) ** 2

_STATE_ID_PATTERN = re.compile(r"^[a-z0-9-]+__[a-z0-9-]+__[a-z0-9-]+$")
_INLINE_REGION_PATTERN = re.compile(
    r"^(?:(?P<name>[A-Za-z0-9_.-]+)\s*[:=]\s*)?"
    r"(?P<x>\d+)\s*,\s*(?P<y>\d+)\s*,\s*(?P<w>\d+)\s*,\s*(?P<h>\d+)"
    r"(?:\s*[\(\[]\s*(?P<kind>[A-Za-z0-9_-]+)\s*[\)\]])?$"
)


def _gaussian_1d_kernel(size: int = SSIM_WINDOW_SIZE, sigma: float = SSIM_SIGMA) -> np.ndarray:
    radius = size // 2
    coords = np.arange(-radius, radius + 1, dtype=np.float64)
    weights = np.exp(-(coords * coords) / (2.0 * sigma * sigma))
    weights /= np.sum(weights)
    return weights


_GAUSSIAN_1D_7 = _gaussian_1d_kernel(SSIM_WINDOW_SIZE, SSIM_SIGMA)


def _convolve_separable_7x7(plane: np.ndarray) -> np.ndarray:
    """Apply a normalized 7x7 Gaussian filter using separable 1D passes and edge padding."""
    height, width = plane.shape
    pad = SSIM_WINDOW_SIZE // 2
    padded_h = np.pad(plane, ((0, 0), (pad, pad)), mode="edge")
    horiz = np.zeros((height, width), dtype=np.float64)
    for idx in range(SSIM_WINDOW_SIZE):
        horiz += _GAUSSIAN_1D_7[idx] * padded_h[:, idx : idx + width]

    padded_v = np.pad(horiz, ((pad, pad), (0, 0)), mode="edge")
    out = np.zeros((height, width), dtype=np.float64)
    for idx in range(SSIM_WINDOW_SIZE):
        out += _GAUSSIAN_1D_7[idx] * padded_v[idx : idx + height, :]
    return out


def to_luminance_float64(image: np.ndarray) -> np.ndarray:
    """Convert an RGB ``(H, W, 3)`` or greyscale ``(H, W)`` array to BT.601 float64 luminance."""
    arr = np.asarray(image, dtype=np.float64)
    if arr.ndim == 2:
        return arr
    if arr.ndim == 3 and arr.shape[2] >= 3:
        return 0.299 * arr[:, :, 0] + 0.587 * arr[:, :, 1] + 0.114 * arr[:, :, 2]
    raise ValueError(f"Expected 2D greyscale or 3D RGB image array, got shape {arr.shape}")


def compute_ssim_map_7x7_gaussian(
    image_a: np.ndarray, image_b: np.ndarray
) -> tuple[float, np.ndarray]:
    """Compute mean SSIM and per-pixel SSIM map using a 7x7 Gaussian window on greyscale."""
    lum_a = to_luminance_float64(image_a)
    lum_b = to_luminance_float64(image_b)
    if lum_a.shape != lum_b.shape:
        raise ValueError(
            f"Image shapes must match for SSIM, got {lum_a.shape} vs {lum_b.shape}"
        )
    if lum_a.size == 0:
        raise ValueError("Cannot compute SSIM on empty array")

    mu_a = _convolve_separable_7x7(lum_a)
    mu_b = _convolve_separable_7x7(lum_b)
    mu_a_sq = mu_a * mu_a
    mu_b_sq = mu_b * mu_b
    mu_ab = mu_a * mu_b

    sigma_a_sq = np.maximum(0.0, _convolve_separable_7x7(lum_a * lum_a) - mu_a_sq)
    sigma_b_sq = np.maximum(0.0, _convolve_separable_7x7(lum_b * lum_b) - mu_b_sq)
    sigma_ab = _convolve_separable_7x7(lum_a * lum_b) - mu_ab

    numerator = (2.0 * mu_ab + SSIM_C1) * (2.0 * sigma_ab + SSIM_C2)
    denominator = (mu_a_sq + mu_b_sq + SSIM_C1) * (sigma_a_sq + sigma_b_sq + SSIM_C2)
    ssim_map = numerator / denominator
    mean_ssim = float(np.mean(ssim_map))
    return mean_ssim, ssim_map


def compute_ssim_7x7_gaussian(image_a: np.ndarray, image_b: np.ndarray) -> float:
    """Compute scalar mean SSIM using a pure-NumPy 7x7 Gaussian window on greyscale."""
    mean_ssim, _ = compute_ssim_map_7x7_gaussian(image_a, image_b)
    return mean_ssim


def load_rgb_array(image_path: str) -> np.ndarray:
    """Load an image file as an ``(H, W, 3)`` ``uint8`` NumPy array."""
    if not os.path.exists(image_path):
        raise FileNotFoundError(f"Image not found: {image_path}")
    with Image.open(image_path) as img:
        return np.asarray(img.convert("RGB"), dtype=np.uint8)


def _extract_edge_bbox(region_rgb: np.ndarray, threshold: int = EDGE_CONTRAST_THRESHOLD) -> list[int] | None:
    """Find inclusive ``[left, top, right, bottom]`` bbox of gradient edges in a region."""
    height, width = region_rgb.shape[:2]
    arr = region_rgb.astype(np.int16)
    edge_mask = np.zeros((height, width), dtype=bool)

    if width > 1:
        dx = np.max(np.abs(arr[:, 1:, :] - arr[:, :-1, :]), axis=2) > threshold
        edge_mask[:, :-1] |= dx
        edge_mask[:, 1:] |= dx
    if height > 1:
        dy = np.max(np.abs(arr[1:, :, :] - arr[:-1, :, :]), axis=2) > threshold
        edge_mask[:-1, :] |= dy
        edge_mask[1:, :] |= dy

    if not np.any(edge_mask):
        return None
    ys, xs = np.nonzero(edge_mask)
    return [int(xs.min()), int(ys.min()), int(xs.max()), int(ys.max())]


def compute_edge_bbox_diff(
    region_live: np.ndarray,
    region_craft: np.ndarray,
    max_edge_delta_px: int = MAX_EDGE_DELTA_PX,
    threshold: int = EDGE_CONTRAST_THRESHOLD,
) -> dict[str, Any]:
    """Compare content gradient edge bounding boxes between live and craft regions."""
    live_edges = _extract_edge_bbox(region_live, threshold=threshold)
    craft_edges = _extract_edge_bbox(region_craft, threshold=threshold)

    if live_edges is None and craft_edges is None:
        return {
            "applicable": False,
            "passed": True,
            "live_bbox": None,
            "craft_bbox": None,
            "edge_deltas": None,
            "max_edge_delta_px": 0,
            "threshold_px": max_edge_delta_px,
        }

    if live_edges is None or craft_edges is None:
        return {
            "applicable": True,
            "passed": False,
            "live_bbox": live_edges,
            "craft_bbox": craft_edges,
            "edge_deltas": None,
            "max_edge_delta_px": None,
            "threshold_px": max_edge_delta_px,
        }

    deltas = {
        "left": abs(craft_edges[0] - live_edges[0]),
        "top": abs(craft_edges[1] - live_edges[1]),
        "right": abs(craft_edges[2] - live_edges[2]),
        "bottom": abs(craft_edges[3] - live_edges[3]),
    }
    max_delta = max(deltas.values())
    return {
        "applicable": True,
        "passed": max_delta <= max_edge_delta_px,
        "live_bbox": live_edges,
        "craft_bbox": craft_edges,
        "edge_deltas": deltas,
        "max_edge_delta_px": int(max_delta),
        "threshold_px": max_edge_delta_px,
    }


def _is_solid_region(region_rgb: np.ndarray, tolerance: int = SOLID_PTP_TOLERANCE) -> bool:
    ptp = np.ptp(region_rgb, axis=(0, 1))
    return bool(np.max(ptp) <= tolerance)


def compute_solid_color_diff(
    region_live: np.ndarray,
    region_craft: np.ndarray,
    max_delta: int = MAX_SOLID_CHANNEL_DELTA,
    force: bool = False,
) -> dict[str, Any]:
    """Check whether each RGB channel delta between live and craft stays within ``max_delta``."""
    is_live_solid = _is_solid_region(region_live)
    if not force and not is_live_solid:
        return {
            "applicable": False,
            "passed": True,
            "live_mean_rgb": [round(float(v), 2) for v in np.mean(region_live, axis=(0, 1))],
            "craft_mean_rgb": [round(float(v), 2) for v in np.mean(region_craft, axis=(0, 1))],
            "max_channel_delta": 0,
            "channel_deltas": [0, 0, 0],
            "threshold": max_delta,
        }

    diff = np.abs(region_live.astype(np.int16) - region_craft.astype(np.int16))
    per_channel_max = [int(v) for v in np.max(diff, axis=(0, 1))]
    overall_max = max(per_channel_max)
    return {
        "applicable": True,
        "passed": overall_max <= max_delta,
        "live_mean_rgb": [round(float(v), 2) for v in np.mean(region_live, axis=(0, 1))],
        "craft_mean_rgb": [round(float(v), 2) for v in np.mean(region_craft, axis=(0, 1))],
        "channel_deltas": per_channel_max,
        "max_channel_delta": overall_max,
        "threshold": max_delta,
    }


def _estimate_border_bg_and_coverage(region_rgb: np.ndarray) -> tuple[np.ndarray, float]:
    height, width = region_rgb.shape[:2]
    if height <= 2 or width <= 2:
        samples = region_rgb.reshape(-1, 3)
    else:
        samples = np.concatenate(
            [
                region_rgb[0, :],
                region_rgb[-1, :],
                region_rgb[1:-1, 0],
                region_rgb[1:-1, -1],
            ],
            axis=0,
        )
    unique_colors, counts = np.unique(samples, axis=0, return_counts=True)
    best_idx = int(np.argmax(counts))
    bg_rgb = unique_colors[best_idx]
    coverage = float(counts[best_idx]) / float(len(samples))
    return bg_rgb, coverage


def _find_profile_shift(live_profile: np.ndarray, craft_profile: np.ndarray) -> tuple[int, float]:
    """Find integer lag ``d`` aligning ``live_profile[i]`` to ``craft_profile[i + d]``."""
    length = len(live_profile)
    if length == 0:
        return 0, 1.0

    norm_live = float(np.linalg.norm(live_profile))
    norm_craft = float(np.linalg.norm(craft_profile))
    if norm_live == 0.0 and norm_craft == 0.0:
        return 0, 1.0
    if norm_live == 0.0 or norm_craft == 0.0:
        return length, 0.0

    corr = np.correlate(craft_profile, live_profile, mode="full") / (norm_live * norm_craft)
    lags = np.arange(-(length - 1), length, dtype=np.int64)
    max_corr = float(np.max(corr))
    near_best = np.flatnonzero(corr >= max_corr - 1e-9)
    best_idx = int(min(near_best, key=lambda idx: abs(int(lags[idx]))))
    return int(lags[best_idx]), round(float(corr[best_idx]), 6)


def compute_text_alignment_diff(
    region_live: np.ndarray,
    region_craft: np.ndarray,
    max_shift_px: int = MAX_TEXT_SHIFT_PX,
    force: bool = False,
) -> dict[str, Any]:
    """Measure row and column ink projection profile shift between live and craft."""
    bg_rgb, bg_coverage = _estimate_border_bg_and_coverage(region_live)
    bg_i16 = bg_rgb.astype(np.int16)

    ink_live = np.max(np.abs(region_live.astype(np.int16) - bg_i16), axis=2).astype(np.float64)
    ink_craft = np.max(np.abs(region_craft.astype(np.int16) - bg_i16), axis=2).astype(np.float64)
    ink_live = np.where(ink_live > float(INK_CONTRAST_THRESHOLD), ink_live, 0.0)
    ink_craft = np.where(ink_craft > float(INK_CONTRAST_THRESHOLD), ink_craft, 0.0)

    has_live_ink = bool(np.any(ink_live > 0.0))
    has_craft_ink = bool(np.any(ink_craft > 0.0))

    if not force:
        if _is_solid_region(region_live) or bg_coverage < 0.25 or not has_live_ink:
            return {
                "applicable": False,
                "passed": True,
                "row_shift_px": 0,
                "col_shift_px": 0,
                "max_shift_px": 0,
                "row_correlation": 1.0,
                "col_correlation": 1.0,
                "threshold_px": max_shift_px,
            }

    if not has_live_ink and not has_craft_ink:
        return {
            "applicable": True,
            "passed": True,
            "row_shift_px": 0,
            "col_shift_px": 0,
            "max_shift_px": 0,
            "row_correlation": 1.0,
            "col_correlation": 1.0,
            "threshold_px": max_shift_px,
        }

    if has_live_ink != has_craft_ink:
        return {
            "applicable": True,
            "passed": False,
            "row_shift_px": None,
            "col_shift_px": None,
            "max_shift_px": None,
            "row_correlation": 0.0,
            "col_correlation": 0.0,
            "threshold_px": max_shift_px,
        }

    row_live = np.sum(ink_live, axis=1)
    row_craft = np.sum(ink_craft, axis=1)
    col_live = np.sum(ink_live, axis=0)
    col_craft = np.sum(ink_craft, axis=0)

    row_shift, row_corr = _find_profile_shift(row_live, row_craft)
    col_shift, col_corr = _find_profile_shift(col_live, col_craft)
    max_abs_shift = max(abs(row_shift), abs(col_shift))
    passed = max_abs_shift <= max_shift_px and min(row_corr, col_corr) >= MIN_TEXT_CORRELATION

    return {
        "applicable": True,
        "passed": bool(passed),
        "row_shift_px": row_shift,
        "col_shift_px": col_shift,
        "max_shift_px": max_abs_shift,
        "row_correlation": row_corr,
        "col_correlation": col_corr,
        "threshold_px": max_shift_px,
    }


def _crop_region(image_rgb: np.ndarray, rect: list[int] | None) -> np.ndarray:
    height, width = image_rgb.shape[:2]
    if rect is None:
        return image_rgb
    x, y, w, h = [int(v) for v in rect]
    if w <= 0 or h <= 0:
        raise ValueError(f"Region width and height must be positive, got {rect}")
    if x < 0 or y < 0 or x + w > width or y + h > height:
        raise ValueError(f"Region {rect} exceeds image bounds ({width}x{height})")
    return image_rgb[y : y + h, x : x + w]


def compare_region(
    live_rgb: np.ndarray,
    craft_rgb: np.ndarray,
    region_spec: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Run all UI fidelity checks on one cropped or full-frame region."""
    spec = region_spec or {}
    name = str(spec.get("name", "full_frame"))
    kind = str(spec.get("kind", "auto")).lower()
    rect = spec.get("rect")

    live_crop = _crop_region(live_rgb, rect)
    craft_crop = _crop_region(craft_rgb, rect)
    crop_h, crop_w = live_crop.shape[:2]
    effective_rect = [0, 0, int(crop_w), int(crop_h)] if rect is None else [int(v) for v in rect]

    if "min_ssim" in spec and spec["min_ssim"] is not None:
        ssim_threshold = float(spec["min_ssim"])
    elif kind == "text":
        ssim_threshold = MIN_SSIM_TEXT
    else:
        ssim_threshold = MIN_SSIM_DEFAULT

    ssim_score = compute_ssim_7x7_gaussian(live_crop, craft_crop)
    ssim_result = {
        "applicable": True,
        "passed": bool(ssim_score >= ssim_threshold),
        "score": round(ssim_score, 6),
        "threshold": ssim_threshold,
    }

    bbox_result = compute_edge_bbox_diff(live_crop, craft_crop)
    solid_result = compute_solid_color_diff(
        live_crop, craft_crop, force=(kind == "solid")
    )
    text_result = compute_text_alignment_diff(
        live_crop,
        craft_crop,
        force=(kind == "text"),
    ) if kind not in {"solid", "box", "bbox"} else {
        "applicable": False,
        "passed": True,
        "row_shift_px": 0,
        "col_shift_px": 0,
        "max_shift_px": 0,
        "row_correlation": 1.0,
        "col_correlation": 1.0,
        "threshold_px": MAX_TEXT_SHIFT_PX,
    }

    checks = {
        "bbox": bbox_result,
        "solid_color": solid_result,
        "ssim": ssim_result,
        "text_alignment": text_result,
    }
    passed = all(check["passed"] for check in checks.values())

    return {
        "name": name,
        "kind": kind,
        "rect": effective_rect,
        "passed": bool(passed),
        "checks": checks,
    }


def _safe_filename(stem: str) -> str:
    cleaned = re.sub(r"[^A-Za-z0-9_.-]+", "_", stem).strip("._")
    return cleaned or "capture"


def save_diff_heatmap(
    live_rgb: np.ndarray,
    craft_rgb: np.ndarray,
    out_dir: str,
    state_id: str,
) -> str:
    """Save an amplified pixel-difference heatmap PNG inside ``<out_dir>/assets/``."""
    assets_dir = os.path.join(out_dir, "assets")
    os.makedirs(assets_dir, exist_ok=True)
    filename = f"{_safe_filename(state_id)}_heatmap.png"
    abs_out_dir = os.path.abspath(out_dir)
    target_path = os.path.abspath(os.path.join(assets_dir, filename))
    if not target_path.startswith(abs_out_dir + os.sep):
        raise ValueError(f"Unsafe heatmap path outside output directory: {target_path}")

    diff = np.max(np.abs(live_rgb.astype(np.int16) - craft_rgb.astype(np.int16)), axis=2)
    base_gray = np.clip(to_luminance_float64(live_rgb) * 0.25, 0, 255).astype(np.float64)
    amplified = np.clip(diff.astype(np.float64) * 8.0, 0, 255)

    has_diff = diff > 0
    red = np.where(has_diff, np.clip(140.0 + amplified * 0.45, 0, 255), base_gray)
    green = np.where(has_diff, np.clip(220.0 - amplified * 0.85, 0, 255), base_gray)
    blue = np.where(has_diff, np.clip(40.0 + np.maximum(0.0, amplified - 128.0), 0, 255), base_gray)

    heatmap = np.stack([red, green, blue], axis=2).astype(np.uint8)
    Image.fromarray(heatmap, mode="RGB").save(target_path)
    return os.path.join("assets", filename).replace("\\", "/")


def compare_image_pair(
    live_path: str,
    craft_path: str,
    regions: list[dict[str, Any]] | None = None,
    state_id: str | None = None,
    out_dir: str | None = None,
) -> dict[str, Any]:
    """Compare one Live reference image and one LiveCraft image across all specified regions."""
    resolved_state = state_id or os.path.splitext(os.path.basename(live_path))[0]
    live_rgb = load_rgb_array(live_path)
    craft_rgb = load_rgb_array(craft_path)

    if live_rgb.shape != craft_rgb.shape:
        return {
            "state_id": resolved_state,
            "status": "failed",
            "passed": False,
            "error": (
                f"Dimension mismatch: live {live_rgb.shape[1]}x{live_rgb.shape[0]} "
                f"vs craft {craft_rgb.shape[1]}x{craft_rgb.shape[0]}"
            ),
            "live_path": live_path,
            "craft_path": craft_path,
            "heatmap_path": None,
            "regions": [],
        }

    region_list = regions if regions else [{"name": "full_frame", "rect": None, "kind": "auto"}]
    region_results = [compare_region(live_rgb, craft_rgb, region) for region in region_list]
    passed = all(result["passed"] for result in region_results)

    heatmap_rel = None
    if out_dir is not None:
        heatmap_rel = save_diff_heatmap(live_rgb, craft_rgb, out_dir, resolved_state)

    return {
        "state_id": resolved_state,
        "status": "passed" if passed else "failed",
        "passed": bool(passed),
        "live_path": live_path,
        "craft_path": craft_path,
        "heatmap_path": heatmap_rel,
        "regions": region_results,
    }


def _parse_inline_regions(text: str) -> list[dict[str, Any]]:
    cleaned = text.strip().strip("`").strip()
    if not cleaned or cleaned in {"-", "—", "none"}:
        return []
    if cleaned.startswith("["):
        parsed = json.loads(cleaned)
        if isinstance(parsed, list):
            return [_normalize_region_dict(item, idx) for idx, item in enumerate(parsed)]

    regions: list[dict[str, Any]] = []
    for idx, token in enumerate(re.split(r"\s*;\s*", cleaned)):
        part = token.strip()
        if not part:
            continue
        match = _INLINE_REGION_PATTERN.match(part)
        if not match:
            raise ValueError(f"Could not parse inline region spec: {part!r}")
        name = match.group("name") or f"region_{idx}"
        kind = (match.group("kind") or "auto").lower()
        rect = [
            int(match.group("x")),
            int(match.group("y")),
            int(match.group("w")),
            int(match.group("h")),
        ]
        regions.append({"name": name, "rect": rect, "kind": kind})
    return regions


def _normalize_region_dict(raw: dict[str, Any], index: int = 0) -> dict[str, Any]:
    name = str(raw.get("name", f"region_{index}"))
    kind = str(raw.get("kind", "auto")).lower()
    rect_val = raw.get("rect")
    if rect_val is None and all(k in raw for k in ("x", "y", "w", "h")):
        rect_val = [int(raw["x"]), int(raw["y"]), int(raw["w"]), int(raw["h"])]
    elif rect_val is not None:
        rect_val = [int(v) for v in rect_val]
    out: dict[str, Any] = {"name": name, "rect": rect_val, "kind": kind}
    if "min_ssim" in raw and raw["min_ssim"] is not None:
        out["min_ssim"] = float(raw["min_ssim"])
    return out


def _clean_cell(cell: str) -> str:
    return cell.strip().strip("`").strip()


def _is_separator_row(cells: list[str]) -> bool:
    return all(re.fullmatch(r":?-{3,}:?", cell.strip()) is not None for cell in cells if cell.strip())


def _split_md_row(line: str) -> list[str]:
    stripped = line.strip()
    if stripped.startswith("|"):
        stripped = stripped[1:]
    if stripped.endswith("|"):
        stripped = stripped[:-1]
    return [part.strip() for part in stripped.split("|")]


def parse_capture_list(markdown_path: str) -> list[dict[str, Any]]:
    """Parse state and region Markdown tables from ``capture-list.md``.

    Supports:
    - Tables with ``State ID`` / ``State`` and ``Regions`` columns
    - Tables with ``State``, ``Region``, ``Rect`` (or ``X``, ``Y``, ``W``, ``H``), ``Kind``
    - Heading-scoped region tables where ``## <state_id>`` precedes a ``| Region | Rect | Kind |`` table
    """
    if not os.path.exists(markdown_path):
        return []

    with open(markdown_path, "r", encoding="utf-8") as handle:
        lines = handle.read().splitlines()

    states_by_id: dict[str, dict[str, Any]] = {}
    state_order: list[str] = []
    current_heading_state: str | None = None

    def ensure_state(state_id: str) -> dict[str, Any]:
        if state_id not in states_by_id:
            states_by_id[state_id] = {"id": state_id, "regions": []}
            state_order.append(state_id)
        return states_by_id[state_id]

    idx = 0
    while idx < len(lines):
        line = lines[idx].strip()
        if line.startswith("#"):
            heading_text = _clean_cell(line.lstrip("#").strip())
            if _STATE_ID_PATTERN.match(heading_text):
                current_heading_state = heading_text
                ensure_state(heading_text)
            else:
                tokens = heading_text.split()
                matched = next((tok.strip("`") for tok in tokens if _STATE_ID_PATTERN.match(tok.strip("`"))), None)
                current_heading_state = matched
                if matched:
                    ensure_state(matched)
            idx += 1
            continue

        if "|" in line and idx + 1 < len(lines) and "|" in lines[idx + 1]:
            headers_raw = _split_md_row(line)
            sep_cells = _split_md_row(lines[idx + 1])
            if headers_raw and sep_cells and _is_separator_row(sep_cells):
                headers = [_clean_cell(h).lower().replace(" ", "_").replace("-", "_") for h in headers_raw]
                idx += 2
                while idx < len(lines) and "|" in lines[idx]:
                    row_cells = _split_md_row(lines[idx])
                    if len(row_cells) < len(headers):
                        row_cells.extend([""] * (len(headers) - len(row_cells)))
                    row = {headers[col]: row_cells[col] for col in range(len(headers))}
                    _consume_table_row(row, current_heading_state, ensure_state)
                    idx += 1
                continue
        idx += 1

    return [states_by_id[sid] for sid in state_order]


def _consume_table_row(
    row: dict[str, str],
    current_heading_state: str | None,
    ensure_state: Any,
) -> None:
    state_col = _clean_cell(row.get("state_id") or row.get("state") or row.get("id") or "")
    state_id = state_col if state_col else (current_heading_state or "")
    if not state_id:
        return
    entry = ensure_state(state_id)

    region_name = _clean_cell(row.get("region") or row.get("region_name") or "")
    kind = _clean_cell(row.get("kind") or "auto").lower() or "auto"
    rect_cell = _clean_cell(row.get("rect") or "")
    regions_cell = row.get("regions", "")

    if region_name and (rect_cell or all(k in row for k in ("x", "y", "w", "h"))):
        if rect_cell:
            nums = [int(v.strip()) for v in rect_cell.split(",")]
            if len(nums) != 4:
                raise ValueError(f"Expected 4 integers in rect {rect_cell!r} for {state_id}")
            rect = nums
        else:
            rect = [
                int(_clean_cell(row["x"])),
                int(_clean_cell(row["y"])),
                int(_clean_cell(row["w"])),
                int(_clean_cell(row["h"])),
            ]
        entry["regions"].append({"name": region_name, "rect": rect, "kind": kind})
    elif regions_cell:
        parsed_regions = _parse_inline_regions(regions_cell)
        entry["regions"].extend(parsed_regions)


def load_json_spec(spec_path: str) -> tuple[list[dict[str, Any]] | None, list[dict[str, Any]] | None]:
    """Load either a region list spec or a multi-state batch spec from JSON.

    Returns ``(states, regions)`` where only one element is typically populated.
    """
    with open(spec_path, "r", encoding="utf-8") as handle:
        data = json.load(handle)

    if isinstance(data, list):
        return None, [_normalize_region_dict(item, idx) for idx, item in enumerate(data)]
    if isinstance(data, dict):
        if "states" in data:
            states = []
            for item in data["states"]:
                state_id = str(item["id"])
                regions = [
                    _normalize_region_dict(r, idx)
                    for idx, r in enumerate(item.get("regions", []))
                ]
                states.append(
                    {
                        "id": state_id,
                        "live": item.get("live"),
                        "craft": item.get("craft"),
                        "regions": regions,
                    }
                )
            return states, None
        if "regions" in data:
            return None, [
                _normalize_region_dict(item, idx) for idx, item in enumerate(data["regions"])
            ]
    raise ValueError(f"Unrecognized UI diff spec schema in {spec_path}")


def generate_html_report(report: dict[str, Any], out_dir: str) -> str:
    """Write a standalone HTML report at ``<out_dir>/index.html`` and return its path."""
    os.makedirs(out_dir, exist_ok=True)
    html_path = os.path.join(out_dir, "index.html")
    summary = report["summary"]
    status = report["status"]
    badge_class = f"badge-{status}"

    state_sections: list[str] = []
    if report.get("notice"):
        state_sections.append(
            f"<div class='notice'>{html.escape(report['notice'])}</div>"
        )

    for state in report.get("states", []):
        sid = html.escape(str(state.get("state_id", "")))
        st_status = str(state.get("status", "skipped"))
        rows_html: list[str] = []
        for reg in state.get("regions", []):
            checks = reg["checks"]
            bbox = checks["bbox"]
            solid = checks["solid_color"]
            ssim = checks["ssim"]
            text_al = checks["text_alignment"]

            bbox_str = (
                f"{bbox['max_edge_delta_px']} px"
                if bbox["applicable"]
                else "n/a"
            )
            solid_str = (
                f"{solid['max_channel_delta']} / 255"
                if solid["applicable"]
                else "n/a"
            )
            ssim_str = f"{ssim['score']:.4f} (min {ssim['threshold']:.2f})"
            text_str = (
                f"col={text_al['col_shift_px']}px, row={text_al['row_shift_px']}px"
                if text_al["applicable"]
                else "n/a"
            )
            reg_badge = "PASS" if reg["passed"] else "FAIL"
            reg_cls = "pass" if reg["passed"] else "fail"
            rows_html.append(
                "<tr>"
                f"<td>{html.escape(reg['name'])}</td>"
                f"<td>{html.escape(reg['kind'])}</td>"
                f"<td>{html.escape(str(reg['rect']))}</td>"
                f"<td>{html.escape(bbox_str)}</td>"
                f"<td>{html.escape(solid_str)}</td>"
                f"<td>{html.escape(ssim_str)}</td>"
                f"<td>{html.escape(text_str)}</td>"
                f"<td class='{reg_cls}'>{reg_badge}</td>"
                "</tr>"
            )

        heatmap_html = ""
        if state.get("heatmap_path"):
            hpath = html.escape(str(state["heatmap_path"]))
            heatmap_html = (
                f"<div class='heatmap'><a href='{hpath}'>"
                f"<img src='{hpath}' alt='Diff heatmap for {sid}'></a></div>"
            )
        error_html = (
            f"<p class='error'>{html.escape(state['error'])}</p>"
            if state.get("error")
            else ""
        )
        reason_html = (
            f"<p class='muted'>{html.escape(state['reason'])}</p>"
            if state.get("reason")
            else ""
        )

        table_html = (
            "<table><thead><tr>"
            "<th>Region</th><th>Kind</th><th>Rect</th>"
            "<th>Edge Delta</th><th>Solid Delta</th><th>SSIM (7x7)</th><th>Text Shift</th><th>Status</th>"
            "</tr></thead><tbody>"
            + "".join(rows_html)
            + "</tbody></table>"
            if rows_html
            else ""
        )

        state_sections.append(
            f"<section class='state-card'>"
            f"<h2>{sid} <span class='badge badge-{html.escape(st_status)}'>{html.escape(st_status.upper())}</span></h2>"
            f"{error_html}{reason_html}{heatmap_html}{table_html}"
            "</section>"
        )

    missing_refs_html = ""
    missing_refs = summary.get("missing_references", [])
    if missing_refs:
        items = "".join(f"<li><code>{html.escape(p)}</code></li>" for p in missing_refs)
        missing_refs_html = f"<section class='state-card'><h3>Missing Reference Files</h3><ul>{items}</ul></section>"

    doc = f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>LiveCraft UI Fidelity Diff Report</title>
<style>
  body {{ font-family: system-ui, -apple-system, sans-serif; background: #16171a; color: #e6e8ec; margin: 24px; }}
  h1, h2, h3 {{ margin: 0 0 12px 0; }}
  .summary {{ display: flex; gap: 16px; flex-wrap: wrap; margin-bottom: 20px; padding: 14px; background: #202227; border-radius: 8px; }}
  .badge {{ display: inline-block; padding: 2px 8px; border-radius: 4px; font-size: 12px; font-weight: 700; }}
  .badge-passed, .pass {{ color: #6ee7b7; }}
  .badge-failed, .fail {{ color: #fca5a5; }}
  .badge-skipped {{ color: #fcd34d; }}
  .notice {{ padding: 12px 14px; background: #2b2517; border-left: 4px solid #f59e0b; border-radius: 4px; margin-bottom: 18px; }}
  .state-card {{ background: #202227; border: 1px solid #2e323b; border-radius: 8px; padding: 16px; margin-bottom: 18px; }}
  table {{ width: 100%; border-collapse: collapse; margin-top: 10px; font-size: 13px; }}
  th, td {{ text-align: left; padding: 6px 8px; border-bottom: 1px solid #2e323b; }}
  th {{ color: #a1a7b3; }}
  .heatmap img {{ max-width: 100%; max-height: 360px; border: 1px solid #353944; margin-top: 8px; }}
  .error {{ color: #fca5a5; }}
  .muted {{ color: #9aa0ad; }}
</style>
</head>
<body>
  <h1>LiveCraft UI Fidelity Diff Report <span class="badge {badge_class}">{html.escape(status.upper())}</span></h1>
  <div class="summary">
    <div><strong>Mode:</strong> {html.escape(str(report['mode']))}</div>
    <div><strong>Total states:</strong> {summary['total_states']}</div>
    <div><strong>Compared:</strong> {summary['compared_states']}</div>
    <div><strong>Passed:</strong> {summary['passed_states']}</div>
    <div><strong>Failed:</strong> {summary['failed_states']}</div>
    <div><strong>Skipped:</strong> {summary['skipped_states']}</div>
  </div>
  {''.join(state_sections)}
  {missing_refs_html}
</body>
</html>
"""
    with open(html_path, "w", encoding="utf-8") as handle:
        handle.write(doc)
    return html_path


def write_reports(report: dict[str, Any], out_dir: str) -> tuple[str, str]:
    """Write ``report.json`` and ``index.html`` inside ``out_dir``."""
    os.makedirs(out_dir, exist_ok=True)
    json_path = os.path.join(out_dir, "report.json")
    with open(json_path, "w", encoding="utf-8") as handle:
        json.dump(report, handle, indent=2)
        handle.write("\n")
    html_path = generate_html_report(report, out_dir)
    return json_path, html_path


def _base_thresholds_dict() -> dict[str, Any]:
    return {
        "max_edge_delta_px": MAX_EDGE_DELTA_PX,
        "max_solid_channel_delta": MAX_SOLID_CHANNEL_DELTA,
        "min_ssim": MIN_SSIM_DEFAULT,
        "min_ssim_text": MIN_SSIM_TEXT,
        "max_text_shift_px": MAX_TEXT_SHIFT_PX,
    }


def run_compare_pair(
    live_path: str,
    craft_path: str,
    spec_path: str | None = None,
    out_dir: str = DEFAULT_OUT_DIR,
    state_id: str | None = None,
) -> tuple[int, dict[str, Any]]:
    """Compare one explicit Live vs LiveCraft image pair and write JSON + HTML reports."""
    regions: list[dict[str, Any]] | None = None
    if spec_path:
        _, regions = load_json_spec(spec_path)

    result = compare_image_pair(
        live_path=live_path,
        craft_path=craft_path,
        regions=regions,
        state_id=state_id,
        out_dir=out_dir,
    )
    passed = bool(result["passed"])
    report = {
        "mode": "compare-pair",
        "status": "passed" if passed else "failed",
        "summary": {
            "total_states": 1,
            "compared_states": 1,
            "passed_states": 1 if passed else 0,
            "failed_states": 0 if passed else 1,
            "skipped_states": 0,
            "missing_references": [],
            "missing_craft": [],
        },
        "thresholds": _base_thresholds_dict(),
        "states": [result],
    }
    write_reports(report, out_dir)
    return (0 if passed else 1), report


def _discover_states(
    capture_list_path: str,
    spec_path: str | None,
    ref_dir: str,
) -> list[dict[str, Any]]:
    if spec_path:
        states, regions = load_json_spec(spec_path)
        if states is not None:
            return states
        if regions is not None and os.path.isdir(ref_dir):
            pngs = sorted(f for f in os.listdir(ref_dir) if f.lower().endswith(".png"))
            return [{"id": os.path.splitext(name)[0], "regions": regions} for name in pngs]

    if capture_list_path and os.path.exists(capture_list_path):
        parsed = parse_capture_list(capture_list_path)
        if parsed:
            return parsed

    if os.path.isdir(ref_dir):
        pngs = sorted(f for f in os.listdir(ref_dir) if f.lower().endswith(".png"))
        return [{"id": os.path.splitext(name)[0], "regions": []} for name in pngs]

    return []


def run_batch(
    capture_list_path: str = DEFAULT_CAPTURE_LIST,
    spec_path: str | None = None,
    ref_dir: str = DEFAULT_REF_DIR,
    craft_dir: str = DEFAULT_CRAFT_DIR,
    out_dir: str = DEFAULT_OUT_DIR,
    require_references: bool = False,
) -> tuple[int, dict[str, Any]]:
    """Run batch UI comparison across capture-list states, skipping cleanly when references are absent."""
    states = _discover_states(capture_list_path, spec_path, ref_dir)

    if not os.path.isdir(ref_dir) or not states:
        missing_target = ref_dir
        notice = (
            f"SKIPPED: reference directory '{missing_target}' is absent or has no reference PNGs."
        )
        missing_refs = [os.path.join(ref_dir, f"{s['id']}.png") for s in states] if states else [ref_dir]
        status = "failed" if require_references else "skipped"
        report = {
            "mode": "batch",
            "status": status,
            "notice": notice,
            "summary": {
                "total_states": len(states),
                "compared_states": 0,
                "passed_states": 0,
                "failed_states": 0,
                "skipped_states": len(states),
                "missing_references": missing_refs,
                "missing_craft": [],
            },
            "thresholds": _base_thresholds_dict(),
            "states": [],
        }
        write_reports(report, out_dir)
        return (1 if require_references else 0), report

    state_results: list[dict[str, Any]] = []
    missing_references: list[str] = []
    missing_craft: list[str] = []
    compared_count = 0
    passed_count = 0
    failed_count = 0
    skipped_count = 0

    for state in states:
        sid = str(state["id"])
        live_path = state.get("live") or os.path.join(ref_dir, f"{sid}.png")
        craft_path = state.get("craft") or os.path.join(craft_dir, f"{sid}.png")

        if not os.path.exists(live_path):
            missing_references.append(live_path)
            skipped_count += 1
            state_results.append(
                {
                    "state_id": sid,
                    "status": "skipped",
                    "passed": True,
                    "reason": f"Missing Live reference file: {live_path}",
                    "live_path": live_path,
                    "craft_path": craft_path,
                    "heatmap_path": None,
                    "regions": [],
                }
            )
            continue

        if not os.path.exists(craft_path):
            missing_craft.append(craft_path)
            skipped_count += 1
            state_results.append(
                {
                    "state_id": sid,
                    "status": "skipped",
                    "passed": True,
                    "reason": f"Missing LiveCraft capture file: {craft_path}",
                    "live_path": live_path,
                    "craft_path": craft_path,
                    "heatmap_path": None,
                    "regions": [],
                }
            )
            continue

        compared_count += 1
        pair_result = compare_image_pair(
            live_path=live_path,
            craft_path=craft_path,
            regions=state.get("regions") or None,
            state_id=sid,
            out_dir=out_dir,
        )
        if pair_result["passed"]:
            passed_count += 1
        else:
            failed_count += 1
        state_results.append(pair_result)

    notice = None
    if compared_count == 0 and missing_references:
        notice = (
            f"SKIPPED: all {len(missing_references)} reference capture(s) are absent in '{ref_dir}'."
        )
    elif missing_references:
        notice = (
            f"SKIPPED {len(missing_references)} missing reference capture(s) in '{ref_dir}'."
        )
    elif compared_count == 0 and missing_craft:
        notice = (
            f"SKIPPED: all {len(missing_craft)} LiveCraft capture(s) are absent in '{craft_dir}'."
        )

    if failed_count > 0:
        overall_status = "failed"
        exit_code = 1
    elif require_references and missing_references:
        overall_status = "failed"
        exit_code = 1
    elif compared_count == 0:
        overall_status = "skipped"
        exit_code = 0
    else:
        overall_status = "passed"
        exit_code = 0

    report = {
        "mode": "batch",
        "status": overall_status,
        "notice": notice,
        "summary": {
            "total_states": len(states),
            "compared_states": compared_count,
            "passed_states": passed_count,
            "failed_states": failed_count,
            "skipped_states": skipped_count,
            "missing_references": missing_references,
            "missing_craft": missing_craft,
        },
        "thresholds": _base_thresholds_dict(),
        "states": state_results,
    }
    write_reports(report, out_dir)
    return exit_code, report


def _print_cli_summary(report: dict[str, Any], out_dir: str) -> None:
    if report.get("notice"):
        print(report["notice"])
    missing_refs = report["summary"].get("missing_references", [])
    if missing_refs:
        listed = ", ".join(missing_refs[:10])
        suffix = f" (+{len(missing_refs) - 10} more)" if len(missing_refs) > 10 else ""
        print(f"Missing references: {listed}{suffix}")
    summary = report["summary"]
    print(
        f"UI diff [{report['status'].upper()}]: "
        f"compared={summary['compared_states']}, "
        f"passed={summary['passed_states']}, "
        f"failed={summary['failed_states']}, "
        f"skipped={summary['skipped_states']} "
        f"(report: {os.path.join(out_dir, 'index.html')})"
    )


def _build_batch_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="ui_diff.py [batch]",
        description="Run region-by-region UI parity comparison in batch mode.",
        epilog=(
            "Single-pair mode: ui_diff.py compare-pair --live <png> --craft <png> "
            "[--spec regions.json] [--out-dir dir]"
        ),
    )
    parser.add_argument("--capture-list", default=DEFAULT_CAPTURE_LIST)
    parser.add_argument("--spec", default=None, help="Optional JSON batch or region spec")
    parser.add_argument("--ref-dir", default=DEFAULT_REF_DIR)
    parser.add_argument("--craft-dir", default=DEFAULT_CRAFT_DIR)
    parser.add_argument("--out-dir", default=DEFAULT_OUT_DIR)
    parser.add_argument(
        "--require-references",
        action="store_true",
        help="Fail with non-zero exit code if reference captures are absent",
    )
    return parser


def _build_compare_pair_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="ui_diff.py compare-pair",
        description="Compare one Live reference PNG and one LiveCraft PNG.",
    )
    parser.add_argument("--live", required=True, help="Path to Live reference PNG")
    parser.add_argument("--craft", required=True, help="Path to LiveCraft PNG")
    parser.add_argument("--spec", default=None, help="Optional JSON region spec")
    parser.add_argument("--state-id", default=None, help="Optional state identifier")
    parser.add_argument("--out-dir", default=DEFAULT_OUT_DIR)
    return parser


def main(argv: list[str] | None = None) -> int:
    """CLI entrypoint supporting ``batch`` and ``compare-pair`` modes."""
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")

    raw_args = list(sys.argv[1:] if argv is None else argv)

    try:
        if raw_args and raw_args[0] == "compare-pair":
            args = _build_compare_pair_parser().parse_args(raw_args[1:])
            code, report = run_compare_pair(
                live_path=args.live,
                craft_path=args.craft,
                spec_path=args.spec,
                out_dir=args.out_dir,
                state_id=args.state_id,
            )
            _print_cli_summary(report, args.out_dir)
            return code

        if raw_args and raw_args[0] == "batch":
            args = _build_batch_parser().parse_args(raw_args[1:])
            code, report = run_batch(
                capture_list_path=args.capture_list,
                spec_path=args.spec,
                ref_dir=args.ref_dir,
                craft_dir=args.craft_dir,
                out_dir=args.out_dir,
                require_references=args.require_references,
            )
            _print_cli_summary(report, args.out_dir)
            return code

        if "--live" in raw_args or "--craft" in raw_args:
            args = _build_compare_pair_parser().parse_args(raw_args)
            code, report = run_compare_pair(
                live_path=args.live,
                craft_path=args.craft,
                spec_path=args.spec,
                out_dir=args.out_dir,
                state_id=args.state_id,
            )
            _print_cli_summary(report, args.out_dir)
            return code

        args = _build_batch_parser().parse_args(raw_args)
        code, report = run_batch(
            capture_list_path=args.capture_list,
            spec_path=args.spec,
            ref_dir=args.ref_dir,
            craft_dir=args.craft_dir,
            out_dir=args.out_dir,
            require_references=args.require_references,
        )
        _print_cli_summary(report, args.out_dir)
        return code
    except (FileNotFoundError, ValueError, json.JSONDecodeError) as exc:
        print(f"Error: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())

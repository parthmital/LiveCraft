"""Synthetic-image unit and CLI tests for ``ui_measure.py`` and ``ui_diff.py``."""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import unittest

import numpy as np
from PIL import Image, ImageDraw, ImageFilter

TOOLS_DIR = os.path.dirname(os.path.abspath(__file__))
if TOOLS_DIR not in sys.path:
    sys.path.insert(0, TOOLS_DIR)

import ui_diff  # noqa: E402
import ui_measure  # noqa: E402

UI_MEASURE_PATH = os.path.join(TOOLS_DIR, "ui_measure.py")
UI_DIFF_PATH = os.path.join(TOOLS_DIR, "ui_diff.py")


def _save_rgb_image(path: str, array: np.ndarray) -> None:
    Image.fromarray(array.astype(np.uint8), mode="RGB").save(path)


def _make_synthetic_text_strip(
    width: int = 120,
    height: int = 40,
    offset_x: int = 0,
    offset_y: int = 0,
    bg: tuple[int, int, int] = (28, 30, 34),
    fg: tuple[int, int, int] = (225, 228, 232),
) -> np.ndarray:
    """Create deterministic crisp 1-pixel-accurate synthetic glyph strokes."""
    img = Image.new("RGB", (width, height), bg)
    draw = ImageDraw.Draw(img)

    x0 = 20 + offset_x
    y0 = 12 + offset_y

    # Glyph 1: 'H' (width 7, height 11)
    draw.rectangle([x0, y0, x0 + 1, y0 + 10], fill=fg)
    draw.rectangle([x0 + 5, y0, x0 + 6, y0 + 10], fill=fg)
    draw.rectangle([x0 + 2, y0 + 4, x0 + 4, y0 + 5], fill=fg)

    # Glyph 2: 'I' (width 4, height 11)
    gx = x0 + 11
    draw.rectangle([gx, y0, gx + 3, y0 + 1], fill=fg)
    draw.rectangle([gx + 1, y0 + 2, gx + 2, y0 + 8], fill=fg)
    draw.rectangle([gx, y0 + 9, gx + 3, y0 + 10], fill=fg)

    # Glyph 3: '1' + horizontal baseline tick
    gx2 = x0 + 19
    draw.rectangle([gx2 + 2, y0, gx2 + 3, y0 + 10], fill=fg)
    draw.rectangle([gx2, y0 + 2, gx2 + 1, y0 + 3], fill=fg)
    draw.rectangle([gx2, y0 + 9, gx2 + 5, y0 + 10], fill=fg)

    # Glyph 4: '0'
    gx3 = x0 + 29
    draw.rectangle([gx3, y0, gx3 + 5, y0 + 10], fill=fg)
    draw.rectangle([gx3 + 2, y0 + 2, gx3 + 3, y0 + 8], fill=bg)

    return np.array(img, dtype=np.uint8)


class TestUiMeasure(unittest.TestCase):
    def test_color_probe_exact_rgb_and_cli(self) -> None:
        with tempfile.TemporaryDirectory() as work_dir:
            img = Image.new("RGB", (64, 64), (30, 31, 34))
            draw = ImageDraw.Draw(img)
            draw.rectangle([10, 10, 19, 19], fill=(255, 136, 0))
            draw.rectangle([30, 20, 39, 29], fill=(64, 180, 220))
            img_path = os.path.join(work_dir, "sample.png")
            out_path = os.path.join(work_dir, "colors.json")
            img.save(img_path)

            arr = ui_measure.load_rgb_array(img_path)
            self.assertEqual(ui_measure.sample_color(arr, 15, 15), [255, 136, 0])
            self.assertEqual(ui_measure.sample_color(arr, 0, 0), [30, 31, 34])

            proc = subprocess.run(
                [
                    sys.executable,
                    UI_MEASURE_PATH,
                    "color",
                    img_path,
                    "--points",
                    "15,15",
                    "35,25",
                    "-o",
                    out_path,
                ],
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(proc.returncode, 0, msg=proc.stderr)
            with open(out_path, "r", encoding="utf-8") as handle:
                payload = json.load(handle)
            self.assertEqual(payload["colors"][0]["rgb"], [255, 136, 0])
            self.assertEqual(payload["colors"][0]["hex"], "#ff8800")
            self.assertEqual(payload["colors"][1]["rgb"], [64, 180, 220])

    def test_runs_horizontal_and_vertical_borders_and_gaps(self) -> None:
        with tempfile.TemporaryDirectory() as work_dir:
            arr = np.full((40, 50, 3), (20, 20, 20), dtype=np.uint8)
            # Horizontal bands along y=10: 10px gap, 2px border, 18px body, 20px tail
            arr[10, 10:12] = [200, 200, 200]
            arr[10, 12:30] = [45, 50, 60]
            # Vertical bands along x=5: 6px gap, 3px border, 31px tail
            arr[6:9, 5] = [255, 140, 0]

            img_path = os.path.join(work_dir, "runs.png")
            _save_rgb_image(img_path, arr)

            loaded = ui_measure.load_rgb_array(img_path)
            axis_h, segs_h = ui_measure.measure_runs(loaded, 0, 10, 49, 10)
            self.assertEqual(axis_h, "horizontal")
            self.assertEqual(
                segs_h,
                [
                    ([20, 20, 20], 0, 10),
                    ([200, 200, 200], 10, 2),
                    ([45, 50, 60], 12, 18),
                    ([20, 20, 20], 30, 20),
                ],
            )

            out_path = os.path.join(work_dir, "runs.json")
            proc = subprocess.run(
                [
                    sys.executable,
                    UI_MEASURE_PATH,
                    "runs",
                    img_path,
                    "--line",
                    "5,0,5,39",
                    "-o",
                    out_path,
                ],
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(proc.returncode, 0, msg=proc.stderr)
            with open(out_path, "r", encoding="utf-8") as handle:
                payload = json.load(handle)
            self.assertEqual(payload["axis"], "vertical")
            self.assertEqual(
                payload["segments"],
                [
                    [[20, 20, 20], 0, 6],
                    [[255, 140, 0], 6, 3],
                    [[20, 20, 20], 9, 31],
                ],
            )

    def test_text_bounds_tight_ink_bbox_and_blank_region(self) -> None:
        with tempfile.TemporaryDirectory() as work_dir:
            arr = _make_synthetic_text_strip(width=120, height=50, offset_x=0, offset_y=0)
            # 'H' starts at x=20, '0' ends at x=20+29+5=54 (inclusive) -> width 35
            # y spans y=12..22 inclusive -> height 11
            img_path = os.path.join(work_dir, "text.png")
            out_path = os.path.join(work_dir, "text_bounds.json")
            _save_rgb_image(img_path, arr)

            proc = subprocess.run(
                [
                    sys.executable,
                    UI_MEASURE_PATH,
                    "text-bounds",
                    img_path,
                    "--rect",
                    "10,5,80,35",
                    "-o",
                    out_path,
                ],
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(proc.returncode, 0, msg=proc.stderr)
            with open(out_path, "r", encoding="utf-8") as handle:
                payload = json.load(handle)
            self.assertEqual(payload["bbox"], [20, 12, 35, 11])
            self.assertEqual(payload["relative_bbox"], [10, 7, 35, 11])
            self.assertGreater(payload["ink_pixel_count"], 0)

            blank = ui_measure.measure_text_bounds(arr, 80, 0, 30, 20)
            self.assertIsNone(blank["bbox"])
            self.assertEqual(blank["ink_pixel_count"], 0)

    def test_spec_mode_cli_writes_combined_json(self) -> None:
        with tempfile.TemporaryDirectory() as work_dir:
            arr = _make_synthetic_text_strip(width=100, height=40)
            arr[0:2, :] = [90, 95, 100]
            img_path = os.path.join(work_dir, "panel.png")
            spec_path = os.path.join(work_dir, "spec.json")
            out_path = os.path.join(work_dir, "measured.json")
            _save_rgb_image(img_path, arr)

            spec = {
                "colors": [{"name": "top_border", "x": 10, "y": 1}],
                "runs": [{"name": "left_edge", "line": [5, 0, 5, 10]}],
                "text_bounds": [{"name": "label", "rect": [15, 8, 60, 20]}],
            }
            with open(spec_path, "w", encoding="utf-8") as handle:
                json.dump(spec, handle)

            proc = subprocess.run(
                [
                    sys.executable,
                    UI_MEASURE_PATH,
                    "--image",
                    img_path,
                    "--spec",
                    spec_path,
                    "-o",
                    out_path,
                ],
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(proc.returncode, 0, msg=proc.stderr)
            with open(out_path, "r", encoding="utf-8") as handle:
                measured = json.load(handle)
            self.assertEqual(measured["colors"][0]["rgb"], [90, 95, 100])
            self.assertEqual(measured["runs"][0]["segments"][0], [[90, 95, 100], 0, 2])
            self.assertEqual(measured["text_bounds"][0]["bbox"], [20, 12, 35, 11])

    def test_rejects_out_of_bounds_and_diagonal_scanlines(self) -> None:
        arr = np.zeros((20, 20, 3), dtype=np.uint8)
        with self.assertRaises(ValueError):
            ui_measure.sample_color(arr, 20, 5)
        with self.assertRaises(ValueError):
            ui_measure.measure_runs(arr, 0, 0, 10, 10)
        with self.assertRaises(ValueError):
            ui_measure.measure_text_bounds(arr, 15, 0, 10, 10)


class TestUiDiff(unittest.TestCase):
    def test_identical_images_pass_all_thresholds(self) -> None:
        with tempfile.TemporaryDirectory() as work_dir:
            arr = _make_synthetic_text_strip(width=120, height=60)
            arr[42:56, 10:110] = [55, 60, 68]
            live_path = os.path.join(work_dir, "live.png")
            craft_path = os.path.join(work_dir, "craft.png")
            _save_rgb_image(live_path, arr)
            _save_rgb_image(craft_path, arr)

            regions = [
                {"name": "label", "rect": [10, 5, 65, 28], "kind": "text"},
                {"name": "solid_bar", "rect": [12, 44, 90, 10], "kind": "solid"},
            ]
            result = ui_diff.compare_image_pair(live_path, craft_path, regions=regions)
            self.assertTrue(result["passed"])
            for reg in result["regions"]:
                self.assertEqual(reg["checks"]["ssim"]["score"], 1.0)
                self.assertTrue(reg["passed"])
            self.assertEqual(result["regions"][0]["checks"]["text_alignment"]["col_shift_px"], 0)
            self.assertEqual(result["regions"][0]["checks"]["text_alignment"]["row_shift_px"], 0)
            self.assertEqual(result["regions"][1]["checks"]["solid_color"]["max_channel_delta"], 0)

    def test_shifted_box_threshold_boundary(self) -> None:
        with tempfile.TemporaryDirectory() as work_dir:
            base = np.full((64, 80, 3), (25, 26, 30), dtype=np.uint8)
            base[16:40, 20:48] = [210, 215, 220]

            shift_1px = np.full((64, 80, 3), (25, 26, 30), dtype=np.uint8)
            shift_1px[16:40, 21:49] = [210, 215, 220]

            shift_2px_x = np.full((64, 80, 3), (25, 26, 30), dtype=np.uint8)
            shift_2px_x[16:40, 22:50] = [210, 215, 220]

            shift_2px_y = np.full((64, 80, 3), (25, 26, 30), dtype=np.uint8)
            shift_2px_y[18:42, 20:48] = [210, 215, 220]

            bbox_1px = ui_diff.compute_edge_bbox_diff(base, shift_1px)
            self.assertTrue(bbox_1px["passed"])
            self.assertEqual(bbox_1px["max_edge_delta_px"], 1)

            bbox_2px_x = ui_diff.compute_edge_bbox_diff(base, shift_2px_x)
            self.assertFalse(bbox_2px_x["passed"])
            self.assertEqual(bbox_2px_x["max_edge_delta_px"], 2)

            bbox_2px_y = ui_diff.compute_edge_bbox_diff(base, shift_2px_y)
            self.assertFalse(bbox_2px_y["passed"])
            self.assertEqual(bbox_2px_y["max_edge_delta_px"], 2)

            live_p = os.path.join(work_dir, "live.png")
            craft_p = os.path.join(work_dir, "craft_2px.png")
            _save_rgb_image(live_p, base)
            _save_rgb_image(craft_p, shift_2px_x)
            pair_res = ui_diff.compare_image_pair(
                live_p, craft_p, regions=[{"name": "box", "rect": [0, 0, 80, 64], "kind": "box"}]
            )
            self.assertFalse(pair_res["passed"])
            self.assertFalse(pair_res["regions"][0]["checks"]["bbox"]["passed"])

    def test_solid_color_delta_threshold(self) -> None:
        with tempfile.TemporaryDirectory() as work_dir:
            ref = np.full((48, 48, 3), (40, 50, 60), dtype=np.uint8)
            within_2 = np.full((48, 48, 3), (42, 48, 62), dtype=np.uint8)
            off_by_3 = np.full((48, 48, 3), (43, 50, 60), dtype=np.uint8)

            live_p = os.path.join(work_dir, "live.png")
            ok_p = os.path.join(work_dir, "ok.png")
            bad_p = os.path.join(work_dir, "bad.png")
            _save_rgb_image(live_p, ref)
            _save_rgb_image(ok_p, within_2)
            _save_rgb_image(bad_p, off_by_3)

            res_ok = ui_diff.compare_image_pair(live_p, ok_p)
            self.assertTrue(res_ok["passed"])
            self.assertEqual(res_ok["regions"][0]["checks"]["solid_color"]["max_channel_delta"], 2)

            res_bad = ui_diff.compare_image_pair(live_p, bad_p)
            self.assertFalse(res_bad["passed"])
            self.assertFalse(res_bad["regions"][0]["checks"]["solid_color"]["passed"])
            self.assertEqual(res_bad["regions"][0]["checks"]["solid_color"]["max_channel_delta"], 3)

    def test_gaussian_blur_fails_ssim_7x7_gaussian(self) -> None:
        with tempfile.TemporaryDirectory() as work_dir:
            arr = _make_synthetic_text_strip(width=96, height=48)
            # Add sharp grid pattern so blur significantly degrades structural detail
            for x in range(6, 90, 6):
                arr[32:44, x : x + 2] = [240, 140, 40]
            sharp_img = Image.fromarray(arr, mode="RGB")
            blurred_img = sharp_img.filter(ImageFilter.GaussianBlur(radius=2.0))

            blurred_arr = np.asarray(blurred_img, dtype=np.uint8)
            ssim_val = ui_diff.compute_ssim_7x7_gaussian(arr, blurred_arr)
            self.assertLess(ssim_val, 0.97)

            live_p = os.path.join(work_dir, "sharp.png")
            blur_p = os.path.join(work_dir, "blurred.png")
            sharp_img.save(live_p)
            blurred_img.save(blur_p)

            res = ui_diff.compare_image_pair(
                live_p,
                blur_p,
                regions=[{"name": "panel", "rect": [0, 0, 96, 48], "kind": "panel"}],
            )
            self.assertFalse(res["passed"])
            self.assertFalse(res["regions"][0]["checks"]["ssim"]["passed"])
            self.assertLess(res["regions"][0]["checks"]["ssim"]["score"], 0.97)

    def test_text_kind_uses_0_95_ssim_threshold(self) -> None:
        base = _make_synthetic_text_strip(width=96, height=40)
        # Mild anti-aliasing blur tuned so SSIM lands in (0.95, 0.97)
        softened = np.array(
            Image.fromarray(base, mode="RGB").filter(ImageFilter.GaussianBlur(radius=0.8)),
            dtype=np.uint8,
        )
        ssim_score = ui_diff.compute_ssim_7x7_gaussian(base, softened)
        self.assertGreater(ssim_score, 0.95)
        self.assertLess(ssim_score, 0.97)

        res_text = ui_diff.compare_region(base, softened, {"name": "lbl", "kind": "text"})
        self.assertEqual(res_text["checks"]["ssim"]["threshold"], 0.95)
        self.assertTrue(res_text["checks"]["ssim"]["passed"])

        res_panel = ui_diff.compare_region(base, softened, {"name": "pnl", "kind": "panel"})
        self.assertEqual(res_panel["checks"]["ssim"]["threshold"], 0.97)
        self.assertFalse(res_panel["checks"]["ssim"]["passed"])

    def test_text_profile_alignment_horizontal_and_vertical_shift(self) -> None:
        with tempfile.TemporaryDirectory() as work_dir:
            ref = _make_synthetic_text_strip(width=120, height=40, offset_x=0, offset_y=0)
            shift_x1 = _make_synthetic_text_strip(width=120, height=40, offset_x=1, offset_y=0)
            shift_x2 = _make_synthetic_text_strip(width=120, height=40, offset_x=2, offset_y=0)
            shift_y2 = _make_synthetic_text_strip(width=120, height=40, offset_x=0, offset_y=2)

            al_x1 = ui_diff.compute_text_alignment_diff(ref, shift_x1, force=True)
            self.assertTrue(al_x1["passed"])
            self.assertEqual(al_x1["col_shift_px"], 1)
            self.assertEqual(al_x1["row_shift_px"], 0)

            al_x2 = ui_diff.compute_text_alignment_diff(ref, shift_x2, force=True)
            self.assertFalse(al_x2["passed"])
            self.assertEqual(al_x2["col_shift_px"], 2)

            al_y2 = ui_diff.compute_text_alignment_diff(ref, shift_y2, force=True)
            self.assertFalse(al_y2["passed"])
            self.assertEqual(al_y2["row_shift_px"], 2)

            live_p = os.path.join(work_dir, "text_live.png")
            craft_p = os.path.join(work_dir, "text_shift2.png")
            _save_rgb_image(live_p, ref)
            _save_rgb_image(craft_p, shift_x2)
            pair_res = ui_diff.compare_image_pair(
                live_p,
                craft_p,
                regions=[{"name": "txt", "rect": [0, 0, 120, 40], "kind": "text"}],
            )
            self.assertFalse(pair_res["passed"])
            self.assertFalse(pair_res["regions"][0]["checks"]["text_alignment"]["passed"])

    def test_parse_capture_list_markdown_formats(self) -> None:
        with tempfile.TemporaryDirectory() as work_dir:
            md_path = os.path.join(work_dir, "capture-list.md")
            with open(md_path, "w", encoding="utf-8") as handle:
                handle.write(
                    "# Capture List\n\n"
                    "| State ID | Fixture | Steps | Regions |\n"
                    "| --- | --- | --- | --- |\n"
                    "| `control-bar__stopped__dark` | `empty.als` | Launch | "
                    "`control_bar: 0,0,1920,40 (panel); bpm: 120,4,60,24 (text)` |\n\n"
                    "### `session__empty__light`\n\n"
                    "| Region | Rect | Kind |\n"
                    "| --- | --- | --- |\n"
                    "| bg_fill | 0,40,400,300 | solid |\n"
                )

            states = ui_diff.parse_capture_list(md_path)
            self.assertEqual(len(states), 2)
            self.assertEqual(states[0]["id"], "control-bar__stopped__dark")
            self.assertEqual(
                states[0]["regions"],
                [
                    {"name": "control_bar", "rect": [0, 0, 1920, 40], "kind": "panel"},
                    {"name": "bpm", "rect": [120, 4, 60, 24], "kind": "text"},
                ],
            )
            self.assertEqual(states[1]["id"], "session__empty__light")
            self.assertEqual(
                states[1]["regions"],
                [{"name": "bg_fill", "rect": [0, 40, 400, 300], "kind": "solid"}],
            )

    def test_compare_pair_cli_writes_html_json_and_heatmap_and_exit_codes(self) -> None:
        with tempfile.TemporaryDirectory() as work_dir:
            base = np.full((64, 80, 3), (30, 30, 34), dtype=np.uint8)
            base[12:36, 15:45] = [220, 140, 40]
            shifted = np.full((64, 80, 3), (30, 30, 34), dtype=np.uint8)
            shifted[12:36, 18:48] = [220, 140, 40]

            live_p = os.path.join(work_dir, "live.png")
            same_p = os.path.join(work_dir, "same.png")
            bad_p = os.path.join(work_dir, "bad.png")
            spec_p = os.path.join(work_dir, "spec.json")
            out_pass = os.path.join(work_dir, "out_pass")
            out_fail = os.path.join(work_dir, "out_fail")

            _save_rgb_image(live_p, base)
            _save_rgb_image(same_p, base)
            _save_rgb_image(bad_p, shifted)
            with open(spec_p, "w", encoding="utf-8") as handle:
                json.dump({"regions": [{"name": "box", "rect": [0, 0, 80, 64], "kind": "box"}]}, handle)

            pass_proc = subprocess.run(
                [
                    sys.executable,
                    UI_DIFF_PATH,
                    "compare-pair",
                    "--live",
                    live_p,
                    "--craft",
                    same_p,
                    "--spec",
                    spec_p,
                    "--state-id",
                    "test__ok__dark",
                    "--out-dir",
                    out_pass,
                ],
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(pass_proc.returncode, 0, msg=pass_proc.stdout + pass_proc.stderr)
            self.assertTrue(os.path.exists(os.path.join(out_pass, "report.json")))
            self.assertTrue(os.path.exists(os.path.join(out_pass, "index.html")))
            self.assertTrue(
                os.path.exists(os.path.join(out_pass, "assets", "test__ok__dark_heatmap.png"))
            )

            fail_proc = subprocess.run(
                [
                    sys.executable,
                    UI_DIFF_PATH,
                    "compare-pair",
                    "--live",
                    live_p,
                    "--craft",
                    bad_p,
                    "--spec",
                    spec_p,
                    "--state-id",
                    "test__bad__dark",
                    "--out-dir",
                    out_fail,
                ],
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertNotEqual(fail_proc.returncode, 0)
            with open(os.path.join(out_fail, "report.json"), "r", encoding="utf-8") as handle:
                fail_report = json.load(handle)
            self.assertEqual(fail_report["status"], "failed")
            self.assertTrue(os.path.exists(os.path.join(out_fail, "index.html")))

    def test_batch_missing_reference_dir_skips_cleanly_unless_required(self) -> None:
        with tempfile.TemporaryDirectory() as work_dir:
            md_path = os.path.join(work_dir, "capture-list.md")
            with open(md_path, "w", encoding="utf-8") as handle:
                handle.write(
                    "| State ID | Regions |\n"
                    "| --- | --- |\n"
                    "| `control-bar__stopped__dark` | `bar: 0,0,64,32 (panel)` |\n"
                )
            missing_ref_dir = os.path.join(work_dir, "missing_ref")
            craft_dir = os.path.join(work_dir, "craft")
            out_dir = os.path.join(work_dir, "out_skip")

            proc_skip = subprocess.run(
                [
                    sys.executable,
                    UI_DIFF_PATH,
                    "batch",
                    "--capture-list",
                    md_path,
                    "--ref-dir",
                    missing_ref_dir,
                    "--craft-dir",
                    craft_dir,
                    "--out-dir",
                    out_dir,
                ],
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(proc_skip.returncode, 0, msg=proc_skip.stdout + proc_skip.stderr)
            self.assertIn("SKIPPED", proc_skip.stdout)
            self.assertTrue(os.path.exists(os.path.join(out_dir, "report.json")))
            self.assertTrue(os.path.exists(os.path.join(out_dir, "index.html")))
            with open(os.path.join(out_dir, "report.json"), "r", encoding="utf-8") as handle:
                skip_report = json.load(handle)
            self.assertEqual(skip_report["status"], "skipped")

            out_req = os.path.join(work_dir, "out_required")
            proc_req = subprocess.run(
                [
                    sys.executable,
                    UI_DIFF_PATH,
                    "batch",
                    "--capture-list",
                    md_path,
                    "--ref-dir",
                    missing_ref_dir,
                    "--craft-dir",
                    craft_dir,
                    "--out-dir",
                    out_req,
                    "--require-references",
                ],
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertNotEqual(proc_req.returncode, 0)
            with open(os.path.join(out_req, "report.json"), "r", encoding="utf-8") as handle:
                req_report = json.load(handle)
            self.assertEqual(req_report["status"], "failed")

    def test_batch_present_references_fail_nonzero_on_threshold_violation(self) -> None:
        with tempfile.TemporaryDirectory() as work_dir:
            md_path = os.path.join(work_dir, "capture-list.md")
            with open(md_path, "w", encoding="utf-8") as handle:
                handle.write(
                    "| State ID | Regions |\n"
                    "| --- | --- |\n"
                    "| `mixer__default__dark` | `bg: 0,0,48,48 (solid)` |\n"
                )
            ref_dir = os.path.join(work_dir, "ref")
            craft_dir = os.path.join(work_dir, "craft")
            out_dir = os.path.join(work_dir, "out")
            os.makedirs(ref_dir)
            os.makedirs(craft_dir)

            _save_rgb_image(
                os.path.join(ref_dir, "mixer__default__dark.png"),
                np.full((48, 48, 3), (35, 35, 38), dtype=np.uint8),
            )
            _save_rgb_image(
                os.path.join(craft_dir, "mixer__default__dark.png"),
                np.full((48, 48, 3), (40, 35, 38), dtype=np.uint8),
            )

            proc = subprocess.run(
                [
                    sys.executable,
                    UI_DIFF_PATH,
                    "--capture-list",
                    md_path,
                    "--ref-dir",
                    ref_dir,
                    "--craft-dir",
                    craft_dir,
                    "--out-dir",
                    out_dir,
                ],
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertNotEqual(proc.returncode, 0)
            with open(os.path.join(out_dir, "report.json"), "r", encoding="utf-8") as handle:
                report = json.load(handle)
            self.assertEqual(report["status"], "failed")
            self.assertEqual(report["summary"]["failed_states"], 1)
            self.assertEqual(report["summary"]["compared_states"], 1)

    def test_implicit_compare_pair_and_dimension_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as work_dir:
            live_p = os.path.join(work_dir, "live.png")
            craft_ok = os.path.join(work_dir, "craft_ok.png")
            craft_bad_size = os.path.join(work_dir, "craft_bad_size.png")
            out_ok = os.path.join(work_dir, "out_ok")
            out_bad = os.path.join(work_dir, "out_bad")

            arr = np.full((32, 32, 3), (40, 40, 40), dtype=np.uint8)
            _save_rgb_image(live_p, arr)
            _save_rgb_image(craft_ok, arr)
            _save_rgb_image(craft_bad_size, np.full((32, 40, 3), (40, 40, 40), dtype=np.uint8))

            ok_proc = subprocess.run(
                [
                    sys.executable,
                    UI_DIFF_PATH,
                    "--live",
                    live_p,
                    "--craft",
                    craft_ok,
                    "--out-dir",
                    out_ok,
                ],
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(ok_proc.returncode, 0, msg=ok_proc.stdout + ok_proc.stderr)

            mismatch_proc = subprocess.run(
                [
                    sys.executable,
                    UI_DIFF_PATH,
                    "--live",
                    live_p,
                    "--craft",
                    craft_bad_size,
                    "--out-dir",
                    out_bad,
                ],
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertNotEqual(mismatch_proc.returncode, 0)
            with open(os.path.join(out_bad, "report.json"), "r", encoding="utf-8") as handle:
                report = json.load(handle)
            self.assertEqual(report["status"], "failed")
            self.assertIn("Dimension mismatch", report["states"][0]["error"])


if __name__ == "__main__":
    unittest.main()

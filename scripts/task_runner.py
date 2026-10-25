#!/usr/bin/env python3
"""
LiveCraft Autonomous Task Runner & Progress Orchestrator

Purpose:
  Manages, selects, verifies, and records the sequential execution of
  granular implementation tasks for the LiveCraft DAW. Designed specifically
  for reliable execution by Gemini in Google Antigravity.

Usage:
  python scripts/task_runner.py status          # View overall progress and phase breakdown
  python scripts/task_runner.py next            # Get the next executable task and instructions
  python scripts/task_runner.py show <id>       # Display the full specification for a task
  python scripts/task_runner.py verify <id>     # Run automated verification checks for a task
  python scripts/task_runner.py complete <id>   # Mark a verified task as completed and advance
  python scripts/task_runner.py fail <id>       # Mark a task as failed with diagnostic notes
  python scripts/task_runner.py reset           # Reset task progress (warning: clears state)
"""

import os
import sys
import json
import subprocess
import argparse
import time
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
TASKS_DIR = REPO_ROOT / "tasks"
SPECS_DIR = TASKS_DIR / "specs"
MANIFEST_PATH = TASKS_DIR / "manifest.json"
STATE_PATH = TASKS_DIR / "state.json"


def load_manifest():
    if not MANIFEST_PATH.exists():
        print(f"Error: Manifest not found at {MANIFEST_PATH}", file=sys.stderr)
        sys.exit(1)
    with open(MANIFEST_PATH, "r", encoding="utf-8") as f:
        return json.load(f)


def load_state():
    if not STATE_PATH.exists():
        state = {
            "current_task_id": None,
            "completed_tasks": [],
            "failed_tasks": {},
            "task_history": [],
            "last_updated": time.strftime("%Y-%m-%dT%H:%M:%S"),
        }
        save_state(state)
        return state
    with open(STATE_PATH, "r", encoding="utf-8") as f:
        return json.load(f)


def save_state(state):
    state["last_updated"] = time.strftime("%Y-%m-%dT%H:%M:%S")
    with open(STATE_PATH, "w", encoding="utf-8") as f:
        json.dump(state, f, indent=2)


def save_manifest(manifest):
    with open(MANIFEST_PATH, "w", encoding="utf-8") as f:
        json.dump(manifest, f, indent=2)


def get_next_task(manifest, state):
    completed = set(state.get("completed_tasks", []))
    for task in manifest.get("tasks", []):
        task_id = task["id"]
        if task_id not in completed:
            # Check dependencies
            deps = task.get("dependencies", [])
            deps_met = all(dep in completed for dep in deps)
            if deps_met:
                return task
            else:
                unmet = [dep for dep in deps if dep not in completed]
                return {"blocked": True, "task": task, "unmet_dependencies": unmet}
    return None


def cmd_status(args):
    manifest = load_manifest()
    state = load_state()
    tasks = manifest.get("tasks", [])
    completed = set(state.get("completed_tasks", []))
    total = len(tasks)
    num_done = len(completed)
    percent = (num_done / total * 100.0) if total > 0 else 0.0

    print("=" * 78)
    print(
        f" LIVECRAFT AUTONOMOUS IMPLEMENTATION PROGRESS: {num_done}/{total} ({percent:.1f}%)"
    )
    print("=" * 78)

    bar_len = 50
    filled = int(bar_len * (num_done / total)) if total > 0 else 0
    bar = "#" * filled + "-" * (bar_len - filled)
    print(f" Progress: [{bar}] {percent:.1f}%\n")

    current_phase = None
    for task in tasks:
        phase = task.get("phase", "General")
        if phase != current_phase:
            current_phase = phase
            print(f"\n--- {current_phase.upper()} ---")

        task_id = task["id"]
        title = task["title"]
        if task_id in completed:
            badge = "[DONE]    "
        elif task_id in state.get("failed_tasks", {}):
            badge = "[FAILED]  "
        elif state.get("current_task_id") == task_id:
            badge = "[ACTIVE]  "
        else:
            badge = "[PENDING] "

        print(f" {badge} {task_id: <32} : {title}")

    next_t = get_next_task(manifest, state)
    print("\n" + "=" * 78)
    if not next_t:
        print(" ALL IMPLEMENTATION TASKS ARE COMPLETED! SYSTEM FULLY VERIFIED.")
    elif "blocked" in next_t:
        print(
            f" Next task {next_t['task']['id']} is BLOCKED by unmet dependencies: {next_t['unmet_dependencies']}"
        )
    else:
        print(f" Next Executable Task: {next_t['id']} - {next_t['title']}")
        print(
            f" Run 'python scripts/task_runner.py next' for explicit task instructions."
        )
    print("=" * 78)


def cmd_next(args):
    manifest = load_manifest()
    state = load_state()
    next_t = get_next_task(manifest, state)

    if not next_t:
        print("ALL_TASKS_COMPLETED")
        print(
            "\nAll tasks in tasks/manifest.json have been successfully executed and verified!"
        )
        return

    if "blocked" in next_t:
        print("ERROR_BLOCKED")
        print(
            f"Task {next_t['task']['id']} is blocked. Unmet dependencies: {next_t['unmet_dependencies']}"
        )
        sys.exit(1)

    task_id = next_t["id"]
    state["current_task_id"] = task_id
    save_state(state)

    spec_file = SPECS_DIR / f"{task_id}.md"
    print("=" * 78)
    print(f" ACTIVE TASK: {task_id}")
    print(f" Title:       {next_t['title']}")
    print(f" Phase:       {next_t.get('phase', 'General')}")
    print(f" Spec File:   {spec_file}")
    print(f" Verify Cmd:  {next_t.get('verification_command', 'cargo test')}")
    print("=" * 78)

    if spec_file.exists():
        print(f"\n--- TASK SPECIFICATION ({spec_file.name}) ---\n")
        with open(spec_file, "r", encoding="utf-8") as f:
            print(f.read())
        print("\n" + "=" * 78)
    else:
        print(
            f"Warning: Specification file {spec_file} does not exist yet.",
            file=sys.stderr,
        )

    print("\nINSTRUCTIONS FOR GEMINI 3.8 FLASH HIGH:")
    print(
        "1. Read ONLY the files listed in 'Files to Inspect' in the specification above."
    )
    print(
        "2. Modify ONLY the files listed in 'Files to Modify'. Do NOT touch forbidden files."
    )
    print("3. Execute the implementation changes.")
    print(f"4. Run: python scripts/task_runner.py verify {task_id}")
    print(f"5. If tests pass, run: python scripts/task_runner.py complete {task_id}")
    print(
        "6. If verification fails, consult the 'Failure Handling Protocol' in the spec."
    )


def cmd_show(args):
    task_id = args.task_id
    spec_file = SPECS_DIR / f"{task_id}.md"
    if not spec_file.exists():
        print(f"Error: Specification file {spec_file} not found.", file=sys.stderr)
        sys.exit(1)
    with open(spec_file, "r", encoding="utf-8") as f:
        print(f.read())


def cmd_verify(args):
    task_id = args.task_id
    manifest = load_manifest()
    task = next((t for t in manifest.get("tasks", []) if t["id"] == task_id), None)
    if not task:
        print(f"Error: Task '{task_id}' not found in manifest.", file=sys.stderr)
        sys.exit(1)

    verify_cmd = task.get("verification_command", "cargo test")
    print(f"Running automated verification for {task_id}...")
    print(f"Command: {verify_cmd}\n")

    start_time = time.time()
    res = subprocess.run(verify_cmd, shell=True, cwd=REPO_ROOT)
    duration = time.time() - start_time

    if res.returncode == 0:
        print("\n" + "=" * 78)
        print(f" VERIFICATION SUCCESSFUL for {task_id} (elapsed: {duration:.2f}s)")
        print(
            f" Next step: Run 'python scripts/task_runner.py complete {task_id}' to finalize."
        )
        print("=" * 78)
        sys.exit(0)
    else:
        print("\n" + "=" * 78)
        print(
            f" VERIFICATION FAILED for {task_id} (exit code: {res.returncode})",
            file=sys.stderr,
        )
        print(
            f" Review compiler or test errors above. Do NOT mark task complete.",
            file=sys.stderr,
        )
        print("=" * 78)
        sys.exit(res.returncode)


def cmd_complete(args):
    task_id = args.task_id
    manifest = load_manifest()
    state = load_state()

    task = next((t for t in manifest.get("tasks", []) if t["id"] == task_id), None)
    if not task:
        print(f"Error: Task '{task_id}' not found in manifest.", file=sys.stderr)
        sys.exit(1)

    # Automatically verify before completing unless --skip-verify is explicitly set
    if not args.skip_verify:
        verify_cmd = task.get("verification_command", "cargo test")
        print(f"Executing pre-completion verification: {verify_cmd}")
        res = subprocess.run(verify_cmd, shell=True, cwd=REPO_ROOT)
        if res.returncode != 0:
            print(
                f"\nError: Pre-completion verification failed for {task_id}.",
                file=sys.stderr,
            )
            print("Fix all errors before completing this task.", file=sys.stderr)
            sys.exit(res.returncode)

    completed = state.setdefault("completed_tasks", [])
    if task_id not in completed:
        completed.append(task_id)

    if task_id in state.get("failed_tasks", {}):
        del state["failed_tasks"][task_id]

    state["task_history"].append(
        {"task_id": task_id, "completed_at": time.strftime("%Y-%m-%dT%H:%M:%S")}
    )

    # Update manifest task status
    for t in manifest.get("tasks", []):
        if t["id"] == task_id:
            t["status"] = "completed"
            t["verified_at"] = time.strftime("%Y-%m-%dT%H:%M:%S")

    save_state(state)
    save_manifest(manifest)

    print("\n" + "=" * 78)
    print(f" TASK COMPLETED & VERIFIED: {task_id}")
    print("=" * 78)

    next_t = get_next_task(manifest, state)
    if next_t and not next_t.get("blocked"):
        print(f"Next task queued: {next_t['id']} ({next_t['title']})")
        print(
            "Run 'python scripts/task_runner.py next' to begin next task immediately."
        )
    elif not next_t:
        print("CONGRATULATIONS: All 28 LiveCraft tasks completed successfully!")


def cmd_fail(args):
    task_id = args.task_id
    reason = args.reason or "Verification or build failure"
    state = load_state()
    manifest = load_manifest()

    state.setdefault("failed_tasks", {})[task_id] = {
        "reason": reason,
        "failed_at": time.strftime("%Y-%m-%dT%H:%M:%S"),
    }

    for t in manifest.get("tasks", []):
        if t["id"] == task_id:
            t["status"] = "failed"

    save_state(state)
    save_manifest(manifest)
    print(f"Recorded failure for {task_id}: {reason}")


def cmd_reset(args):
    if not args.force:
        print(
            "To reset progress state, run: python scripts/task_runner.py reset --force"
        )
        return
    manifest = load_manifest()
    for t in manifest.get("tasks", []):
        t["status"] = "pending"
        t.pop("verified_at", None)
    save_manifest(manifest)
    state = {
        "current_task_id": None,
        "completed_tasks": [],
        "failed_tasks": {},
        "task_history": [],
        "last_updated": time.strftime("%Y-%m-%dT%H:%M:%S"),
    }
    save_state(state)
    print("Implementation task state and manifest reset to zero.")


def main():
    parser = argparse.ArgumentParser(description="LiveCraft Autonomous Task Runner")
    subparsers = parser.add_subparsers(dest="command", help="Available subcommands")

    subparsers.add_parser("status", help="Show current progress and task status")
    subparsers.add_parser("next", help="Fetch next executable task instructions")

    show_p = subparsers.add_parser("show", help="Show specification for a task")
    show_p.add_argument("task_id", help="Task ID (e.g. task-001)")

    verify_p = subparsers.add_parser(
        "verify", help="Run verification checks for a task"
    )
    verify_p.add_argument("task_id", help="Task ID (e.g. task-001)")

    complete_p = subparsers.add_parser(
        "complete", help="Mark task completed and verified"
    )
    complete_p.add_argument("task_id", help="Task ID (e.g. task-001)")
    complete_p.add_argument(
        "--skip-verify",
        action="store_true",
        help="Skip pre-completion test verification",
    )

    fail_p = subparsers.add_parser("fail", help="Record a task failure")
    fail_p.add_argument("task_id", help="Task ID (e.g. task-001)")
    fail_p.add_argument("--reason", default="", help="Reason for failure")

    reset_p = subparsers.add_parser("reset", help="Reset all progress")
    reset_p.add_argument("--force", action="store_true", help="Force reset")

    args = parser.parse_args()
    if not args.command:
        parser.print_help()
        sys.exit(0)

    dispatch = {
        "status": cmd_status,
        "next": cmd_next,
        "show": cmd_show,
        "verify": cmd_verify,
        "complete": cmd_complete,
        "fail": cmd_fail,
        "reset": cmd_reset,
    }
    dispatch[args.command](args)


if __name__ == "__main__":
    main()

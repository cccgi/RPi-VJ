#!/usr/bin/env python3
"""storage-gc.py - Automated Storage Garbage Collection for Appliance V3.

Strictly implements storage hardening for task A3-13, ARCHITECTURE.md, and docs/appliance/STORAGE.md:
1. Orphaned Staging Cleanup:
   - Scans /srv/rpi-vj/staging/
   - Removes job staging directories and quarantine directories older than threshold (default 24h).
   - Reconciles with /var/lib/rpi-vj/ingest-jobs.jsonl when available.
2. Generation Retention Policy:
   - Scans /srv/rpi-vj/shows/
   - Preserves active generation (pointed to by /srv/rpi-vj/current).
   - Preserves immediately preceding generation (for rollback guarantee).
   - Safely removes older immutable generations beyond retention count (default: 2).
"""

from __future__ import annotations

import argparse
import datetime
import json
import os
import shutil
import sys
import tempfile
import time
import unittest
from typing import Any

DEFAULT_SHOWS_DIR = os.environ.get("RPI_VJ_SHOWS_DIR", "/srv/rpi-vj/shows")
DEFAULT_STAGING_DIR = os.environ.get("RPI_VJ_STAGING_DIR", "/srv/rpi-vj/staging")
DEFAULT_CURRENT_LINK = os.environ.get("RPI_VJ_CURRENT_LINK", "/srv/rpi-vj/current")
DEFAULT_JOB_LEDGER = os.environ.get(
    "RPI_VJ_JOB_LEDGER", "/var/lib/rpi-vj/ingest-jobs.jsonl"
)
DEFAULT_MAX_STAGING_AGE_HOURS = 24.0
DEFAULT_KEEP_GENERATIONS = 2


def resolve_active_generation_name(
    current_link: str, shows_dir: str
) -> str | None:
    """Resolves the name of the active generation pointed to by current_link."""
    if not os.path.lexists(current_link):
        return None

    target_path: str | None = None
    if os.path.islink(current_link):
        try:
            link_dest = os.readlink(current_link)
            if not os.path.isabs(link_dest):
                link_dest = os.path.normpath(
                    os.path.join(os.path.dirname(current_link), link_dest)
                )
            target_path = link_dest
        except OSError:
            pass
    elif os.path.isfile(current_link):
        try:
            with open(current_link, "r", encoding="utf-8") as f:
                content = f.read().strip()
                if content:
                    target_path = content
        except OSError:
            pass

    if target_path:
        base = os.path.basename(os.path.normpath(target_path))
        if base:
            return base

    return None


def get_path_age_seconds(path: str, reference_time: float | None = None) -> float:
    """Returns age of path in seconds based on mtime."""
    if reference_time is None:
        reference_time = time.time()
    try:
        st = os.stat(path)
        return max(0.0, reference_time - st.st_mtime)
    except OSError:
        return 0.0


def collect_terminal_job_ids(ledger_path: str) -> set[str]:
    """Reads ingest-jobs.jsonl to find job IDs in terminal states (Completed, Failed, Cancelled)."""
    terminal_ids: set[str] = set()
    if not os.path.exists(ledger_path):
        return terminal_ids

    try:
        with open(ledger_path, "r", encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                try:
                    job = json.loads(line)
                    jid = job.get("id")
                    state = job.get("state")
                    if jid and state:
                        if isinstance(state, str) and state in (
                            "Completed",
                            "Failed",
                            "Cancelled",
                        ):
                            terminal_ids.add(str(jid))
                        elif isinstance(state, dict):
                            # e.g. {"Failed": {"reason": "..."}}
                            k = next(iter(state.keys()), "")
                            if k in ("Completed", "Failed", "Cancelled"):
                                terminal_ids.add(str(jid))
                except json.JSONDecodeError:
                    continue
    except OSError:
        pass

    return terminal_ids


def run_storage_gc(
    shows_dir: str = DEFAULT_SHOWS_DIR,
    staging_dir: str = DEFAULT_STAGING_DIR,
    current_link: str = DEFAULT_CURRENT_LINK,
    job_ledger: str = DEFAULT_JOB_LEDGER,
    max_staging_age_hours: float = DEFAULT_MAX_STAGING_AGE_HOURS,
    keep_generations: int = DEFAULT_KEEP_GENERATIONS,
    dry_run: bool = False,
    reference_time: float | None = None,
) -> dict[str, Any]:
    """Performs garbage collection on staging and show generations."""
    if reference_time is None:
        reference_time = time.time()

    max_age_seconds = max_staging_age_hours * 3600.0
    keep_gen_count = max(2, keep_generations)  # Always keep at least 2 (active + rollback)

    result: dict[str, Any] = {
        "timestamp": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "dry_run": dry_run,
        "staging_cleaned": [],
        "staging_skipped": [],
        "generations_retained": [],
        "generations_pruned": [],
        "active_generation": None,
        "bytes_freed_estimate": 0,
        "errors": [],
    }

    # ---------------------------------------------------------
    # 1. Clean Staging Directory
    # ---------------------------------------------------------
    terminal_jobs = collect_terminal_job_ids(job_ledger)

    if os.path.exists(staging_dir) and os.path.isdir(staging_dir):
        try:
            entries = os.listdir(staging_dir)
        except OSError as e:
            result["errors"].append(f"Failed to list staging_dir: {e}")
            entries = []

        for item in entries:
            item_path = os.path.join(staging_dir, item)
            # Only manage job-* and quarantine-* or temporary artifacts
            if not (
                item.startswith("job-")
                or item.startswith("quarantine-")
                or item.startswith(".tmp")
            ):
                continue

            age_sec = get_path_age_seconds(item_path, reference_time=reference_time)
            is_orphaned_by_age = age_sec >= max_age_seconds

            # Check terminal state if job id is extractable
            job_id_suffix = item.replace("job-", "").replace("quarantine-", "").split("-")[-1]
            is_terminal = job_id_suffix in terminal_jobs

            should_delete = is_orphaned_by_age or (is_terminal and age_sec >= 300.0)

            size_bytes = 0
            if os.path.isdir(item_path):
                for root, _, files in os.walk(item_path):
                    for f in files:
                        try:
                            size_bytes += os.path.getsize(os.path.join(root, f))
                        except OSError:
                            pass
            elif os.path.isfile(item_path):
                try:
                    size_bytes = os.path.getsize(item_path)
                except OSError:
                    pass

            item_info = {
                "name": item,
                "path": item_path,
                "age_hours": round(age_sec / 3600.0, 2),
                "is_terminal_job": is_terminal,
                "size_bytes": size_bytes,
            }

            if should_delete:
                result["staging_cleaned"].append(item_info)
                result["bytes_freed_estimate"] += size_bytes
                if not dry_run:
                    try:
                        if os.path.isdir(item_path) and not os.path.islink(item_path):
                            shutil.rmtree(item_path)
                        else:
                            os.remove(item_path)
                    except OSError as e:
                        result["errors"].append(f"Failed to delete {item_path}: {e}")
            else:
                result["staging_skipped"].append(item_info)

    # ---------------------------------------------------------
    # 2. Enforce Generation Retention Policy
    # ---------------------------------------------------------
    active_gen = resolve_active_generation_name(current_link, shows_dir)
    result["active_generation"] = active_gen

    if os.path.exists(shows_dir) and os.path.isdir(shows_dir):
        try:
            gen_entries = [
                d
                for d in os.listdir(shows_dir)
                if d.startswith("gen-") and os.path.isdir(os.path.join(shows_dir, d))
            ]
        except OSError as e:
            result["errors"].append(f"Failed to list shows_dir: {e}")
            gen_entries = []

        # Sort descending by name (ISO timestamp prefix gen-YYYYMMDDHHMMSS-hash)
        gen_entries.sort(reverse=True)

        # Separate into retained and to-prune
        retained: list[str] = []
        to_prune: list[str] = []

        # Always retain the active generation
        if active_gen and active_gen in gen_entries:
            retained.append(active_gen)

        # Fill remaining retention slots with the newest other generations (for rollback)
        for gen in gen_entries:
            if gen not in retained and len(retained) < keep_gen_count:
                retained.append(gen)

        for gen in gen_entries:
            if gen not in retained:
                to_prune.append(gen)

        result["generations_retained"] = retained

        for gen in to_prune:
            gen_path = os.path.join(shows_dir, gen)
            gen_size = 0
            for root, _, files in os.walk(gen_path):
                for f in files:
                    try:
                        gen_size += os.path.getsize(os.path.join(root, f))
                    except OSError:
                        pass

            gen_info = {
                "name": gen,
                "path": gen_path,
                "size_bytes": gen_size,
            }
            result["generations_pruned"].append(gen_info)
            result["bytes_freed_estimate"] += gen_size

            if not dry_run:
                try:
                    shutil.rmtree(gen_path)
                except OSError as e:
                    result["errors"].append(f"Failed to delete old generation {gen_path}: {e}")

    return result


# ---------------------------------------------------------------------------
# Unit Tests
# ---------------------------------------------------------------------------


class TestStorageGC(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp_dir = tempfile.TemporaryDirectory()
        self.base = self.tmp_dir.name
        self.shows_dir = os.path.join(self.base, "shows")
        self.staging_dir = os.path.join(self.base, "staging")
        self.current_link = os.path.join(self.base, "current")
        self.ledger_path = os.path.join(self.base, "ingest-jobs.jsonl")

        os.makedirs(self.shows_dir, exist_ok=True)
        os.makedirs(self.staging_dir, exist_ok=True)

    def tearDown(self) -> None:
        self.tmp_dir.cleanup()

    def test_staging_cleanup_orphaned_and_recent(self) -> None:
        """Verifies staging directories >24h are deleted while recent ones are kept."""
        old_staging = os.path.join(self.staging_dir, "job-old-uuid")
        new_staging = os.path.join(self.staging_dir, "job-new-uuid")
        quarantine_dir = os.path.join(self.staging_dir, "quarantine-old-uuid")

        os.makedirs(old_staging)
        os.makedirs(new_staging)
        os.makedirs(quarantine_dir)

        with open(os.path.join(old_staging, "video.mp4"), "w") as f:
            f.write("fake video data" * 100)
        with open(os.path.join(new_staging, "video.mp4"), "w") as f:
            f.write("active staging data")
        with open(os.path.join(quarantine_dir, "corrupt.mp4"), "w") as f:
            f.write("corrupt data")

        now = 1700000000.0
        # Set old_staging to 25 hours ago
        os.utime(old_staging, (now - 25 * 3600, now - 25 * 3600))
        # Set quarantine to 30 hours ago
        os.utime(quarantine_dir, (now - 30 * 3600, now - 30 * 3600))
        # Set new_staging to 2 hours ago
        os.utime(new_staging, (now - 2 * 3600, now - 2 * 3600))

        res = run_storage_gc(
            shows_dir=self.shows_dir,
            staging_dir=self.staging_dir,
            current_link=self.current_link,
            job_ledger=self.ledger_path,
            max_staging_age_hours=24.0,
            dry_run=False,
            reference_time=now,
        )

        cleaned_names = [x["name"] for x in res["staging_cleaned"]]
        skipped_names = [x["name"] for x in res["staging_skipped"]]

        self.assertIn("job-old-uuid", cleaned_names)
        self.assertIn("quarantine-old-uuid", cleaned_names)
        self.assertIn("job-new-uuid", skipped_names)

        self.assertFalse(os.path.exists(old_staging))
        self.assertFalse(os.path.exists(quarantine_dir))
        self.assertTrue(os.path.exists(new_staging))

    def test_generation_retention_and_rollback_protection(self) -> None:
        """Verifies active generation and previous generation are retained, older ones pruned."""
        gen1 = os.path.join(self.shows_dir, "gen-20260910100000-aaaa1111")
        gen2 = os.path.join(self.shows_dir, "gen-20260912100000-bbbb2222")
        gen3 = os.path.join(self.shows_dir, "gen-20260915100000-cccc3333")

        os.makedirs(gen1)
        os.makedirs(gen2)
        os.makedirs(gen3)

        # Set active to gen3
        with open(self.current_link, "w") as f:
            f.write(gen3)

        res = run_storage_gc(
            shows_dir=self.shows_dir,
            staging_dir=self.staging_dir,
            current_link=self.current_link,
            job_ledger=self.ledger_path,
            keep_generations=2,
            dry_run=False,
        )

        self.assertEqual(res["active_generation"], "gen-20260915100000-cccc3333")
        self.assertIn("gen-20260915100000-cccc3333", res["generations_retained"])
        self.assertIn("gen-20260912100000-bbbb2222", res["generations_retained"])
        pruned_names = [p["name"] for p in res["generations_pruned"]]
        self.assertIn("gen-20260910100000-aaaa1111", pruned_names)

        self.assertTrue(os.path.exists(gen3))
        self.assertTrue(os.path.exists(gen2))
        self.assertFalse(os.path.exists(gen1))

    def test_dry_run_leaves_filesystem_unchanged(self) -> None:
        """Verifies dry-run does not delete any directories."""
        old_staging = os.path.join(self.staging_dir, "job-old-uuid")
        os.makedirs(old_staging)
        now = 1700000000.0
        os.utime(old_staging, (now - 50 * 3600, now - 50 * 3600))

        res = run_storage_gc(
            shows_dir=self.shows_dir,
            staging_dir=self.staging_dir,
            current_link=self.current_link,
            job_ledger=self.ledger_path,
            dry_run=True,
            reference_time=now,
        )

        self.assertEqual(len(res["staging_cleaned"]), 1)
        self.assertTrue(os.path.exists(old_staging))


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="RPi-VJ Appliance V3 Storage Garbage Collector"
    )
    parser.add_argument("--shows-dir", default=DEFAULT_SHOWS_DIR, help="Shows directory")
    parser.add_argument(
        "--staging-dir", default=DEFAULT_STAGING_DIR, help="Staging directory"
    )
    parser.add_argument(
        "--current-link",
        default=DEFAULT_CURRENT_LINK,
        help="Active show symlink or pointer",
    )
    parser.add_argument(
        "--job-ledger", default=DEFAULT_JOB_LEDGER, help="Ingest job ledger"
    )
    parser.add_argument(
        "--max-staging-age-hours",
        type=float,
        default=DEFAULT_MAX_STAGING_AGE_HOURS,
        help="Max age in hours for orphaned staging directories (default: 24)",
    )
    parser.add_argument(
        "--keep-generations",
        type=int,
        default=DEFAULT_KEEP_GENERATIONS,
        help="Number of generations to retain (minimum 2: active + rollback)",
    )
    parser.add_argument(
        "--dry-run", "-n", action="store_true", help="Simulate changes without deleting"
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="Check mode: returns non-zero if garbage was found to clean",
    )
    parser.add_argument(
        "--json", action="store_true", help="Output report as formatted JSON"
    )
    parser.add_argument("--test", action="store_true", help="Run unit test suite")

    return parser


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()

    if args.test:
        suite = unittest.TestLoader().loadTestsFromTestCase(TestStorageGC)
        runner = unittest.TextTestRunner(verbosity=2)
        res = runner.run(suite)
        return 0 if res.wasSuccessful() else 1

    dry_run = args.dry_run or args.check

    result = run_storage_gc(
        shows_dir=args.shows_dir,
        staging_dir=args.staging_dir,
        current_link=args.current_link,
        job_ledger=args.job_ledger,
        max_staging_age_hours=args.max_staging_age_hours,
        keep_generations=args.keep_generations,
        dry_run=dry_run,
    )

    if args.json:
        print(json.dumps(result, indent=2))
    else:
        print("RPi-VJ Appliance Storage Garbage Collection Summary:")
        print(f"  Mode: {'DRY-RUN' if dry_run else 'ENFORCE'}")
        print(f"  Active Generation: {result['active_generation'] or 'None'}")
        print(f"  Retained Generations: {len(result['generations_retained'])} ({', '.join(result['generations_retained'])})")
        print(f"  Pruned Generations: {len(result['generations_pruned'])}")
        for p in result["generations_pruned"]:
            print(f"    - {p['name']} ({p['size_bytes']} bytes)")
        print(f"  Cleaned Staging: {len(result['staging_cleaned'])}")
        for s in result["staging_cleaned"]:
            print(f"    - {s['name']} (age {s['age_hours']}h, {s['size_bytes']} bytes)")
        print(f"  Skipped Staging: {len(result['staging_skipped'])}")
        print(f"  Estimated Bytes Freed: {result['bytes_freed_estimate']}")

    if args.check:
        garbage_found = (
            len(result["staging_cleaned"]) > 0
            or len(result["generations_pruned"]) > 0
        )
        return 1 if garbage_found else 0

    return 0


if __name__ == "__main__":
    sys.exit(main())

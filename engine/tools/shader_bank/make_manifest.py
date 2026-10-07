#!/usr/bin/env python3
"""Build a bench manifest (plan section 3.1) and the matching staging directory.

    # the 18 originals, then the self-contained class, base variant, 720 then 1080
    python engine/tools/shader_bank/make_manifest.py --name originals --class original18
    python engine/tools/shader_bank/make_manifest.py --name selfcontained --class selfcontained
    # only what a results file does not hold yet (new shaders since the last run)
    python engine/tools/shader_bank/make_manifest.py --name todo --class selfcontained \
        --not-in target/shader-bank/results/results.jsonl
    # lower tiers for an explicit list, lite variant included
    python engine/tools/shader_bank/make_manifest.py --name lowtiers --ids-file target/shader-bank/followup-lower-tiers.txt \
        --variant base --variant lite --tiers 540,360 --pi-out /var/lib/rpi-vj-bench/results-lowtiers.jsonl
    # texture shaders with their channel textures bound: the `tex` variant where one exists, else base
    python engine/tools/shader_bank/make_manifest.py --name textures --class needs_texture --first-variant tex,base \
        --pi-out /var/lib/rpi-vj-bench/results-textures.jsonl
    # the same shaders unbound, as a control
    python engine/tools/shader_bank/make_manifest.py --name textures-control --class needs_texture --no-channels \
        --pi-out /var/lib/rpi-vj-bench/results-textures-control.jsonl

Output: target/shader-bank/staging/<name>/manifest.json and
target/shader-bank/staging/<name>/shaders/<variant>/<id>.fs. Upload the
`shaders` tree to /srv/rpi-vj-bench/shaders/ and the manifest anywhere on the Pi.

Channel textures: an entry carries `channels` (asset path, kind, filter, wrap,
vflip per bound iChannel) for every Image-pass input the asset store
(target/shader-bank/assets/manifest.json, see fetch_assets.py) satisfies, unless
--no-channels is given. The files those bindings need are listed in
staging/<name>/assets.txt; upload them from target/shader-bank/assets/ to
/srv/rpi-vj-bench/assets/.
"""
from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from common import (ASSET_MANIFEST_PATH, ASSETS_DIR, CLASSES, CLASS_ORIGINAL, CLASS_SELF, DEFAULT_TIERS,  # noqa: E402
                    ERROR_STATUSES, PI_ASSET_ROOT, PI_RESULTS_PATH, PI_SHADER_ROOT, PI_THUMB_DIR, STAGING_DIR,
                    VARIANTS_DIR, binding_files, channel_bindings, iter_results, load_asset_manifest, load_catalog,
                    manifest_channels, merge_results, parse_tiers, read_id_list, rel, sha256_bytes,
                    write_bytes_if_changed, write_json)

MANIFEST_VERSION = 1
DEFAULT_CLASSES = (CLASS_ORIGINAL, CLASS_SELF)


def done_keys(result_paths, tiers, retry_statuses=()) -> set:
    """(id, variant) keys that need no further measurement for `tiers`.

    A key is done when its merged results hold an entry (measured or
    `skipped`) for every requested tier, or when its latest status is an error
    that is not listed in `retry_statuses` - re-running a shader that does not
    compile will not change the outcome."""
    events, _ = iter_results(result_paths)
    heights = [str(height) for _, height in tiers]
    done = set()
    for key, record in merge_results(events).items():
        status = record.get("status", "ok")
        if status in retry_statuses:
            continue
        if status in ERROR_STATUSES or all(h in record["tiers"] for h in heights):
            done.add(key)
    return done


def select(catalog: dict, classes=None, ids=None, variants=("base",), variants_dir: Path = VARIANTS_DIR,
           skip_keys=frozenset(), limit=None, first_only=False) -> tuple[list, list]:
    """Return (entries, notes). An entry is (record, variant, local_path).

    With explicit `ids` the class filter applies only if `classes` was given.
    A variant file that does not exist (no transform applied) is skipped
    silently for non-base variants and reported for `base`. With `first_only`
    each shader gets one entry: the first listed variant that has a file."""
    by_id = {record["id"]: record for record in catalog["shaders"]}
    notes = []
    if ids is not None:
        unknown = [i for i in ids if i not in by_id]
        if unknown:
            notes.append(f"{len(unknown)} ids not in catalog: {', '.join(unknown[:8])}{' ...' if len(unknown) > 8 else ''}")
        records = [by_id[i] for i in dict.fromkeys(ids) if i in by_id]
        if classes:
            records = [r for r in records if r["class"] in classes]
    else:
        wanted = classes or DEFAULT_CLASSES
        records = [r for r in catalog["shaders"] if r["class"] in wanted]
        # originals first: they define the reference envelope
        records.sort(key=lambda r: (r["class"] != CLASS_ORIGINAL, r["id"].lower(), r["id"]))
    entries, skipped_done, missing_base = [], 0, []
    for record in records:
        for variant in variants:
            path = Path(variants_dir) / variant / f"{record['id']}.fs"
            if not path.is_file():
                if variant == "base":
                    missing_base.append(record["id"])
                continue
            if (record["id"], variant) in skip_keys:
                skipped_done += 1
                if first_only:
                    break
                continue
            entries.append((record, variant, path))
            if first_only:
                break
    if missing_base:
        notes.append(f"{len(missing_base)} shaders have no base variant file (run adapt.py): {', '.join(missing_base[:8])}")
    if skipped_done:
        notes.append(f"{skipped_done} entries already in the given results were left out")
    if limit is not None and len(entries) > limit:
        notes.append(f"limited to the first {limit} of {len(entries)} entries")
        entries = entries[:limit]
    return entries, notes


def entry_bindings(entries, asset_manifest) -> dict:
    """{shader id: (bindings, unbound)} for the entries that have Image-pass inputs."""
    found = {}
    for record, _, _ in entries:
        if record["id"] not in found:
            bindings, unbound = channel_bindings(asset_manifest, record["id"])
            if bindings or unbound:
                found[record["id"]] = (bindings, unbound)
    return found


def build_manifest(entries, tiers, pi_shader_root=PI_SHADER_ROOT, pi_out=PI_RESULTS_PATH, pi_thumb_dir=PI_THUMB_DIR,
                   bindings=None, pi_asset_root=PI_ASSET_ROOT) -> dict:
    """The section 3.1 shape. `bindings` ({id: (bindings, unbound)} from
    `entry_bindings`) adds `channels` to the entries that have file inputs; an
    entry without them keeps exactly the three section 3.1 fields."""
    root = pi_shader_root.rstrip("/")
    rows = []
    for record, variant, _ in entries:
        row = {"id": record["id"], "variant": variant, "path": f"{root}/{variant}/{record['id']}.fs"}
        bound = (bindings or {}).get(record["id"], ([], []))[0]
        if bound:
            row["channels"] = manifest_channels(bound, pi_asset_root)
        rows.append(row)
    return {"version": MANIFEST_VERSION, "out": pi_out, "thumb_dir": pi_thumb_dir,
            "tiers": [[int(w), int(h)] for w, h in tiers], "entries": rows}


def asset_files(entries, bindings) -> list:
    """Sorted store file names the manifest's bindings read."""
    names = set()
    for record, _, _ in entries:
        for binding in (bindings or {}).get(record["id"], ([], []))[0]:
            names.update(binding_files(binding))
    return sorted(names)


def stage(entries, manifest: dict, stage_dir: Path, assets=None) -> dict:
    """Mirror the Pi layout locally: <stage_dir>/shaders/<variant>/<id>.fs.
    `assets` (store file names) is written to assets.txt, the upload list for
    the texture files; they stay in the store and are not copied."""
    stage_dir = Path(stage_dir)
    shader_root = stage_dir / "shaders"
    wanted, index, written = set(), [], 0
    for record, variant, path in entries:
        target = shader_root / variant / f"{record['id']}.fs"
        data = path.read_bytes()
        written += write_bytes_if_changed(target, data)
        wanted.add(target.resolve())
        index.append({"id": record["id"], "variant": variant, "class": record["class"], "sha256": sha256_bytes(data)})
    removed = 0
    if shader_root.is_dir():
        for stale in shader_root.rglob("*.fs"):
            if stale.resolve() not in wanted:
                stale.unlink()
                removed += 1
        for directory in sorted((d for d in shader_root.rglob("*") if d.is_dir()), reverse=True):
            if not any(directory.iterdir()):
                directory.rmdir()
    write_json(stage_dir / "manifest.json", manifest)
    # Local bookkeeping only (not uploaded to the engine): content hashes let a
    # later run tell which results were measured on an older version of a file.
    write_json(stage_dir / "staging-index.json", {"entries": index, "assets": list(assets or [])})
    write_bytes_if_changed(stage_dir / "assets.txt", "".join(f"{name}\n" for name in assets or []).encode("utf-8"))
    return {"written": written, "removed": removed}


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--name", default=None, help="staging directory name (default: derived from the selection)")
    parser.add_argument("--class", dest="classes", action="append", metavar="CLASS",
                        help=f"class filter, repeatable or comma separated ({', '.join(CLASSES)}); "
                             f"default {','.join(DEFAULT_CLASSES)} when no ids are given")
    parser.add_argument("--variant", dest="variants", action="append", help="variant, repeatable or comma separated (default base)")
    parser.add_argument("--first-variant", default=None, metavar="V1,V2",
                        help="one entry per shader: the first of these variants that has a file (e.g. tex,base)")
    parser.add_argument("--no-channels", action="store_true",
                        help="leave the channel texture bindings out (measures texture shaders unbound, as a control)")
    parser.add_argument("--asset-manifest", type=Path, default=ASSET_MANIFEST_PATH)
    parser.add_argument("--pi-asset-root", default=PI_ASSET_ROOT)
    parser.add_argument("--ids", default=None, help="comma separated explicit shader ids")
    parser.add_argument("--ids-file", type=Path, action="append", help="file with ids (text lines or JSON list); repeatable")
    parser.add_argument("--not-in", type=Path, action="append", default=[], metavar="RESULTS.jsonl",
                        help="leave out (id, variant) keys these results already cover for the chosen tiers; repeatable")
    parser.add_argument("--retry-status", default="", help="comma separated statuses to measure again despite --not-in (e.g. crash,hang)")
    parser.add_argument("--tiers", default=",".join(str(t) for t in DEFAULT_TIERS),
                        help="heights (or WxH) in measurement order; default 720,1080")
    parser.add_argument("--limit", type=int, default=None)
    parser.add_argument("--catalog", type=Path, default=None)
    parser.add_argument("--variants-dir", type=Path, default=VARIANTS_DIR)
    parser.add_argument("--staging-root", type=Path, default=STAGING_DIR)
    parser.add_argument("--pi-shader-root", default=PI_SHADER_ROOT)
    parser.add_argument("--pi-out", default=PI_RESULTS_PATH, help="results JSONL path on the Pi (manifest `out`)")
    parser.add_argument("--pi-thumb-dir", default=PI_THUMB_DIR)
    parser.add_argument("--no-stage", action="store_true", help="write only the manifest, do not copy shader files")
    args = parser.parse_args(argv)

    def multi(values):
        return [v.strip() for value in (values or []) for v in value.split(",") if v.strip()]

    classes = multi(args.classes)
    unknown = [c for c in classes if c not in CLASSES]
    if unknown:
        parser.error(f"unknown class: {', '.join(unknown)}")
    if args.first_variant and args.variants:
        parser.error("--first-variant and --variant exclude each other")
    variants = multi([args.first_variant] if args.first_variant else args.variants) or ["base"]
    tiers = parse_tiers(args.tiers)
    ids = None
    if args.ids or args.ids_file:
        ids = multi([args.ids] if args.ids else [])
        for path in args.ids_file or []:
            ids += read_id_list(path)
    catalog = load_catalog(args.catalog) if args.catalog else load_catalog()
    retry = tuple(multi([args.retry_status]))
    skip = done_keys(args.not_in, tiers, retry) if args.not_in else frozenset()
    entries, notes = select(catalog, classes or None, ids, variants, args.variants_dir, skip, args.limit,
                            first_only=bool(args.first_variant))
    bindings = {}
    if not args.no_channels:
        asset_manifest = load_asset_manifest(args.asset_manifest)
        if asset_manifest is None:
            notes.append(f"no asset manifest at {rel(args.asset_manifest)}: entries carry no channel bindings")
        bindings = entry_bindings(entries, asset_manifest)
    assets = asset_files(entries, bindings)
    name = args.name or "-".join((["ids"] if ids is not None else (classes or list(DEFAULT_CLASSES))) + variants
                                 + ["x".join(str(h) for _, h in tiers)])
    stage_dir = args.staging_root / name
    manifest = build_manifest(entries, tiers, args.pi_shader_root, args.pi_out, args.pi_thumb_dir, bindings,
                              args.pi_asset_root)
    if args.no_stage:
        write_json(stage_dir / "manifest.json", manifest)
        counts = {"written": 0, "removed": 0}
    else:
        counts = stage(entries, manifest, stage_dir, assets)
    print(f"MANIFEST {len(entries)} entries, tiers {[h for _, h in tiers]} -> {rel(stage_dir / 'manifest.json')}")
    print(f"  staged shader files: {counts['written']} written, {counts['removed']} stale removed under {rel(stage_dir / 'shaders')}")
    bound = sum(1 for entry in manifest["entries"] if entry.get("channels"))
    if bound:
        partial = sorted({record["id"] for record, _, _ in entries if bindings.get(record["id"], ([], []))[1]
                          and bindings[record["id"]][0]})
        absent = [name for name in assets if not (ASSETS_DIR / name).is_file()]
        print(f"  channel textures: {bound} entries carry bindings, {len(assets)} asset files -> {rel(stage_dir / 'assets.txt')}")
        print(f"  upload: those files from {rel(ASSETS_DIR)}/ -> {args.pi_asset_root}/")
        if partial:
            notes.append(f"{len(partial)} shaders are only partly bound (another input has no file): {', '.join(partial[:8])}"
                         f"{' ...' if len(partial) > 8 else ''}")
        if absent:
            notes.append(f"{len(absent)} asset files are not in the local store: {', '.join(absent[:4])}")
    for note in notes:
        print(f"  note: {note}")
    if [h for _, h in tiers] != list(DEFAULT_TIERS) and args.pi_out == PI_RESULTS_PATH:
        print("  note: non-default tiers with the default results path. The engine's --resume skips (id, variant) keys that")
        print("        already hold a result line, so give a lower-tier pass its own --pi-out file; report.py merges files per tier.")
    print(f"  upload: {rel(stage_dir / 'shaders')}/ -> {args.pi_shader_root}/   and manifest.json -> the Pi")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

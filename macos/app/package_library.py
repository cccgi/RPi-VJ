#!/usr/bin/env python3
"""Create bundle Resources/library from the checkout or the portable library payload."""
import json, os, shutil, sys
from pathlib import Path

repo = Path(__file__).resolve().parents[2]
out = Path(sys.argv[1]).resolve()
catalog = repo / "target/shader-bank/catalog.json"
assets = repo / "target/shader-bank/assets"
payload = repo / "target/mac-app/library-payload"
if not catalog.is_file():
    if not (payload / "target/shader-bank/catalog.json").is_file():
        raise SystemExit(f"shader bank missing: expected {catalog} or {payload}")
    if out.exists(): shutil.rmtree(out)
    shutil.copytree(payload, out)
    data = json.loads((out / "target/shader-bank/catalog.json").read_text(encoding="utf-8"))
    assets_dir = out / "target/shader-bank/assets"
    print(f"PACKAGED_SHADERS {len(data.get('shaders', []))}")
    print(f"PACKAGED_ASSET_FILES {sum(1 for p in assets_dir.rglob('*') if p.is_file())}")
    print(f"ASSET_BYTES {sum(p.stat().st_size for p in assets_dir.rglob('*') if p.is_file())}")
    raise SystemExit(0)
if not assets.is_dir(): raise SystemExit(f"shader assets directory not found: {assets}")
data = json.loads(catalog.read_text(encoding="utf-8"))
shaders = data.get("shaders", [])
if not shaders: raise SystemExit("catalog contains no shader entries")
if out.exists(): shutil.rmtree(out)
(out / "target/shader-bank").mkdir(parents=True)
shutil.copy2(catalog, out / "target/shader-bank/catalog.json")
missing = []
for entry in shaders:
    relative = entry.get("path")
    if not relative: continue
    source = (repo / relative).resolve()
    try: source.relative_to(repo.resolve())
    except ValueError: raise SystemExit(f"shader path escapes project: {relative}")
    if not source.is_file():
        missing.append(relative)
        continue
    target = out / relative
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, target)
if missing: raise SystemExit(f"{len(missing)} shader sources missing; first: {missing[0]}")
shutil.copytree(assets, out / "target/shader-bank/assets", dirs_exist_ok=True)
show = repo / "target/shader-bank/live/show.json"
if show.is_file():
    dest = out / "target/shader-bank/live/show.json"
    dest.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(show, dest)
else:
    show = repo / "engine/show/show.json"
    if show.is_file():
        dest = out / "engine/show/show.json"
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(show, dest)
tools = repo / "engine/tools"
if tools.is_dir(): shutil.copytree(tools, out / "engine/tools", dirs_exist_ok=True)
mpv = os.environ.get("RPI_VJ_MPV")
if mpv:
    mpv = Path(mpv).expanduser().resolve()
    if not mpv.is_file(): raise SystemExit(f"RPI_VJ_MPV is not a file: {mpv}")
    target = out / "target/desktop/mpv" / mpv.name
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(mpv, target)
asset_files = [p for p in (out / "target/shader-bank/assets").rglob("*") if p.is_file()]
print(f"PACKAGED_SHADERS {len(shaders)}")
print(f"PACKAGED_SOURCE_FILES {len(shaders) - len(missing)}")
print(f"PACKAGED_ASSET_FILES {len(asset_files)}")
print(f"ASSET_BYTES {sum(p.stat().st_size for p in asset_files)}")

"""Safety checks for the append-only Pi shader pipeline."""
import hashlib
import json
import tempfile
import unittest
from argparse import Namespace
from pathlib import Path

from run_shader_pipeline import (chosen_tier, merge_show, native_manifest,
                                 normalize_native_results, read_json, source_spec,
                                 unique_ids, write_json)


class ShaderPipelineTests(unittest.TestCase):
    def test_profile_url_is_accepted(self):
        mode, value, name = source_spec(Namespace(resume=None, profile="https://www.shadertoy.com/user/diatribes",
                                                   url=None, file=None, batch=None))
        self.assertEqual((mode, value, name), ("profile", "diatribes", "diatribes"))

    def test_native_benchmark_requires_both_variants_and_both_audio_patterns(self):
        with tempfile.TemporaryDirectory() as temporary:
            package = Path(temporary)
            (package / "native").mkdir()
            (package / "fallback").mkdir()
            (package / "native" / "3dyGRR.fs").write_text("react")
            (package / "fallback" / "3dyGRR.fs").write_text("legacy")
            write_json(package / "candidate-show.json", {"shaders": [
                {"path": "native/3dyGRR.fs", "channels": [
                    {"channel": 0, "path": "assets/noise.png", "kind": "2d"}]}]})
            manifest = native_manifest(package, "/srv/batch")
            self.assertEqual(len(manifest["entries"]), 2)
            self.assertEqual(manifest["entries"][0]["channels"][0]["path"],
                             "/srv/batch/assets/noise.png")
            results = package / "results.jsonl"
            measurements = []
            for variant in ("react", "legacy"):
                measurements.append({"event": "result", "id": "3dyGRR", "variant": variant,
                                     "status": "ok", "tiers": {"720": {"p95_ms": 8, "frames": 3},
                                                                       "1080": {"p95_ms": 14, "frames": 3}},
                                     "tiers_max": {"720": {"p95_ms": 9, "frames": 3},
                                                   "1080": {"p95_ms": 15, "frames": 3}},
                                     "pixels": {"mean_luma": 0.2, "spatial_std": 0.1}})
            results.write_text("\n".join(json.dumps(row) for row in measurements))
            record = normalize_native_results(package, results)["results"][0]
            self.assertEqual(chosen_tier(record), (720, False))
            measurements.pop()
            results.write_text(json.dumps(measurements[0]))
            with self.assertRaisesRegex(RuntimeError, "incomplete"):
                normalize_native_results(package, results)

    def test_input_deduplicates_urls_and_bare_ids(self):
        self.assertEqual(
            unique_ids(["https://www.shadertoy.com/view/3dyGRR", "3dyGRR", "3sdGDX"]),
            ["3dyGRR", "3sdGDX"],
        )

    def test_append_preserves_existing_pages_and_copies_texture(self):
        with tempfile.TemporaryDirectory() as temporary:
            batch = Path(temporary)
            package = batch / "package"
            (package / "native").mkdir(parents=True)
            (package / "assets").mkdir()
            old = {"version": 2, "bpm": 128, "audio_device": None, "clips": [],
                   "shaders": [{"label": "old", "path": "bank-v2/old.fs"}],
                   "pages": [{"name": "ORIGINAL", "bindings": [{"key": 0, "label": "old",
                              "command": {"type": "TriggerShader", "shader": 0},
                              "color": [25, 75, 120]}]}]}
            candidates, results = [], []
            for number in range(25):
                sid = f"A{number:05d}"
                source = package / "native" / f"{sid}.fs"
                source.write_text("void main(){}", encoding="utf-8")
                row = {"label": sid, "path": f"native/{sid}.fs", "blend": "Additive", "opacity": 0.9}
                if number == 0:
                    (package / "assets" / "texture.png").write_bytes(b"texture")
                    row["channels"] = [{"channel": 0, "path": "assets/texture.png", "kind": "2d",
                                        "filter": "linear", "wrap": "repeat", "vflip": True}]
                candidates.append(row)
                results.append({"shader": sid, "source_sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
                                "1080p": {"suitable_60fps": False},
                                "720p": {"suitable_60fps": True}})
            write_json(package / "candidate-show.json", {"shaders": candidates})
            stage, selected = merge_show(old, package, {"results": results}, "new-batch", batch)
            merged = read_json(stage / "show.json")
            self.assertEqual(merged["pages"][:1], old["pages"])
            self.assertEqual(merged["shaders"][:1], old["shaders"])
            self.assertEqual((len(selected), len(merged["pages"]), len(merged["shaders"])), (25, 3, 26))
            self.assertEqual(merged["pages"][1]["bindings"][0]["command"]["shader"], 1)
            self.assertEqual(merged["pages"][2]["bindings"][0]["command"]["shader"], 25)
            self.assertTrue((stage / "batches" / "new-batch" / "assets" / "texture.png").is_file())


if __name__ == "__main__":
    unittest.main()

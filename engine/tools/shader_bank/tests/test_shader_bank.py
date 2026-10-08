#!/usr/bin/env python3
"""Unit tests for the shader-bank tooling.

    python -m unittest discover -s engine/tools/shader_bank/tests -v
"""
import contextlib
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import adapt  # noqa: E402
import catalog  # noqa: E402
import common  # noqa: E402
import glsl_features  # noqa: E402
import make_manifest  # noqa: E402
import make_native_shadertoy  # noqa: E402  (on the path through adapt)
import report  # noqa: E402
import synth_results  # noqa: E402

HEAD = ('/* {"LABEL":"Shadertoy native"} */\n#define HW_PERFORMANCE 1\nprecision highp float;\n'
        'uniform vec4 iMouse;\nuniform sampler2D iChannel0;\nuniform sampler2D iChannel1;\n'
        'uniform sampler2D iChannel2;\nuniform sampler2D iChannel3;\nuniform float iChannelTime[4];\n'
        'uniform vec3 iChannelResolution[4];\n')
WRAPPER = ('\nvoid main(){\n    vec2 center = 0.5 * u_resolution;\n    vec4 c=vec4(0.0);\n'
           '    mainImage(c, gl_FragCoord.xy);\n    gl_FragColor=c;\n}\n')


def native(body: str) -> str:
    return HEAD + body + WRAPPER


def meta(image_inputs=(), extra_passes=(), known=True):
    passes = [{"type": "image", "name": "Image", "inputs": [{"channel": c, "ctype": t} for c, t in image_inputs]}]
    passes += [{"type": t, "name": t, "inputs": []} for t in extra_passes]
    return {"source": "x", "inputs_known": known, "name": "Fixture", "username": "u", "tags": [], "passes": passes}


PLAIN = "void mainImage(out vec4 o, in vec2 u){ o = vec4(sin(u_time)); }"
TEX = "void mainImage(out vec4 o, in vec2 u){ o = texture(iChannel0, u/u_resolution); }"
TEX1 = "void mainImage(out vec4 o, in vec2 u){ o = texture(iChannel1, u/u_resolution); }"


class SplitAndStrip(unittest.TestCase):
    def test_body_excludes_header_and_wrapper(self):
        text = native(PLAIN)
        start, end = common.split_shadertoy(text)
        self.assertEqual(text[start:end].strip(), PLAIN)

    def test_crlf_layout(self):
        text = native(PLAIN).replace("\n", "\r\n")
        start, end = common.split_shadertoy(text)
        self.assertIn("mainImage", text[start:end])
        self.assertNotIn("iChannel0", text[start:end])
        self.assertNotIn("vec2 center", text[start:end])

    def test_isf_file_is_all_body_after_header(self):
        text = '/*{"DESCRIPTION":"d"}*/\nuniform float u_time;\nvoid main(){}'
        start, end = common.split_shadertoy(text)
        self.assertTrue(text[start:end].lstrip().startswith("uniform"))

    def test_strip_comments_keeps_offsets(self):
        text = "a /* iChannel0 \n x */ b // iChannel1\nc"
        stripped = common.strip_comments(text)
        self.assertEqual(len(stripped), len(text))
        self.assertEqual(stripped.count("\n"), text.count("\n"))
        self.assertNotIn("iChannel", stripped)

    def test_parse_tiers(self):
        self.assertEqual(common.parse_tiers("720,1080"), [[1280, 720], [1920, 1080]])
        self.assertEqual(common.parse_tiers("540,360"), [[960, 540], [640, 360]])
        self.assertEqual(common.parse_tiers("1024x576"), [[1024, 576]])
        with self.assertRaises(ValueError):
            common.parse_tiers("")


class Classification(unittest.TestCase):
    def classify(self, body, metadata, origin="native13"):
        return catalog.classify(body, metadata, origin)

    def test_selfcontained(self):
        result = self.classify(PLAIN, meta())
        self.assertEqual(result["class"], "selfcontained")
        self.assertFalse(any(result["flags"][k] for k in ("texture", "buffer", "keyboard", "audio_channel", "mouse")))

    def test_needs_texture(self):
        result = self.classify(TEX, meta([(0, "texture")]))
        self.assertEqual(result["class"], "needs_texture")
        self.assertTrue(result["flags"]["texture"])
        self.assertEqual(result["channels"]["kinds"], {"0": "texture"})

    def test_declared_but_unused_input_is_selfcontained(self):
        result = self.classify(PLAIN, meta([(0, "texture")]))
        self.assertEqual(result["class"], "selfcontained")
        self.assertTrue(result["flags"]["declared_unused_inputs"])

    def test_commented_reference_does_not_count(self):
        body = "// texture(iChannel0, u)\n/* iChannel1 */\n" + PLAIN
        self.assertEqual(self.classify(body, meta([(0, "texture")]))["class"], "selfcontained")

    def test_multipass(self):
        result = self.classify(TEX, meta([(0, "buffer")], ["buffer"]))
        self.assertEqual(result["class"], "multipass")
        self.assertTrue(result["flags"]["buffer"])

    def test_buffer_pass_not_read_by_image(self):
        result = self.classify(TEX, meta([(0, "musicstream")], ["buffer"]))
        self.assertEqual(result["class"], "needs_texture")
        self.assertTrue(result["flags"]["audio_channel"])
        self.assertTrue(result["flags"]["buffer_passes_unused_by_image"])

    def test_buffer_pass_and_untyped_channel_is_multipass(self):
        self.assertEqual(self.classify(TEX1, meta([(0, "texture")], ["buffer"]))["class"], "multipass")

    def test_input_kind_flags(self):
        for ctype, flag in (("keyboard", "keyboard"), ("music", "audio_channel"), ("mic", "audio_channel"),
                            ("video", "video_webcam"), ("webcam", "video_webcam"), ("cubemap", "cubemap"),
                            ("volume", "volume")):
            result = self.classify(TEX, meta([(0, ctype)]))
            self.assertEqual(result["class"], "needs_texture", ctype)
            self.assertTrue(result["flags"][flag], ctype)

    def test_undeclared_channel(self):
        result = self.classify(TEX, meta())
        self.assertEqual(result["class"], "needs_texture")
        self.assertTrue(result["flags"]["undeclared_channel"])
        self.assertEqual(result["channels"]["kinds"], {"0": "undeclared"})

    def test_unknown_inputs_without_shader_json(self):
        result = self.classify(TEX, meta(known=False))
        self.assertEqual(result["channels"]["kinds"], {"0": "unknown"})
        self.assertTrue(result["flags"]["metadata_inputs_unknown"])

    def test_mouse_flag_and_original(self):
        body = "void mainImage(out vec4 o, in vec2 u){ o = vec4(iMouse.xy, 0, 1); }"
        self.assertTrue(self.classify(body, meta())["flags"]["mouse"])
        self.assertEqual(self.classify(PLAIN, None, "original18")["class"], "original18")


class Features(unittest.TestCase):
    def test_loop_iterations(self):
        constants = {"AA": 2.0, "N": 80.0}
        cases = {
            "int i=0;i<N;i++": 80, "float i=0.;i++<1e2;": 100, "float i,d;i++<80.;": 80,
            "int i=10;i>0;i--": 10, "float i=-AA;i<AA;++i": 4, "int m=ZERO; m<AA; m++": 2,
            "float t=0.;t<10.;t+=.5": 20, "int X=0;X<=AA;X++": 3, "int i=0;80>i;i++": 80,
            "n = .06; n < 2.;n += n": 6, "a = .01; a < 1.; a += a+a": 5, "int i=0;i<int(N);i++": 80,
            "int i = 0; i < iterations; i++": None, "; i > .15; i *= .201": None,
        }
        for header, expected in cases.items():
            self.assertEqual(glsl_features.loop_iterations(header, constants), expected, header)

    def test_extract_features(self):
        body = """
#define STEPS 64
#define AA 2
// #define HIGH_QUALITY
float map(vec3 p){ float d=0.; for(int i=0;i<5;i++){ d+=length(p);} return d; }
float fbm(vec2 p){ float f=0.; for(int i=0;i<6;i++) f+=noise(p); return f; }
vec3 calcNormal(vec3 p){ return normalize(vec3(map(p),map(p),map(p))); }
void mainImage(out vec4 o, in vec2 u){
  float t=0.;
  for(int i=0;i<STEPS;i++){ float d=map(vec3(u,t)); if(d<.001) break; t+=d; }
  for(int m=0;m<AA;m++) for(int n=0;n<AA;n++) { o+=texture(iChannel0,u); }
  while(t<9.) t+=1.;
  o=vec4(fwidth(t));
#if HW_PERFORMANCE==0
  o*=.5;
#endif
}
"""
        f = glsl_features.extract_features(body)
        self.assertEqual(f["for_loops"], 5)
        self.assertEqual(f["while_loops"], 1)
        self.assertEqual(f["loop_bound_max"], 64)
        self.assertEqual(f["loop_bounds"], [64, 6, 5, 2, 2])
        self.assertEqual(f["loop_nest_depth_max"], 2)
        self.assertEqual(f["loop_nest_product_max"], 64)
        self.assertEqual(f["est_iter_product"], 320)  # 64 march steps x 5 iterations inside map()
        self.assertTrue(f["raymarch"])
        self.assertEqual(f["fbm_octaves_hint"], 6)
        self.assertEqual(f["aa_defines"], {"AA": ["2"]})
        self.assertEqual(f["step_defines"], {"STEPS": ["64"]})
        self.assertEqual(f["commented_quality_defines"], ["HIGH_QUALITY"])
        self.assertTrue(f["uses_hw_performance"])
        self.assertEqual(f["derivatives"], 1)
        self.assertEqual(f["texture_fetches"], 1)
        self.assertEqual(f["texture_fetches_in_loops"], 1)
        self.assertEqual(f["loops_unresolved"], 1)

    def test_no_loops(self):
        f = glsl_features.extract_features(PLAIN)
        self.assertEqual((f["loops"], f["est_iter_product"], f["raymarch"]), (0, 1, False))

    def test_macro_loops_are_counted_separately(self):
        f = glsl_features.extract_features("#define R(n) for(int i=0;i<n;i++)\n" + PLAIN)
        self.assertEqual((f["loops"], f["loops_in_macros"]), (0, 1))


class Transforms(unittest.TestCase):
    def test_hw_performance_only_when_tested(self):
        used = native("#if HW_PERFORMANCE==0\n#define Q 1\n#else\n#define Q 2\n#endif\n" + PLAIN)
        out, applied = adapt.transform(used, ["hw_performance_0"])
        self.assertIn("#define HW_PERFORMANCE 0", out)
        self.assertEqual([a["rule"] for a in applied], ["hw_performance_0"])
        self.assertEqual(applied[0]["edits"][0]["line"], 2)
        out, applied = adapt.transform(native(PLAIN), ["hw_performance_0"])
        self.assertEqual((out, applied), (native(PLAIN), []))

    def test_hw_performance_reference_in_comment_ignored(self):
        text = native("// uses HW_PERFORMANCE\n" + PLAIN)
        self.assertEqual(adapt.transform(text, ["hw_performance_0"])[1], [])

    def test_aa_samples_int_float_const(self):
        loop = "void mainImage(out vec4 o, in vec2 u){ for(int m=0;m<AA;m++) for(int n=0;n<AA;n++) o+=1.; }"
        out, applied = adapt.transform("#define AA 3 // comment\n" + loop, ["aa_samples_1"])
        self.assertTrue(out.startswith("#define AA 1 // comment\n"))
        self.assertEqual(applied[0]["edits"], [{"line": 1, "before": "#define AA 3 // comment", "after": "#define AA 1 // comment"}])
        out, _ = adapt.transform("#define AA\t 2.\nvoid f(){ for(float i=-AA;i<AA;++i){} }", ["aa_samples_1"])
        self.assertTrue(out.startswith("#define AA\t 1.\n"))
        out, _ = adapt.transform("#define SSAA 20.0\nvoid f(){ for(float i=0.;i<SSAA;i++){} }", ["aa_samples_1"])
        self.assertTrue(out.startswith("#define SSAA 1.0\n"))
        out, _ = adapt.transform("const float antialias=2.;\nvoid f(){ if (aa+1.>antialias*antialias) break; }", ["aa_samples_1"])
        self.assertTrue(out.startswith("const float antialias=1.;"))
        out, _ = adapt.transform("#define AA 2\n#if AA>1\nfloat x;\n#endif\n", ["aa_samples_1"])
        self.assertTrue(out.startswith("#define AA 1\n"))

    def test_aa_samples_leaves_other_things_alone(self):
        for text in (
            "#define AA 1\nvoid f(){ for(int m=0;m<AA;m++){} }",                 # already 1
            "#define AA 3.\nvoid f(){ c = smoothstep(AA/r.y, 0., d); }",          # a smoothing width, not a count
            "#define AA(x) smoothstep(2., 0., x)\nvoid f(){ for(int i=0;i<4;i++){} }",  # function-like macro
            "#define SAMPLES 64\nvoid f(){ for(int i=0;i<SAMPLES;i++){} }",       # blur/path samples change the look
            "#define AA 2\n/* for(int m=0;m<AA;m++) */\nvoid f(){}",              # only used in a comment
            "#define AA_FRAMES 16.0\nvoid f(){ for(float i=0.;i<AA_FRAMES;i++){} }",
        ):
            self.assertEqual(adapt.transform(text, adapt.VARIANT_RULES["lite"]), (text, []), text)

    def test_aa_switch_off(self):
        text = "#define AA  1  // optional\n#if ! AA\nfloat a;\n#else\nfloat b;\n#endif\n"
        out, applied = adapt.transform(text, ["aa_switch_off"])
        self.assertTrue(out.startswith("#define AA  0  // optional\n"))
        self.assertEqual(applied[0]["rule"], "aa_switch_off")
        text = "#define AA 1\n#if AA == 0\nfloat a;\n#elif AA == 1\nfloat b;\n#endif\n"
        self.assertTrue(adapt.transform(text, ["aa_switch_off"])[0].startswith("#define AA 0\n"))
        loop = "#define AA 1\n#if AA == 0\n#endif\nvoid f(){ for(int m=0;m<AA;m++){} }"
        self.assertEqual(adapt.transform(loop, ["aa_switch_off"])[1], [])   # also a count: leave it

    def test_crlf_and_everything_else_preserved(self):
        text = native("#define AA 2\r\nvoid mainImage(out vec4 o, in vec2 u){ for(int m=0;m<AA;m++){} }").replace("\n", "\r\n").replace("\r\r\n", "\r\n")
        out, applied = adapt.transform(text, adapt.VARIANT_RULES["lite"])
        self.assertEqual(len(applied), 1)
        self.assertEqual(out, text.replace("#define AA 2", "#define AA 1"))

    def test_rules_registry_is_consistent(self):
        for variant, rule_ids in adapt.VARIANT_RULES.items():
            for rule_id in rule_ids:
                self.assertIn(rule_id, adapt.RULES, variant)
        self.assertEqual(adapt.VARIANT_RULES["base"], ())


class Workspace(unittest.TestCase):
    """A small bank on disk: two libraries, metadata, catalog, variants."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        root = Path(self.tmp.name)
        self.root = root
        self.native, self.fractals, self.originals = root / "native", root / "fractals", root / "originals"
        for directory in (self.native, self.fractals / "shaders", self.originals):
            directory.mkdir(parents=True)
        aa_body = "#define AA 2\nvoid mainImage(out vec4 o, in vec2 u){ for(int m=0;m<AA;m++) for(int n=0;n<AA;n++) o+=1.; }"
        files = {
            self.native / "aaaaaa.fs": native(PLAIN),
            self.native / "bbbbbb.fs": native(TEX),
            self.native / "cccccc.fs": native(TEX.replace("u/u_resolution", "u/u_resolution*.5")),
            self.native / "dddddd.fs": native(aa_body),
            self.native / "eeeeee.fs": native(PLAIN),                       # identical content to aaaaaa
            self.native / "ffffff.fs": native(PLAIN + "\n// fork"),         # same code, different bytes
            self.fractals / "shaders" / "aaaaaa.fs": native(PLAIN),         # same id, same content
            self.fractals / "shaders" / "dddddd.fs": native(PLAIN + " "),   # same id, different content
            self.fractals / "shaders" / "mandel.fs": '/* {"LABEL":"Mandel"} */\nuniform float u_time;\nvoid main(){ gl_FragColor=vec4(1.); }',
            self.originals / "plasma.fs": '/*{"DESCRIPTION":"Plasma"}*/\nuniform float u_time;\nvoid main(){ gl_FragColor=vec4(sin(u_time)); }',
        }
        for path, text in files.items():
            path.write_text(text, encoding="utf-8", newline="")
        self.metadata = {"aaaaaa": meta(), "bbbbbb": meta([(0, "texture")]), "cccccc": meta([(0, "buffer")], ["buffer"]),
                         "dddddd": meta(), "eeeeee": meta(), "ffffff": meta()}
        show = {"version": 2, "shaders": [{"label": "dddddd Name D", "path": "dddddd.fs"}, {"label": "aaaaaa A", "path": "aaaaaa.fs"}]}
        self.review = self.native / "show-fixed-first-latest.json"
        self.review.write_text(json.dumps(show), encoding="utf-8")
        self.catalog = catalog.build_catalog(
            libraries=(("original18", self.originals), ("native13", self.native), ("fractals", self.fractals / "shaders")),
            metadata_loader=lambda shader_id: self.metadata.get(shader_id), review_show=self.review,
            fractals_dir=self.fractals, native_dir=self.native)
        self.by_id = {r["id"]: r for r in self.catalog["shaders"]}
        self.variants = root / "variants"
        self.reports = {v: adapt.build_variant(v, self.catalog, self.variants, root) for v in ("base", "lite")}

    def write_results(self, name, events):
        path = self.root / name
        path.write_text("".join(json.dumps(e) + "\n" for e in events), encoding="utf-8")
        return path


def ok_event(shader_id, variant="base", p95_720=4.0, p95_1080=9.0, pixels=None, **extra):
    def tier(p95):
        return {"frames": 90, "mean_ms": p95 * 0.9, "p50_ms": p95 * 0.9, "p95_ms": p95, "max_ms": p95 * 1.2, "first_ms": p95 * 2}
    event = {"event": "result", "id": shader_id, "variant": variant, "status": "ok", "compile_ms": 100.0,
             "tiers": {"720": tier(p95_720), "1080": tier(p95_1080)},
             "pixels": pixels or {"mean_luma": 0.3, "spatial_std": 0.2, "temporal_diff": 0.05},
             "thumb": f"{shader_id}.{variant}.ppm", "temp_c": 60.0, "throttled": "0x0"}
    event.update(extra)
    return event


class CatalogBuild(Workspace):
    def test_one_record_per_id_and_classes(self):
        self.assertEqual(sorted(self.by_id), ["aaaaaa", "bbbbbb", "cccccc", "dddddd", "eeeeee", "ffffff", "mandel", "plasma"])
        classes = {i: r["class"] for i, r in self.by_id.items()}
        self.assertEqual(classes, {"aaaaaa": "selfcontained", "bbbbbb": "needs_texture", "cccccc": "multipass",
                                   "dddddd": "selfcontained", "eeeeee": "selfcontained", "ffffff": "selfcontained",
                                   "mandel": "selfcontained", "plasma": "original18"})
        self.assertEqual(self.catalog["summary"]["total"], 8)
        self.assertEqual(self.catalog["shaders"][0]["id"], "plasma")       # originals first

    def test_duplicates_and_conflicts(self):
        dup = self.catalog["duplicates"]
        self.assertEqual([d["id"] for d in dup["same_id_same_content"]], ["aaaaaa"])
        self.assertEqual(self.by_id["aaaaaa"]["libraries"], ["native13", "fractals"])
        self.assertEqual(dup["identical_content_different_id"], [["aaaaaa", "eeeeee"]])
        self.assertEqual(dup["same_code_different_id"], [["aaaaaa", "eeeeee", "ffffff"]])
        self.assertEqual([c["id"] for c in self.catalog["conflicts"]], ["dddddd"])
        self.assertEqual(self.by_id["dddddd"]["origin"], "native13")       # first library wins

    def test_names_review_numbers_and_hash(self):
        self.assertEqual(self.by_id["dddddd"]["review_number"], 1)
        self.assertEqual(self.by_id["aaaaaa"]["review_number"], 2)
        self.assertIsNone(self.by_id["bbbbbb"]["review_number"])
        self.assertEqual(self.by_id["plasma"]["name"], "Plasma")
        self.assertEqual(self.by_id["aaaaaa"]["name"], "Fixture")
        data = (self.native / "aaaaaa.fs").read_bytes()
        self.assertEqual(self.by_id["aaaaaa"]["sha256"], common.sha256_bytes(data))
        self.assertEqual(self.by_id["aaaaaa"]["size_bytes"], len(data))

    def test_diff_catalogs(self):
        (self.native / "gggggg.fs").write_text(native(PLAIN + "//x"), encoding="utf-8")
        (self.native / "bbbbbb.fs").write_text(native(PLAIN), encoding="utf-8")
        newer = catalog.build_catalog(
            libraries=(("original18", self.originals), ("native13", self.native)),
            metadata_loader=lambda shader_id: self.metadata.get(shader_id), review_show=self.review,
            fractals_dir=self.fractals, native_dir=self.native)
        diff = catalog.diff_catalogs(self.catalog, newer)
        self.assertEqual(diff["added"], ["gggggg"])
        self.assertEqual(diff["removed"], ["mandel"])
        self.assertEqual(diff["content_changed"], ["bbbbbb"])
        self.assertEqual(diff["class_changed"], ["bbbbbb"])


class Variants(Workspace):
    def test_base_is_byte_identical_and_complete(self):
        self.assertEqual(self.reports["base"]["summary"]["shaders"], 8)
        for record in self.catalog["shaders"]:
            self.assertEqual((self.variants / "base" / f"{record['id']}.fs").read_bytes(),
                             (self.root / record["path"]).read_bytes() if not Path(record["path"]).is_absolute()
                             else Path(record["path"]).read_bytes())

    def test_lite_only_where_a_transform_applies(self):
        self.assertEqual(sorted(p.name for p in (self.variants / "lite").glob("*.fs")), ["dddddd.fs"])
        document = json.loads((self.variants / "lite" / "transforms.json").read_text(encoding="utf-8"))
        self.assertEqual(list(document["shaders"]), ["dddddd"])
        self.assertEqual(document["shaders"]["dddddd"]["transforms"][0]["rule"], "aa_samples_1")
        self.assertEqual(document["summary"]["rule_counts"], {"hw_performance_0": 0, "aa_samples_1": 1, "aa_switch_off": 0})
        self.assertIn("#define AA 1", (self.variants / "lite" / "dddddd.fs").read_text(encoding="utf-8"))

    def test_rerun_is_incremental_and_removes_stale(self):
        (self.variants / "lite" / "zzzzzz.fs").write_text("stale", encoding="utf-8")
        again = adapt.build_variant("lite", self.catalog, self.variants, self.root)
        self.assertEqual((again["files_written"], again["files_removed"]), (0, 1))


class Manifests(Workspace):
    def select(self, **kwargs):
        kwargs.setdefault("variants_dir", self.variants)
        return make_manifest.select(self.catalog, **kwargs)

    def test_default_selection_is_originals_then_selfcontained(self):
        entries, _ = self.select()
        self.assertEqual([r["id"] for r, _, _ in entries], ["plasma", "aaaaaa", "dddddd", "eeeeee", "ffffff", "mandel"])

    def test_class_and_variant_selection(self):
        entries, _ = self.select(classes=["needs_texture"])
        self.assertEqual([(r["id"], v) for r, v, _ in entries], [("bbbbbb", "base")])
        entries, _ = self.select(classes=["selfcontained"], variants=["base", "lite"])
        self.assertEqual([(r["id"], v) for r, v, _ in entries if r["id"] == "dddddd"], [("dddddd", "base"), ("dddddd", "lite")])
        self.assertEqual(sum(1 for _, v, _ in entries if v == "lite"), 1)   # no lite file -> no entry

    def test_explicit_ids(self):
        entries, notes = self.select(ids=["cccccc", "nope", "aaaaaa"])
        self.assertEqual([r["id"] for r, _, _ in entries], ["cccccc", "aaaaaa"])
        self.assertTrue(any("not in catalog" in n for n in notes))

    def test_manifest_matches_contract(self):
        entries, _ = self.select(classes=["original18"])
        manifest = make_manifest.build_manifest(entries, common.parse_tiers("720,1080"))
        self.assertEqual(manifest, {"version": 1, "out": "/var/lib/rpi-vj-bench/results.jsonl",
                                    "thumb_dir": "/var/lib/rpi-vj-bench/thumbs", "tiers": [[1280, 720], [1920, 1080]],
                                    "entries": [{"id": "plasma", "variant": "base", "path": "/srv/rpi-vj-bench/shaders/base/plasma.fs"}]})

    def test_not_in_results_is_tier_aware(self):
        results = self.write_results("r.jsonl", [
            {"event": "begin", "id": "aaaaaa", "variant": "base"},
            ok_event("aaaaaa"),
            {"event": "result", "id": "dddddd", "variant": "base", "status": "compile_error", "error": "x"},
            {"event": "result", "id": "eeeeee", "variant": "base", "status": "hang", "error": "x"},
            {"event": "begin", "id": "ffffff", "variant": "base"},            # begun, never finished
        ])
        done = make_manifest.done_keys([results], common.parse_tiers("720,1080"))
        self.assertEqual(done, {("aaaaaa", "base"), ("dddddd", "base"), ("eeeeee", "base")})
        entries, notes = self.select(skip_keys=done)
        self.assertEqual([r["id"] for r, _, _ in entries], ["plasma", "ffffff", "mandel"])
        self.assertTrue(any("already in the given results" in n for n in notes))
        # lower tiers were never measured for aaaaaa, errors stay done
        low = make_manifest.done_keys([results], common.parse_tiers("540,360"))
        self.assertEqual(low, {("dddddd", "base"), ("eeeeee", "base")})
        retry = make_manifest.done_keys([results], common.parse_tiers("720,1080"), ("hang",))
        self.assertNotIn(("eeeeee", "base"), retry)

    def test_staging_mirrors_pi_layout_and_cleans_stale(self):
        entries, _ = self.select(classes=["selfcontained"], variants=["base", "lite"])
        manifest = make_manifest.build_manifest(entries, common.parse_tiers("720"))
        stage_dir = self.root / "staging" / "t"
        make_manifest.stage(entries, manifest, stage_dir)
        for entry in manifest["entries"]:
            local = stage_dir / "shaders" / entry["path"].split("/shaders/", 1)[1]
            self.assertTrue(local.is_file(), entry)
        self.assertEqual(json.loads((stage_dir / "manifest.json").read_text(encoding="utf-8")), manifest)
        fewer, _ = self.select(ids=["aaaaaa"])
        counts = make_manifest.stage(fewer, make_manifest.build_manifest(fewer, common.parse_tiers("720")), stage_dir)
        self.assertEqual(sorted(p.name for p in (stage_dir / "shaders").rglob("*.fs")), ["aaaaaa.fs"])
        self.assertEqual(counts["removed"], len(entries) - 1)


class Verdicts(unittest.TestCase):
    def test_tier_verdict_thresholds(self):
        def verdict(p95, frames=90):
            return report.tier_verdict({"frames": frames, "p95_ms": p95})
        self.assertEqual([verdict(v) for v in (0.5, 10.0, 10.01, 13.0, 13.01, 16.0, 16.01, 400.0)],
                         ["SOLID", "SOLID", "OK", "OK", "MARGINAL", "MARGINAL", "FAIL", "FAIL"])
        self.assertEqual(report.tier_verdict({"skipped": "too_slow"}), "SKIPPED")
        self.assertEqual(report.tier_verdict({"frames": 0, "first_ms": 2100.0}), "NO_DATA")
        self.assertEqual(verdict(5.0, frames=2), "NO_DATA")
        self.assertIsNone(report.tier_verdict(None))
        self.assertEqual((report.SOLID_P95_MS, report.OK_P95_MS, report.MARGINAL_P95_MS), (10.0, 13.0, 16.0))

    def test_pixel_flags(self):
        self.assertEqual(report.pixel_flags({"mean_luma": 0.3, "spatial_std": 0.2, "temporal_diff": 0.05}), [])
        self.assertEqual(report.pixel_flags({"mean_luma": 0.001, "spatial_std": 0.2, "temporal_diff": 0.05}), ["black"])
        self.assertEqual(report.pixel_flags({"mean_luma": 0.5, "spatial_std": 0.0, "temporal_diff": 0.05}), ["flat"])
        self.assertEqual(report.pixel_flags({"mean_luma": 0.5, "spatial_std": 0.2, "temporal_diff": 0.0}), ["static"])
        self.assertEqual(report.pixel_flags({"mean_luma": 0.0, "spatial_std": 0.0, "temporal_diff": 0.0}), ["black", "flat", "static"])
        self.assertEqual(report.pixel_flags(None), ["no_pixel_stats"])

    def test_merge_later_line_wins_per_tier(self):
        first = ok_event("x", p95_720=20.0, p95_1080=40.0)
        second = {"event": "result", "id": "x", "variant": "base", "status": "ok",
                  "tiers": {"720": {"frames": 90, "p50_ms": 11.0, "p95_ms": 12.0}, "540": {"frames": 90, "p50_ms": 6.0, "p95_ms": 7.0}}}
        merged = common.merge_results([{"event": "begin", "id": "x", "variant": "base"}, first, second])
        record = merged[("x", "base")]
        self.assertEqual(record["tiers"]["720"]["p95_ms"], 12.0)      # later line
        self.assertEqual(record["tiers"]["1080"]["p95_ms"], 40.0)     # kept from the earlier line
        self.assertEqual(record["tiers"]["540"]["p95_ms"], 7.0)
        self.assertEqual(record["pixels"]["mean_luma"], 0.3)          # kept: later line had none
        self.assertEqual(record["lines"], 2)
        assessed = report.assess_variant(record)
        self.assertEqual(assessed["verdicts"], {720: "OK", 1080: "FAIL", 540: "SOLID"})
        self.assertEqual(assessed["assigned_tier"], 720)

    def test_later_status_wins(self):
        hang = {"event": "result", "id": "x", "variant": "base", "status": "hang", "error": "no output"}
        self.assertEqual(common.merge_results([hang, ok_event("x")])[("x", "base")]["status"], "ok")
        self.assertEqual(common.merge_results([ok_event("x"), hang])[("x", "base")]["status"], "hang")

    def test_iter_results_skips_torn_lines(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "r.jsonl"
            path.write_text(json.dumps(ok_event("x")) + '\n\n{"event":"result","id":"y","sta', encoding="utf-8")
            events, warnings = common.iter_results([path])
        self.assertEqual(len(events), 1)
        self.assertEqual(len(warnings), 1)


class Assessment(unittest.TestCase):
    def assess(self, events, klass="selfcontained", kinds=None):
        record = {"id": "x", "class": klass, "channels": {"kinds": kinds or {}}, "name": "X", "review_number": 7}
        merged = common.merge_results(events)
        return report.assess_shader(record, {v: r for (i, v), r in merged.items()})

    def test_assigned_tier_is_highest_solid_or_ok(self):
        self.assertEqual(self.assess([ok_event("x", p95_720=4.0, p95_1080=9.9)])["render_height"], 1080)
        self.assertEqual(self.assess([ok_event("x", p95_720=4.0, p95_1080=12.9)])["render_height"], 1080)
        item = self.assess([ok_event("x", p95_720=9.0, p95_1080=15.0)])       # MARGINAL at 1080 is not assigned
        self.assertEqual((item["outcome"], item["render_height"]), ("qualified", 720))

    def test_too_heavy_then_lower_tier_rescues(self):
        heavy = ok_event("x", p95_720=20.0, p95_1080=45.0)
        item = self.assess([heavy])
        self.assertEqual(item["outcome"], "too_heavy")
        self.assertIn("not yet measured at 540p/360p", item["reason"])
        low = {"event": "result", "id": "x", "variant": "base", "status": "ok",
               "tiers": {"540": {"frames": 90, "p50_ms": 11.5, "p95_ms": 12.5}, "360": {"frames": 90, "p50_ms": 5.0, "p95_ms": 6.0}}}
        item = self.assess([heavy, low])
        self.assertEqual((item["outcome"], item["render_height"]), ("qualified", 540))
        lowest = {"event": "result", "id": "x", "variant": "base", "status": "ok",
                  "tiers": {"540": {"frames": 60, "p95_ms": 30.0}, "360": {"frames": 90, "p95_ms": 17.0}}}
        item = self.assess([heavy, lowest])
        self.assertEqual(item["outcome"], "too_heavy")
        self.assertNotIn("not yet measured", item["reason"])

    def test_every_error_status_fails_with_reason(self):
        for status in common.ERROR_STATUSES:
            event = {"event": "result", "id": "x", "variant": "base", "status": status, "error": f"why {status}"}
            if status == "too_slow":
                event["tiers"] = {"720": {"frames": 0, "first_ms": 2100.0}, "1080": {"skipped": "too_slow"}}
            item = self.assess([event])
            self.assertEqual(item["outcome"], "failed", status)
            self.assertEqual(item["reason"], f"{status}: why {status}")
            self.assertIsNone(item["render_height"])
        self.assertEqual(set(common.ALL_STATUSES), {"ok", "prepare_error", "compile_error", "too_slow", "gl_error", "crash", "hang"})

    def test_suspect_pixels_are_not_qualified(self):
        for pixels, flag in (({"mean_luma": 0.0, "spatial_std": 0.0, "temporal_diff": 0.0}, "black"),
                             ({"mean_luma": 0.4, "spatial_std": 0.001, "temporal_diff": 0.1}, "flat"),
                             ({"mean_luma": 0.4, "spatial_std": 0.2, "temporal_diff": 0.0}, "static")):
            item = self.assess([ok_event("x", pixels=pixels)])
            self.assertEqual(item["outcome"], "suspect")
            self.assertIn(flag, item["reason"])
        event = ok_event("x")
        del event["pixels"]
        self.assertEqual(self.assess([event])["outcome"], "suspect")

    def test_texture_and_multipass_are_never_qualified(self):
        fast = [ok_event("x", p95_720=1.0, p95_1080=2.0)]
        item = self.assess(fast, "needs_texture", {"0": "texture"})
        self.assertEqual(item["outcome"], "unverified_texture")
        self.assertIn("texture", item["reason"])
        self.assertEqual(self.assess(fast, "multipass", {"0": "buffer"})["outcome"], "excluded_multipass")
        self.assertEqual(self.assess([], "multipass")["outcome"], "excluded_multipass")
        self.assertEqual(self.assess([], "needs_texture")["outcome"], "unverified_texture")
        self.assertEqual(self.assess([])["outcome"], "not_measured")
        self.assertEqual(self.assess(fast, "original18")["outcome"], "qualified")

    def test_best_variant(self):
        # same tier: the unmodified base wins
        item = self.assess([ok_event("x", "base", 4.0, 9.0), ok_event("x", "lite", 2.0, 5.0)])
        self.assertEqual((item["variant"], item["render_height"]), ("base", 1080))
        # lite reaches a higher tier: lite wins
        item = self.assess([ok_event("x", "base", 6.0, 14.0), ok_event("x", "lite", 4.0, 9.0)])
        self.assertEqual((item["variant"], item["render_height"]), ("lite", 1080))
        # base does not compile, lite does
        broken = {"event": "result", "id": "x", "variant": "base", "status": "compile_error", "error": "e"}
        item = self.assess([broken, ok_event("x", "lite", 4.0, 9.0)])
        self.assertEqual((item["outcome"], item["variant"]), ("qualified", "lite"))
        # a clean variant beats a faster one with suspect pixels
        black = ok_event("x", "lite", 1.0, 2.0, pixels={"mean_luma": 0.0, "spatial_std": 0.0, "temporal_diff": 0.0})
        item = self.assess([ok_event("x", "base", 9.0, 15.0), black])
        self.assertEqual((item["outcome"], item["variant"], item["render_height"]), ("qualified", "base", 720))


class Show(unittest.TestCase):
    def test_layout(self):
        entries = [{"id": f"s{i:03d}", "review_number": (i + 1 if i % 2 else None), "render_height": (720 if i % 3 else 1080),
                    "text": "raymarch" if i < 5 else "plain"} for i in range(30)]
        manifest, groups, dropped = report.build_show(entries)
        self.assertEqual(dropped, [])
        self.assertEqual(len(manifest["shaders"]), 30)
        self.assertEqual(len(manifest["pages"]), 2)
        self.assertEqual(manifest["version"], 2)
        self.assertEqual(set(manifest["shaders"][0]), {"label", "path", "blend", "opacity", "render_height"})
        self.assertTrue(all(s["render_height"] in (360, 540, 720, 1080) for s in manifest["shaders"]))
        first = manifest["pages"][0]
        self.assertEqual(first["name"], "SHADERS 01 / 02")
        self.assertEqual([b["key"] for b in first["bindings"]], list(range(26)) + [31])
        self.assertEqual([b["command"]["type"] for b in first["bindings"][24:]],
                         ["Page", "Page", "Blackout"])
        second = manifest["pages"][1]
        self.assertEqual([b["key"] for b in second["bindings"]], list(range(6)) + [24, 25, 31])
        self.assertEqual([b["command"]["shader"] for b in second["bindings"][:6]], list(range(24, 30)))
        # crate 1 (raymarch keyword) comes first; numbered shaders before unnumbered inside a crate
        self.assertEqual([s["label"] for s in manifest["shaders"][:5]], ["s001", "s003", "s000", "s002", "s004"])
        # keys are captioned with the entry's label, the id when it has none; never truncated
        self.assertEqual(first["bindings"][0]["label"], "s001")
        self.assertEqual(first["bindings"][2]["label"], "s000")
        named, _, _ = report.build_show([{"id": "abc", "render_height": 720, "label": "A much longer shader title",
                                          "thumbnail": "thumbs/abc.jpg"}])
        self.assertEqual(named["pages"][0]["bindings"][0]["label"], "A much longer shader title")
        self.assertEqual(named["shaders"][0]["thumbnail"], "thumbs/abc.jpg")
        self.assertEqual(len(groups["crate_1"]), 5)
        for binding in first["bindings"][:24]:
            shader = manifest["shaders"][binding["command"]["shader"]]
            self.assertTrue(shader["path"].endswith(".fs"))

    def test_engine_limits(self):
        entries = [{"id": f"s{i:04d}", "review_number": None, "render_height": 1080, "text": ""} for i in range(1030)]
        manifest, _, dropped = report.build_show(entries)
        self.assertEqual(len(manifest["shaders"]), report.SHOW_MAX_SHADERS)
        self.assertLessEqual(len(manifest["pages"]), report.SHOW_MAX_PAGES)
        self.assertEqual(len(dropped), 6)


class EndToEnd(Workspace):
    def events(self):
        heavy = ok_event("eeeeee", p95_720=30.0, p95_1080=70.0)
        black = ok_event("ffffff", pixels={"mean_luma": 0.0, "spatial_std": 0.0, "temporal_diff": 0.0})
        return [
            {"event": "begin", "id": "plasma", "variant": "base"}, ok_event("plasma", p95_720=2.0, p95_1080=4.5),
            ok_event("aaaaaa", p95_720=5.0, p95_1080=11.0),
            ok_event("bbbbbb", p95_720=1.0, p95_1080=2.0),
            ok_event("dddddd", "base", 8.0, 18.0), ok_event("dddddd", "lite", 4.0, 9.0),
            heavy, black,
            {"event": "result", "id": "mandel", "variant": "base", "status": "compile_error", "compile_ms": 80.0, "error": "0:1: syntax error"},
            ok_event("ghost!"),
        ]

    def run_report(self, events, *extra):
        results = self.write_results("results.jsonl", events)
        catalog_path = self.root / "catalog.json"
        common.write_json(catalog_path, self.catalog)
        out = self.root / "out"
        with contextlib.redirect_stdout(io.StringIO()):
            code = report.main([str(results), "--catalog", str(catalog_path), "--variants-dir", str(self.variants),
                                "--doc", str(out / "doc.md"), "--qualified", str(out / "qualified.json"),
                                "--show-dir", str(out / "show"), *extra])
        self.assertEqual(code, 0)
        return out

    def test_report_outputs(self):
        out = self.run_report(self.events())
        qualified = json.loads((out / "qualified.json").read_text(encoding="utf-8"))
        self.assertEqual({q["id"]: (q["variant"], q["render_height"]) for q in qualified["qualified"]},
                         {"plasma": ("base", 1080), "aaaaaa": ("base", 1080), "dddddd": ("lite", 1080)})
        self.assertEqual([s["id"] for s in qualified["suspect"]], ["ffffff"])
        self.assertEqual([s["id"] for s in qualified["too_heavy"]], ["eeeeee"])
        self.assertEqual([s["id"] for s in qualified["failed"]], ["mandel"])
        self.assertEqual([s["id"] for s in qualified["unverified_texture"]], ["bbbbbb"])
        self.assertEqual(qualified["excluded_multipass"], ["cccccc"])
        self.assertEqual(qualified["followup"]["measure_lower_tiers"], ["eeeeee"])
        self.assertEqual(sum(qualified["summary"].values()), 8)
        self.assertFalse(qualified["synthetic"])

        show = json.loads((out / "show" / "show.json").read_text(encoding="utf-8"))
        self.assertEqual({s["label"]: s["render_height"] for s in show["shaders"]}, {"plasma": 1080, "aaaaaa": 1080, "dddddd": 1080})
        self.assertEqual(sorted(p.name for p in (out / "show").glob("*.fs")), ["aaaaaa.fs", "dddddd.fs", "plasma.fs"])
        # the show carries the chosen variant's file
        self.assertEqual((out / "show" / "dddddd.fs").read_bytes(), (self.variants / "lite" / "dddddd.fs").read_bytes())
        labels = [b["label"] for b in show["pages"][0]["bindings"][:3]]
        self.assertEqual(sorted(labels), ["Fixture", "Fixture", "plasma"])   # titles; a hand-built shader keeps its file name

        doc = (out / "doc.md").read_text(encoding="utf-8")
        for heading in ("## Summary", "## Reference envelope: the original 18", "## Qualified (3)", "## Renders but suspect (1)",
                        "## Too heavy at the measured tiers (1)", "## Does not work (1)",
                        "## Needs texture: benchmarked, not qualified (1)", "## Multipass: excluded (1)", "## Not measured (0)",
                        "## Variant comparison (1)", "## Result keys without a catalog entry"):
            self.assertIn(heading, doc)
        self.assertIn("compile_error: 0:1: syntax error", doc)
        self.assertIn("| `dddddd` | #001 |", doc)
        self.assertNotIn("SYNTHETIC", doc)

    def test_rerun_removes_shaders_that_no_longer_qualify(self):
        out = self.run_report(self.events())
        events = self.events() + [{"event": "result", "id": "aaaaaa", "variant": "base", "status": "crash", "error": "signal 11"}]
        self.run_report(events)
        self.assertEqual(sorted(p.name for p in (out / "show").glob("*.fs")), ["dddddd.fs", "plasma.fs"])

    def test_synthetic_results_need_the_flag(self):
        events = [dict(e, synthetic=True) for e in self.events()]
        results = self.write_results("s.jsonl", events)
        with self.assertRaises(SystemExit), contextlib.redirect_stderr(io.StringIO()):
            report.main([str(results), "--catalog", str(self.root / "missing.json")])
        out = self.run_report(events, "--synthetic")
        self.assertIn("SYNTHETIC DATA", (out / "doc.md").read_text(encoding="utf-8"))
        self.assertTrue(json.loads((out / "qualified.json").read_text(encoding="utf-8"))["synthetic"])

    def test_synthetic_generator_covers_every_status(self):
        records = []
        for index in range(40):
            record = dict(self.by_id["aaaaaa"])
            record["id"] = f"syn{index:03d}"
            records.append(record)
        records.append(self.by_id["plasma"])
        main_events, low_events = synth_results.generate({"shaders": records}, lite_ids={"syn001"})
        statuses = {e["status"] for e in main_events if e["event"] == "result"}
        self.assertEqual(statuses, set(common.ALL_STATUSES))
        self.assertTrue(all(e["synthetic"] for e in main_events + low_events))
        merged = common.merge_results(main_events + low_events)
        assessments, orphans = report.assess_all({"shaders": records}, merged)
        self.assertEqual(orphans, [])
        self.assertEqual(len(assessments), 41)
        self.assertIn("failed", {a["outcome"] for a in assessments})
        self.assertTrue(report.render_markdown(assessments, {"synthetic": True}).startswith("# Shader bench results"))


HASH_2D, HASH_CUBE, HASH_VOLUME = "aa11", "cc22", "dd33"


def asset_manifest(shaders):
    """Asset store manifest fixture: `shaders` is {id: [(channel, type, asset key or None, sampler overrides)]}."""
    def sampler(**overrides):
        return dict({"filter": "mipmap", "wrap": "repeat", "vflip": True, "srgb": False, "internal": "byte"}, **overrides)
    assets = [
        {"source_path": f"/media/a/{HASH_2D}.png", "type": "texture", "layout": "2d", "status": "ok", "local_file": f"{HASH_2D}.png"},
        {"source_path": f"/media/a/{HASH_CUBE}.jpg", "type": "cubemap", "layout": "cube6", "status": "ok", "local_file": f"{HASH_CUBE}.jpg"},
        {"source_path": f"/media/a/{HASH_VOLUME}.bin", "type": "volume", "layout": "volume3d", "status": "ok", "local_file": f"{HASH_VOLUME}.bin"},
        {"source_path": "/media/a/gone.png", "type": "texture", "layout": "2d", "status": "missing", "local_file": None},
    ]
    document = {"assets": assets, "shaders": {}}
    for shader_id, channels in shaders.items():
        rows = []
        for channel, kind, asset, overrides in channels:
            row = {"channel": channel, "type": kind, "sampler": sampler(**overrides)}
            if asset:
                row["asset"] = f"/media/a/{asset}"
            rows.append(row)
        document["shaders"][shader_id] = {"passes": [{"index": 1, "type": "buffer", "channels": [{"channel": 0, "type": "texture", "asset": f"/media/a/{HASH_2D}.png"}]},
                                                     {"index": 0, "type": "image", "channels": rows}]}
    return document


class ChannelBindings(unittest.TestCase):
    def setUp(self):
        self.manifest = asset_manifest({
            "tex2d": [(1, "texture", f"{HASH_2D}.png", {"filter": "nearest", "vflip": False})],
            "mixed": [(3, "keyboard", None, {}), (0, "cubemap", f"{HASH_CUBE}.jpg", {"filter": "linear", "wrap": "clamp", "vflip": False}),
                      (2, "volume", f"{HASH_VOLUME}.bin", {})],
            "lost": [(0, "texture", "gone.png", {})],
            "none": [],
        })

    def test_bindings_come_from_the_image_pass_with_sampler_settings(self):
        bindings, unbound = common.channel_bindings(self.manifest, "tex2d")
        self.assertEqual(bindings, [{"channel": 1, "file": f"{HASH_2D}.png", "kind": "2d", "filter": "nearest", "wrap": "repeat", "vflip": False}])
        self.assertEqual(unbound, [])
        bindings, unbound = common.channel_bindings(self.manifest, "mixed")
        self.assertEqual([(b["channel"], b["kind"], b["filter"], b["wrap"], b["vflip"]) for b in bindings],
                         [(0, "cube", "linear", "clamp", False), (2, "volume", "mipmap", "repeat", True)])
        self.assertEqual(unbound, [{"channel": 3, "type": "keyboard", "reason": "no file input"}])

    def test_missing_asset_and_unknown_shader_bind_nothing(self):
        bindings, unbound = common.channel_bindings(self.manifest, "lost")
        self.assertEqual((bindings, unbound[0]["reason"]), ([], "asset missing from the store"))
        self.assertEqual(common.channel_bindings(self.manifest, "none"), ([], []))
        self.assertEqual(common.channel_bindings(self.manifest, "absent"), ([], []))
        self.assertEqual(common.channel_bindings(None, "tex2d"), ([], []))

    def test_cubemap_needs_six_files_and_paths_follow_the_root(self):
        bindings, _ = common.channel_bindings(self.manifest, "mixed")
        self.assertEqual(common.binding_files(bindings[0]),
                         [f"{HASH_CUBE}.jpg"] + [f"{HASH_CUBE}_{face}.jpg" for face in range(1, 6)])
        self.assertEqual(common.binding_files(bindings[1]), [f"{HASH_VOLUME}.bin"])
        channels = common.manifest_channels(bindings, "/srv/rpi-vj-bench/assets/")
        self.assertEqual(channels[0], {"channel": 0, "path": f"/srv/rpi-vj-bench/assets/{HASH_CUBE}.jpg", "kind": "cube",
                                       "filter": "linear", "wrap": "clamp", "vflip": False})
        self.assertEqual(common.manifest_channels(bindings, "assets")[1]["path"], f"assets/{HASH_VOLUME}.bin")
        # the spelling the engine parses (engine/src/texture.rs)
        self.assertEqual(set(channels[0]), {"channel", "path", "kind", "filter", "wrap", "vflip"})
        self.assertEqual(set(common.BINDING_KINDS.values()), {"2d", "cube", "volume"})

    def test_real_asset_manifest_if_present(self):
        document = common.load_asset_manifest()
        if document is None:
            self.skipTest("asset store not built")
        bound = 0
        for shader_id in document["shaders"]:
            bindings, _ = common.channel_bindings(document, shader_id)
            self.assertLessEqual(len(bindings), 4)
            self.assertEqual(len({b["channel"] for b in bindings}), len(bindings))
            for binding in bindings:
                self.assertIn(binding["filter"], ("mipmap", "linear", "nearest"))
                self.assertIn(binding["wrap"], ("repeat", "clamp"))
                for name in common.binding_files(binding):
                    self.assertTrue((common.ASSETS_DIR / name).is_file(), name)
            bound += bool(bindings)
        self.assertGreater(bound, 0)


class ChannelManifests(Workspace):
    def setUp(self):
        super().setUp()
        self.assets = asset_manifest({"bbbbbb": [(0, "texture", f"{HASH_2D}.png", {}), (1, "cubemap", f"{HASH_CUBE}.jpg", {"wrap": "clamp", "vflip": False})],
                                      "aaaaaa": []})

    def test_entries_carry_bindings_with_pi_paths(self):
        entries, _ = make_manifest.select(self.catalog, ids=["aaaaaa", "bbbbbb"], variants_dir=self.variants)
        bindings = make_manifest.entry_bindings(entries, self.assets)
        self.assertEqual(list(bindings), ["bbbbbb"])
        manifest = make_manifest.build_manifest(entries, common.parse_tiers("720,1080"), bindings=bindings)
        plain, textured = manifest["entries"]
        self.assertEqual(plain, {"id": "aaaaaa", "variant": "base", "path": "/srv/rpi-vj-bench/shaders/base/aaaaaa.fs"})
        self.assertEqual(textured["channels"], [
            {"channel": 0, "path": f"/srv/rpi-vj-bench/assets/{HASH_2D}.png", "kind": "2d", "filter": "mipmap", "wrap": "repeat", "vflip": True},
            {"channel": 1, "path": f"/srv/rpi-vj-bench/assets/{HASH_CUBE}.jpg", "kind": "cube", "filter": "mipmap", "wrap": "clamp", "vflip": False}])
        self.assertEqual(make_manifest.asset_files(entries, bindings),
                         sorted([f"{HASH_2D}.png", f"{HASH_CUBE}.jpg"] + [f"{HASH_CUBE}_{face}.jpg" for face in range(1, 6)]))
        # a control manifest is the plain section 3.1 shape
        control = make_manifest.build_manifest(entries, common.parse_tiers("720,1080"))
        self.assertTrue(all(set(e) == {"id", "variant", "path"} for e in control["entries"]))

    def test_stage_writes_the_asset_upload_list(self):
        entries, _ = make_manifest.select(self.catalog, ids=["bbbbbb"], variants_dir=self.variants)
        bindings = make_manifest.entry_bindings(entries, self.assets)
        assets = make_manifest.asset_files(entries, bindings)
        stage_dir = self.root / "staging" / "tex"
        make_manifest.stage(entries, make_manifest.build_manifest(entries, common.parse_tiers("720"), bindings=bindings), stage_dir, assets)
        self.assertEqual((stage_dir / "assets.txt").read_text(encoding="utf-8").split(), assets)
        self.assertEqual(json.loads((stage_dir / "staging-index.json").read_text(encoding="utf-8"))["assets"], assets)
        self.assertFalse((stage_dir / "assets").exists())              # the store is the source; nothing is copied

    def test_first_variant_gives_one_entry_per_shader(self):
        entries, _ = make_manifest.select(self.catalog, classes=["selfcontained"], variants=["lite", "base"],
                                          variants_dir=self.variants, first_only=True)
        chosen = {r["id"]: v for r, v, _ in entries}
        self.assertEqual(len(entries), len(chosen))
        self.assertEqual(chosen["dddddd"], "lite")                     # has a lite file
        self.assertEqual(chosen["aaaaaa"], "base")                     # falls back
        done = {("dddddd", "lite")}
        entries, _ = make_manifest.select(self.catalog, ids=["dddddd"], variants=["lite", "base"], variants_dir=self.variants,
                                          first_only=True, skip_keys=done)
        self.assertEqual(entries, [])                                  # done in its preferred variant: not re-run as base

    def test_main_writes_bound_and_control_manifests(self):
        catalog_path, assets_path = self.root / "catalog.json", self.root / "assets.json"
        common.write_json(catalog_path, self.catalog)
        common.write_json(assets_path, self.assets)
        base = ["--class", "needs_texture", "--catalog", str(catalog_path), "--variants-dir", str(self.variants),
                "--staging-root", str(self.root / "staging"), "--asset-manifest", str(assets_path)]
        with contextlib.redirect_stdout(io.StringIO()) as out:
            self.assertEqual(make_manifest.main(base + ["--name", "bound"]), 0)
            self.assertEqual(make_manifest.main(base + ["--name", "control", "--no-channels"]), 0)
        bound = json.loads((self.root / "staging" / "bound" / "manifest.json").read_text(encoding="utf-8"))
        control = json.loads((self.root / "staging" / "control" / "manifest.json").read_text(encoding="utf-8"))
        self.assertEqual([len(e.get("channels", [])) for e in bound["entries"]], [2])
        self.assertEqual([set(e) for e in control["entries"]], [{"id", "variant", "path"}])
        self.assertIn("/srv/rpi-vj-bench/assets/", out.getvalue())


class CoordinateRestore(unittest.TestCase):
    CUBE = "vec3 sky(vec3 dir){ return texture(iChannel0, dir.xy).rgb; }\nvoid mainImage(out vec4 o, in vec2 u){ o = vec4(sky(vec3(u,1.)),1.) + texture(iChannel1, u); }"

    def record(self, **kinds):
        return {"id": "x", "channels": {"kinds": {str(k): v for k, v in kinds.items()}}}

    def test_cut_coordinates_come_back_on_cube_and_volume_channels(self):
        out, applied = adapt.transform(native(self.CUBE), adapt.VARIANT_RULES["tex"], self.record(**{"0": "cubemap", "1": "texture"}))
        self.assertIn("texture(iChannel0, dir).rgb", out)
        self.assertIn("texture(iChannel1, u)", out)
        self.assertEqual([(a["rule"], a["count"]) for a in applied], [("restore_3d_coords", 1)])
        self.assertEqual(applied[0]["edits"][0]["after"], "vec3 sky(vec3 dir){ return texture(iChannel0, dir).rgb; }")
        # the header is not touched: the engine sets the sampler type from the binding
        self.assertEqual(out.count("uniform sampler2D iChannel"), 4)
        volume = native("#define N(p) texture(iChannel0,(p).xy/18.).r\nfloat f(vec3 s){ return texture(iChannel0, s.xy * 0.014 + vec2(0.618)).r; }")
        out, _ = adapt.transform(volume, ["restore_3d_coords"], self.record(**{"0": "volume"}))
        self.assertIn("texture(iChannel0,(p)/18.)", out)
        self.assertIn("texture(iChannel0, s * 0.014 + vec3(0.618))", out)

    def test_two_dimensional_and_unknown_channels_are_left_alone(self):
        text = native(self.CUBE)
        for record in (self.record(**{"0": "texture"}), self.record(**{"1": "cubemap"}), self.record(), None, {"id": "x"}):
            self.assertEqual(adapt.transform(text, ["restore_3d_coords"], record), (text, []), record)
        # the lite rules never see the record and still work with one
        self.assertEqual(adapt.transform(text, adapt.VARIANT_RULES["lite"], self.record(**{"0": "cubemap"}))[1], [])

    def test_macro_rewrites_without_a_channel_name_need_a_spatial_channel(self):
        text = native("#define T texture(iChannel0//\n#define M (T,a.xy/180.).r)")
        out, applied = adapt.transform(text, ["restore_3d_coords"], self.record(**{"0": "volume"}))
        self.assertIn("(T,a/180.).r)", out)
        self.assertEqual(adapt.transform(text, ["restore_3d_coords"], self.record(**{"0": "texture"}))[1], [])

    def test_restore_inverts_the_converter(self):
        import make_native_shadertoy as converter
        for before, after in converter.COORD_REWRITES:
            self.assertEqual(before.replace(before, after), after)
            channels = set(adapt._CHANNEL_RE.findall(after)) or {"0"}
            record = self.record(**{c: "volume" for c in channels})
            self.assertEqual(adapt.rule_restore_3d_coords(f"x = {after};", f"x = {after};", record)[0].count(".xy"),
                             before.count(".xy"), after)

    def test_converter_types_channels_from_metadata_and_keeps_coordinates(self):
        import make_native_shadertoy as converter
        code = "void mainImage(out vec4 o, in vec2 u){ vec3 dir=vec3(u,1.); o = texture(iChannel0, dir) + texture(iChannel2, dir.xzy); }"
        document = {"info": {"name": "Sky"}, "renderpass": [{"code": code, "type": "image", "inputs": [
            {"channel": 0, "ctype": "cubemap"}, {"channel": 2, "ctype": "volume"}, {"channel": 1, "ctype": "texture"}]}]}
        text, label = converter.convert("abc123", document=document)
        self.assertEqual(label, "Sky")
        for line in ("uniform samplerCube iChannel0;", "uniform sampler2D iChannel1;", "uniform sampler3D iChannel2;", "uniform sampler2D iChannel3;"):
            self.assertIn(line, text)
        self.assertIn("texture(iChannel0, dir)", text)
        self.assertNotIn("dir.xy", text)
        legacy, _ = converter.convert("abc123", document=document, typed_channels=False)
        self.assertEqual(legacy.count("uniform sampler2D iChannel"), 4)
        self.assertIn("texture(iChannel0, dir.xy)", legacy)
        self.assertTrue(legacy.startswith(converter.HEAD))
        self.assertTrue(converter.HEAD.endswith(common.SHADERTOY_HEAD_END + "\n"))
        plain, _ = converter.convert("abc123", document={"renderpass": [{"code": code.replace("iChannel2", "iChannel0"), "inputs": []}]})
        self.assertTrue(plain.startswith(converter.HEAD))


class TextureAssessment(unittest.TestCase):
    def assess(self, events, kinds):
        record = {"id": "x", "class": "needs_texture", "channels": {"kinds": kinds}, "name": "X", "review_number": 7}
        merged = common.merge_results(events)
        return report.assess_shader(record, {v: r for (i, v), r in merged.items()})

    def test_bound_measurement_qualifies(self):
        item = self.assess([ok_event("x", p95_720=4.0, p95_1080=9.0, channels={"0": "2d"})], {"0": "texture"})
        self.assertEqual((item["outcome"], item["render_height"], item["variant"]), ("qualified", 1080, "base"))
        item = self.assess([ok_event("x", "tex", 4.0, 14.0, channels={"0": "cube", "2": "volume"})], {"0": "cubemap", "2": "volume"})
        self.assertEqual((item["outcome"], item["render_height"], item["variant"]), ("qualified", 720, "tex"))

    def test_bound_measurement_is_judged_like_any_shader(self):
        black = {"mean_luma": 0.0, "spatial_std": 0.0, "temporal_diff": 0.0}
        self.assertEqual(self.assess([ok_event("x", pixels=black, channels={"0": "2d"})], {"0": "texture"})["outcome"], "suspect")
        self.assertEqual(self.assess([ok_event("x", p95_720=30.0, p95_1080=60.0, channels={"0": "2d"})], {"0": "texture"})["outcome"], "too_heavy")
        broken = {"event": "result", "id": "x", "variant": "base", "status": "gl_error", "error": "e", "channels": {"0": "2d"}}
        self.assertEqual(self.assess([broken], {"0": "texture"})["outcome"], "failed")

    def test_unbound_partly_bound_and_wrong_kind_stay_unverified(self):
        fast = dict(p95_720=1.0, p95_1080=2.0)
        item = self.assess([ok_event("x", **fast)], {"0": "texture"})
        self.assertEqual(item["outcome"], "unverified_texture")
        self.assertIn("measured without its channel textures (ch0=texture)", item["reason"])
        self.assertEqual(item["render_height"], 1080)                      # the indicative tier is still reported
        item = self.assess([ok_event("x", channels={"0": "2d"}, **fast)], {"0": "texture", "1": "cubemap"})
        self.assertEqual((item["outcome"], "ch1=cubemap" in item["reason"]), ("unverified_texture", True))
        self.assertEqual(self.assess([ok_event("x", channels={"0": "2d"}, **fast)], {"0": "cubemap"})["outcome"], "unverified_texture")
        self.assertEqual(self.assess([], {"0": "volume"})["reason"], "not measured (reads ch0=volume)")

    def test_inputs_without_a_file_can_never_qualify(self):
        for kind in ("keyboard", "musicstream", "music", "mic", "video", "webcam", "buffer", "undeclared", "unknown"):
            item = self.assess([ok_event("x", channels={"0": "2d"})], {"0": "texture", "1": kind})
            self.assertEqual(item["outcome"], "unverified_texture", kind)
            self.assertIn(f"cannot supply (ch1={kind})", item["reason"])

    def test_bound_variant_wins_over_an_unbound_one(self):
        events = [ok_event("x", "base", 1.0, 2.0), ok_event("x", "tex", 6.0, 12.0, channels={"0": "volume"})]
        item = self.assess(events, {"0": "volume"})
        self.assertEqual((item["outcome"], item["variant"], item["render_height"]), ("qualified", "tex", 1080))

    def test_heavy_bound_texture_shader_is_listed_for_lower_tiers(self):
        heavy = self.assess([ok_event("x", p95_720=20.0, p95_1080=45.0, channels={"0": "2d"})], {"0": "texture"})
        self.assertEqual(heavy["outcome"], "too_heavy")
        unbound = self.assess([ok_event("x", p95_720=20.0, p95_1080=45.0)], {"0": "texture"})
        for item in (heavy, unbound):
            item["catalog"] = {"id": "x"}
        self.assertEqual(report.followups([heavy, unbound], {})["measure_lower_tiers"], ["x"])

    def test_latest_line_decides_whether_a_key_is_bound(self):
        bound, unbound = ok_event("x", channels={"0": "2d"}), ok_event("x")
        self.assertEqual(common.merge_results([unbound, bound])[("x", "base")]["channels"], {"0": "2d"})
        self.assertEqual(common.merge_results([bound, unbound])[("x", "base")]["channels"], {})
        self.assertEqual(self.assess([bound, unbound], {"0": "texture"})["outcome"], "unverified_texture")


class TextureShow(Workspace):
    events = EndToEnd.events
    run_report = EndToEnd.run_report

    def test_qualified_texture_shader_ships_with_bindings_and_files(self):
        store = self.root / "store"
        store.mkdir()
        (store / f"{HASH_2D}.png").write_bytes(b"png")
        assets_path = self.root / "assets.json"
        common.write_json(assets_path, asset_manifest({"bbbbbb": [(0, "texture", f"{HASH_2D}.png", {"filter": "linear"})]}))
        events = [e for e in self.events() if e.get("id") != "bbbbbb"] + [ok_event("bbbbbb", p95_720=3.0, p95_1080=12.0, channels={"0": "2d"})]
        out = self.run_report(events, "--asset-manifest", str(assets_path), "--assets-dir", str(store))
        qualified = json.loads((out / "qualified.json").read_text(encoding="utf-8"))
        self.assertIn("bbbbbb", [q["id"] for q in qualified["qualified"]])
        self.assertEqual(qualified["unverified_texture"], [])
        self.assertEqual(next(q for q in qualified["qualified"] if q["id"] == "bbbbbb")["channels"], {"0": "2d"})
        show = json.loads((out / "show" / "show.json").read_text(encoding="utf-8"))
        shader = next(s for s in show["shaders"] if s["label"] == "bbbbbb")
        self.assertEqual(shader["channels"], [{"channel": 0, "path": f"assets/{HASH_2D}.png", "kind": "2d", "filter": "linear",
                                               "wrap": "repeat", "vflip": True}])
        self.assertEqual((out / "show" / "assets" / f"{HASH_2D}.png").read_bytes(), b"png")
        # shaders without bindings keep the exact previous entry shape
        self.assertEqual(set(next(s for s in show["shaders"] if s["label"] == "aaaaaa")), {"label", "path", "blend", "opacity", "render_height"})
        self.assertIn("## Needs texture: benchmarked, not qualified (0)", (out / "doc.md").read_text(encoding="utf-8"))
        # measured unbound again later: out of the show, texture file removed
        self.run_report(events + [ok_event("bbbbbb", p95_720=3.0, p95_1080=12.0)], "--asset-manifest", str(assets_path), "--assets-dir", str(store))
        show = json.loads((out / "show" / "show.json").read_text(encoding="utf-8"))
        self.assertNotIn("bbbbbb", [s["label"] for s in show["shaders"]])
        self.assertEqual(list((out / "show" / "assets").iterdir()), [])

    def test_texture_shader_without_its_file_is_left_out_of_the_show(self):
        assets_path = self.root / "assets.json"
        common.write_json(assets_path, asset_manifest({"bbbbbb": [(0, "texture", f"{HASH_2D}.png", {})]}))
        events = [e for e in self.events() if e.get("id") != "bbbbbb"] + [ok_event("bbbbbb", channels={"0": "2d"})]
        out = self.run_report(events, "--asset-manifest", str(assets_path), "--assets-dir", str(self.root / "empty"))
        show = json.loads((out / "show" / "show.json").read_text(encoding="utf-8"))
        self.assertNotIn("bbbbbb", [s["label"] for s in show["shaders"]])
        self.assertFalse((out / "show" / "bbbbbb.fs").exists())


def legacy(body: str, newline: str = "\n") -> str:
    """A bank file as the converter wrote it: real header, real legacy wrapper."""
    return (make_native_shadertoy.HEAD + body + make_native_shadertoy.WRAPPER).replace("\n", newline)


class ReactWrapper(unittest.TestCase):
    REACT = adapt.VARIANT_RULES["react"]

    def test_replaces_exactly_the_legacy_wrapper(self):
        for newline in ("\n", "\r\n"):
            text = legacy(PLAIN, newline)
            out, applied = adapt.transform(text, self.REACT)
            self.assertEqual([a["rule"] for a in applied], ["react_wrapper"])
            cut = text.index(make_native_shadertoy.WRAPPER.replace("\n", newline))
            # everything before the wrapper is byte-identical, everything after is the new wrapper
            self.assertEqual(out[:cut], text[:cut])
            self.assertEqual(out[cut:], make_native_shadertoy.react_wrapper("normal").replace("\n", newline))
            self.assertEqual(out.count("void main("), 1)
            self.assertNotIn("audio_zoom", out)
            self.assertNotIn("\r\r", out)
            if newline == "\r\n":
                self.assertNotRegex(out, r"[^\r]\n")
            edit = applied[0]["edits"][0]
            self.assertEqual((edit["mode"], edit["reasons"], edit["direct_fragcoord"]), ("normal", [], False))

    def test_idempotent(self):
        once, _ = adapt.transform(legacy(PLAIN), self.REACT)
        twice, applied = adapt.transform(once, self.REACT)
        self.assertEqual((twice, applied), (once, []))
        self.assertEqual(adapt.react_skip_reason(once), "already has the react wrapper")

    def test_other_wrappers_and_own_main_are_left_alone(self):
        for text, reason in ((native(PLAIN), "differs"), ('uniform float u_time;\nvoid main(){ gl_FragColor=vec4(1.); }', "own main"),
                             (legacy(PLAIN).replace("0.28*u_bass", "0.30*u_bass"), "differs")):
            out, applied = adapt.transform(text, self.REACT)
            self.assertEqual((out, applied), (text, []))
            self.assertIn(reason, adapt.react_skip_reason(text))
        # the wrapper text inside a comment or mid-file is not the wrapper
        mid = legacy(PLAIN) + "\nfloat tail;\n"
        self.assertEqual(adapt.transform(mid, self.REACT), (mid, []))

    def test_shader_that_reads_audio_uniforms_is_reported_not_wrapped(self):
        text = legacy("void mainImage(out vec4 o, in vec2 u){ o = vec4(u_bass + u_flow_low); }")
        self.assertEqual(adapt.transform(text, self.REACT), (text, []))
        self.assertEqual(adapt.react_skip_reason(text), "reads audio uniforms itself: u_bass, u_flow_low")
        # a mention in a comment, or of the time uniform, is not audio
        quiet = legacy("// u_bass\nvoid mainImage(out vec4 o, in vec2 u){ o = vec4(sin(u_time)); }")
        self.assertIsNone(adapt.react_skip_reason(quiet))

    def test_pixel_exact_shaders_get_the_whole_pixel_mode(self):
        cases = {
            "void mainImage(out vec4 o, in vec2 u){ o = texelFetch(iChannel0, ivec2(3,4), 0); }": ["texelFetch"],
            "void mainImage(out vec4 o, vec2 fc){ ivec2 p = ivec2(fc); o = vec4(p.x & p.y); }": ["ivec2(fc)"],
            "void mainImage(out vec4 O, vec2 I){ O = vec4(int(I.x) ^ int(I.y)); }": ["int(I)"],
            "void mainImage(out vec4 o, in vec2 u){ o = vec4(mod(u.y, 2.)); }": ["mod(u)"],
            "void mainImage(out vec4 o, in vec2 u){ o = vec4(floor(u.x)); }": ["floor(u)"],
        }
        for body, reasons in cases.items():
            out, applied = adapt.transform(legacy(body), self.REACT)
            edit = applied[0]["edits"][0]
            self.assertEqual((edit["mode"], edit["reasons"]), ("pixel", reasons), body)
            wrapper = out[out.index(make_native_shadertoy.REACT_MARKER):]
            self.assertIn("floor(", wrapper)
            self.assertIn("u_resolution - 0.5", wrapper)
            for absent in ("rv_wave", "RV_SWAY *", "rv_sn"):        # no rotation, no ripple
                self.assertNotIn(absent, wrapper.split("void main")[1], body)
        # scaled or normalised coordinates are not pixel-exact
        for body in ("void mainImage(out vec4 o, in vec2 u){ o = vec4(floor(u / 8.)); }",
                     "void mainImage(out vec4 o, in vec2 u){ vec2 p = u / u_resolution; o = vec4(mod(p.x * 9., 1.)); }", PLAIN):
            self.assertEqual(make_native_shadertoy.react_analysis(body)["mode"], "normal", body)

    def test_direct_gl_fragcoord_reads_are_redirected(self):
        body = "void mainImage(out vec4 o, in vec2 u){ vec2 uv = gl_FragCoord.xy / u_resolution; o = vec4(uv, 0, 1); }"
        out, applied = adapt.transform(legacy(body), self.REACT)
        self.assertTrue(applied[0]["edits"][-1]["direct_fragcoord"])
        head_end = out.index(common.SHADERTOY_HEAD_END) + len(common.SHADERTOY_HEAD_END)
        self.assertTrue(out[head_end:].startswith("\nvec4 rv_FragCoord;\n#define gl_FragCoord rv_FragCoord\n"))
        wrapper = out[out.index(make_native_shadertoy.REACT_MARKER):]
        self.assertLess(wrapper.index("#undef gl_FragCoord"), wrapper.index("void main"))
        self.assertLess(wrapper.index("rv_FragCoord = vec4(rv_q, gl_FragCoord.zw);"), wrapper.index("mainImage(rv_out"))
        self.assertEqual(adapt.transform(out, self.REACT), (out, []))
        self.assertNotIn("rv_FragCoord", adapt.transform(legacy(PLAIN), self.REACT)[0])

    def test_macros_named_like_builtins_are_undefined_first(self):
        out, _ = adapt.transform(legacy("#define max(a,b,c) (a)\n" + PLAIN), self.REACT)
        wrapper = out[out.index(make_native_shadertoy.REACT_MARKER):]
        self.assertLess(wrapper.index("#undef max"), wrapper.index("void main"))
        self.assertNotIn("#undef", adapt.transform(legacy(PLAIN), self.REACT)[0])

    def test_wrapper_costs_no_trig_or_pow_and_has_a_low_floor(self):
        for mode in make_native_shadertoy.REACT_MODES:
            code = common.strip_comments(make_native_shadertoy.react_wrapper(mode))
            for call in ("sin", "cos", "tan", "atan", "exp", "pow", "log", "sqrt", "normalize", "length"):
                self.assertNotRegex(code, rf"\b{call}\s*\(", f"{mode}: {call}")
            self.assertIn("const float RV_FLOOR = 0.15;", code)
            self.assertNotIn("u_time", code)                       # motion comes from the flow clocks
            self.assertEqual(code.count("mainImage("), 1)
        normal = make_native_shadertoy.react_wrapper("normal")
        for uniform in ("u_flow_low", "u_flow_mid", "u_flow_high", "u_hit_low", "u_hit_mid", "u_hit_high", "u_onset",
                        "u_bass", "u_sub", "u_treble", "u_high_mid"):
            self.assertIn(uniform, normal)
        # the legacy wrapper is what the comparison is against
        old = make_native_shadertoy.WRAPPER
        self.assertEqual((old.count("sin(") + old.count("cos("), old.count("pow(")), (7, 1))

    def test_composes_with_lite_and_tex(self):
        aa = "#define AA 2\nvoid mainImage(out vec4 o, in vec2 u){ for(int m=0;m<AA;m++) o+=1.; }"
        out, applied = adapt.transform(legacy(aa), adapt.VARIANT_RULES["react_lite"])
        self.assertEqual([a["rule"] for a in applied], ["aa_samples_1", "react_wrapper"])
        self.assertIn("#define AA 1", out)
        self.assertIn(make_native_shadertoy.REACT_MARKER, out)
        # order does not matter for the result: lite on a react file gives the same text
        react_first, _ = adapt.transform(legacy(aa), self.REACT)
        self.assertEqual(adapt.transform(react_first, adapt.VARIANT_RULES["lite"])[0], out)
        # tex: the cut cubemap coordinate comes back and the wrapper is replaced
        cube = "void mainImage(out vec4 o, in vec2 u){ vec3 dir = vec3(u, 1.); o = texture(iChannel0, dir.xy); }"
        record = {"channels": {"kinds": {"0": "cubemap"}}}
        out, applied = adapt.transform(legacy(cube), self.REACT, record)
        self.assertEqual([a["rule"] for a in applied], ["restore_3d_coords", "react_wrapper"])
        self.assertIn("texture(iChannel0, dir)", out)
        self.assertEqual(adapt.transform(out, self.REACT, record), (out, []))

    def test_converter_can_emit_the_react_wrapper(self):
        document = {"info": {"name": "N"}, "renderpass": [{"type": "image", "code": "void mainImage(out vec4 o, in vec2 u){ o = vec4(iTime); }", "inputs": []}]}
        old, _ = make_native_shadertoy.convert("x", document=document)
        new, _ = make_native_shadertoy.convert("x", document=document, wrapper="react")
        self.assertTrue(old.endswith(make_native_shadertoy.WRAPPER))
        self.assertTrue(new.endswith(make_native_shadertoy.react_wrapper("normal")))
        # converting with the legacy wrapper and adapting gives the same file
        self.assertEqual(adapt.transform(old, self.REACT)[0], new)
        # the catalog still finds where the shader's own code ends
        start, end = common.split_shadertoy(new)
        self.assertEqual(new[start:end].strip(), "void mainImage(out vec4 o, in vec2 u){ o = vec4(u_time); }")
        with self.assertRaises(ValueError):
            make_native_shadertoy.convert("x", document=document, wrapper="other")


class ReactVariants(Workspace):
    def setUp(self):
        super().setUp()
        aa_body = "#define AA 2\nvoid mainImage(out vec4 o, in vec2 u){ for(int m=0;m<AA;m++) for(int n=0;n<AA;n++) o+=1.; }"
        (self.native / "aaaaaa.fs").write_text(legacy(PLAIN), encoding="utf-8", newline="")
        (self.native / "dddddd.fs").write_text(legacy(aa_body), encoding="utf-8", newline="")
        (self.native / "eeeeee.fs").write_text(legacy("void mainImage(out vec4 o, in vec2 u){ o = vec4(u_bass); }"),
                                              encoding="utf-8", newline="")
        (self.native / "ffffff.fs").write_text(legacy("void mainImage(out vec4 o, in vec2 u){ o = texelFetch(iChannel0, ivec2(u), 0); }"),
                                              encoding="utf-8", newline="")
        self.reports = {v: adapt.build_variant(v, self.catalog, self.variants, self.root) for v in ("react", "react_lite")}

    def test_react_files_only_for_legacy_wrapped_shaders_and_everything_is_reported(self):
        self.assertEqual(sorted(p.name for p in (self.variants / "react").glob("*.fs")), ["aaaaaa.fs", "dddddd.fs", "ffffff.fs"])
        document = json.loads((self.variants / "react" / "transforms.json").read_text(encoding="utf-8"))
        react = document["react"]
        self.assertEqual(react["modes"], {"normal": 2, "pixel": 1})
        self.assertEqual(react["pixel_mode"], {"ffffff": ["texelFetch", "ivec2(u)"]})
        self.assertEqual(react["gl_fragcoord_redirected"], [])
        reasons = {i: v["reason"] for i, v in react["not_wrapped"].items()}
        self.assertEqual(reasons["eeeeee"], "reads audio uniforms itself: u_bass")
        self.assertIn("own main", reasons["plasma"])
        self.assertIn("own main", reasons["mandel"])
        self.assertIn("differs", reasons["bbbbbb"])         # the fixture wrapper is not the converter's
        self.assertEqual(document["summary"]["not_wrapped"], len(reasons))
        # the source library is never written to
        self.assertTrue((self.native / "aaaaaa.fs").read_text(encoding="utf-8").endswith(make_native_shadertoy.WRAPPER))

    def test_react_lite_needs_both_a_lite_transform_and_the_wrapper(self):
        self.assertEqual(sorted(p.name for p in (self.variants / "react_lite").glob("*.fs")), ["dddddd.fs"])
        text = (self.variants / "react_lite" / "dddddd.fs").read_text(encoding="utf-8")
        self.assertIn("#define AA 1", text)
        self.assertIn(make_native_shadertoy.REACT_MARKER, text)
        again = adapt.build_variant("react", self.catalog, self.variants, self.root)
        self.assertEqual((again["files_written"], again["files_removed"]), (0, 0))

    def test_manifest_can_take_react_first(self):
        entries, _ = make_manifest.select(self.catalog, ids=["aaaaaa", "dddddd", "eeeeee"], variants=("react_lite", "react", "base"),
                                          variants_dir=self.variants, first_only=True)
        adapt.build_variant("base", self.catalog, self.variants, self.root)
        entries, _ = make_manifest.select(self.catalog, ids=["aaaaaa", "dddddd", "eeeeee"], variants=("react_lite", "react", "base"),
                                          variants_dir=self.variants, first_only=True)
        self.assertEqual([(r["id"], v) for r, v, _ in entries], [("aaaaaa", "react"), ("dddddd", "react_lite"), ("eeeeee", "base")])


def tier(p95, frames=90):
    return {"frames": frames, "mean_ms": p95 * 0.9, "p50_ms": p95 * 0.9, "p95_ms": p95, "max_ms": p95 * 1.2, "first_ms": p95 * 2}


def two_pattern_event(shader_id, variant="base", beat=(4.0, 9.0), loud=(5.0, 10.0), **extra):
    """A result line of the engine with the worst-case audio extension."""
    event = ok_event(shader_id, variant, beat[0], beat[1], audio=["beat128", "max"],
                     tiers_max={"720": tier(loud[0]), "1080": tier(loud[1])},
                     pixels_max={"mean_luma": 0.5, "spatial_std": 0.2, "temporal_diff": 0.05},
                     thumb_max=f"{shader_id}.{variant}.max.ppm")
    event.update(extra)
    return event


class TwoPatternVerdict(unittest.TestCase):
    def assess(self, events, **kwargs):
        record = {"id": "x", "class": "selfcontained", "channels": {"kinds": {}}, "name": "X", "review_number": 7}
        merged = common.merge_results(events)
        return report.assess_shader(record, {v: r for (i, v), r in merged.items()}, **kwargs)

    def test_combined_verdict_is_the_worse_of_the_two(self):
        self.assertEqual(report.combined_verdict("SOLID", "SOLID"), "SOLID")
        self.assertEqual(report.combined_verdict("SOLID", "OK"), "OK")
        self.assertEqual(report.combined_verdict("OK", "SOLID"), "OK")
        self.assertEqual(report.combined_verdict("SOLID", "FAIL"), "FAIL")
        self.assertEqual(report.combined_verdict("MARGINAL", "OK"), "MARGINAL")
        self.assertEqual(report.combined_verdict("SOLID", None), "SOLID")            # old results: one pattern
        self.assertEqual(report.combined_verdict("SOLID", "SKIPPED"), "SKIPPED")     # attempted, no measurement
        self.assertEqual(report.combined_verdict("SOLID", "NO_DATA"), "NO_DATA")
        self.assertEqual(report.combined_verdict("SKIPPED", "SOLID"), "SKIPPED")

    def test_a_tier_qualifies_only_within_budget_under_both_patterns(self):
        both = self.assess([two_pattern_event("x", beat=(4.0, 9.0), loud=(5.0, 12.9))])
        self.assertEqual((both["outcome"], both["render_height"]), ("qualified", 1080))
        self.assertEqual(both["chosen"]["verdicts"], {720: "SOLID", 1080: "OK"})
        self.assertEqual(both["chosen"]["verdicts_beat"][1080], "SOLID")
        self.assertEqual(both["chosen"]["one_pattern_tiers"], [])
        # fine on the beat pattern, over budget when everything is loud: 1080p is lost
        loud = self.assess([two_pattern_event("x", beat=(4.0, 9.0), loud=(6.0, 13.1))])
        self.assertEqual((loud["outcome"], loud["render_height"]), ("qualified", 720))
        self.assertEqual(loud["chosen"]["verdicts"][1080], "MARGINAL")
        # over budget under worst-case audio at every tier: not qualified at all
        heavy = self.assess([two_pattern_event("x", beat=(9.0, 12.0), loud=(14.0, 30.0))])
        self.assertEqual(heavy["outcome"], "too_heavy")
        self.assertIn("720p/1080p", heavy["reason"])
        # the other way round counts too
        beat = self.assess([two_pattern_event("x", beat=(9.0, 17.0), loud=(5.0, 9.0))])
        self.assertEqual(beat["render_height"], 720)

    def test_worst_case_attempt_without_a_measurement_does_not_qualify(self):
        event = two_pattern_event("x")
        event["tiers_max"]["1080"] = {"skipped": "gpu_reset", "first_ms": 1001.0}
        self.assertEqual(self.assess([event])["render_height"], 720)
        event["tiers_max"]["720"] = tier(5.0, frames=2)
        self.assertNotEqual(self.assess([event])["outcome"], "qualified")

    def test_old_results_are_read_and_judged_on_one_pattern_unless_required(self):
        old = ok_event("x", p95_720=4.0, p95_1080=9.0)
        item = self.assess([old])
        self.assertEqual((item["outcome"], item["render_height"]), ("qualified", 1080))
        self.assertEqual(item["chosen"]["one_pattern_tiers"], [720, 1080])
        self.assertIn("128 BPM pattern only", report.shader_row(dict(item, catalog={"id": "x", "name": "X"}))[-1])
        strict = self.assess([old], require_worst_case=True)
        self.assertEqual(strict["outcome"], "not_measured")
        self.assertIn("worst-case audio is required", strict["reason"])
        self.assertEqual(strict["chosen"]["verdicts"], {720: "ONE_PATTERN", 1080: "ONE_PATTERN"})
        # a two-pattern result is unaffected by the switch
        self.assertEqual(self.assess([two_pattern_event("x")], require_worst_case=True)["render_height"], 1080)

    def test_merge_never_pairs_patterns_from_different_runs(self):
        first = two_pattern_event("x", beat=(4.0, 9.0), loud=(5.0, 30.0))
        # a later single-pattern line for 1080 replaces both patterns of that tier, and keeps 720
        later = {"event": "result", "id": "x", "variant": "base", "status": "ok", "tiers": {"1080": tier(8.0)}}
        record = common.merge_results([first, later])[("x", "base")]
        self.assertEqual(sorted(record["tiers_max"]), ["720"])
        self.assertEqual(record["tiers"]["1080"]["p95_ms"], 8.0)
        self.assertEqual(record["pixels_max"]["mean_luma"], 0.5)        # no new picture: the old pair stands
        # a later line with its own picture but no worst-case picture drops the stale one
        repaint = ok_event("x")
        record = common.merge_results([first, repaint])[("x", "base")]
        self.assertNotIn("pixels_max", record)
        self.assertNotIn("thumb_max", record)
        self.assertEqual(record["tiers_max"], {})
        # and the newest two-pattern line wins as a whole
        record = common.merge_results([repaint, first])[("x", "base")]
        self.assertEqual(record["tiers_max"]["1080"]["p95_ms"], 30.0)
        self.assertEqual(record["thumb_max"], "x.base.max.ppm")
        # a line that carries tiers_max for a tier it does not measure is ignored there
        stray = {"event": "result", "id": "y", "variant": "base", "status": "ok", "tiers": {"720": tier(4.0)},
                 "tiers_max": {"1080": tier(5.0), "720": "broken"}}
        self.assertEqual(common.merge_results([stray])[("y", "base")]["tiers_max"], {})

    def test_loud_picture_is_noted_but_never_decides_the_outcome(self):
        self.assertEqual(report.loud_flags({"mean_luma": 0.6}), [])
        self.assertEqual(report.loud_flags({"mean_luma": 0.9}), ["white_when_loud"])
        self.assertEqual(report.loud_flags({"mean_luma": 0.0}), ["black_when_loud"])
        self.assertEqual(report.loud_flags({"mean_luma": None}), [])
        self.assertEqual(report.loud_flags(None), [])
        white = self.assess([two_pattern_event("x", pixels_max={"mean_luma": 0.95, "spatial_std": 0.0, "temporal_diff": 0.0})])
        self.assertEqual((white["outcome"], white["chosen"]["pixel_flags"]), ("qualified", []))
        self.assertEqual(white["chosen"]["loud_flags"], ["white_when_loud"])
        self.assertIn("white_when_loud", report.shader_row(dict(white, catalog={"id": "x", "name": "X"}))[-1])

    def test_react_variant_is_assessed_beside_base_and_can_be_preferred(self):
        events = [two_pattern_event("x", "base", (4.0, 9.0), (5.0, 10.0)), two_pattern_event("x", "react", (4.0, 9.0), (5.0, 10.0))]
        self.assertEqual(self.assess(events)["variant"], "base")            # tie: least modified
        better = [two_pattern_event("x", "base", (4.0, 12.0), (5.0, 14.0)), two_pattern_event("x", "react", (4.0, 9.0), (5.0, 10.0))]
        item = self.assess(better)
        self.assertEqual((item["variant"], item["render_height"]), ("react", 1080))
        saved = report.VARIANT_PREFERENCE
        try:
            report.VARIANT_PREFERENCE = ("react_lite", "react", "base")
            self.assertEqual(self.assess(events)["variant"], "react")
        finally:
            report.VARIANT_PREFERENCE = saved


class TwoPatternReport(Workspace):
    def run_report(self, events, *extra):
        results = self.write_results("results.jsonl", events)
        catalog_path = self.root / "catalog.json"
        common.write_json(catalog_path, self.catalog)
        out = self.root / "out"
        saved = report.VARIANT_PREFERENCE
        try:
            with contextlib.redirect_stdout(io.StringIO()):
                code = report.main([str(results), "--catalog", str(catalog_path), "--variants-dir", str(self.variants),
                                    "--doc", str(out / "doc.md"), "--qualified", str(out / "qualified.json"), "--no-show", *extra])
        finally:
            report.VARIANT_PREFERENCE = saved
        self.assertEqual(code, 0)
        return json.loads((out / "qualified.json").read_text(encoding="utf-8")), (out / "doc.md").read_text(encoding="utf-8")

    def events(self):
        return [ok_event("plasma", p95_720=2.0, p95_1080=4.5),                                   # old line, one pattern
                two_pattern_event("aaaaaa", "base", (5.0, 11.0), (6.0, 14.0)),                   # loud costs it 1080p
                two_pattern_event("dddddd", "base", (4.0, 9.0), (5.0, 10.0)),
                two_pattern_event("dddddd", "react", (3.0, 8.0), (4.0, 9.0))]

    def test_document_and_qualified_json_carry_both_patterns(self):
        qualified, doc = self.run_report(self.events())
        by_id = {q["id"]: q for q in qualified["qualified"]}
        self.assertEqual({i: (q["variant"], q["render_height"], q["audio"]) for i, q in by_id.items()},
                         {"plasma": ("base", 1080, ["beat128"]), "aaaaaa": ("base", 720, ["beat128", "max"]),
                          "dddddd": ("base", 1080, ["beat128", "max"])})
        self.assertEqual(by_id["aaaaaa"]["p95_ms_max"], {"720": 6.0, "1080": 14.0})
        self.assertEqual(by_id["aaaaaa"]["p95_ms"], {"720": 5.0, "1080": 11.0})          # unchanged meaning
        self.assertEqual(by_id["plasma"]["p95_ms_max"], {})
        self.assertEqual(qualified["qualified_on_one_pattern"], ["plasma"])
        self.assertFalse(qualified["require_worst_case"])
        self.assertIn("9.9 / 11.0 (loud 14.0) | MARGINAL", doc)
        self.assertIn("Qualified on both audio patterns: 2. Qualified on the 128 BPM pattern alone", doc)
        self.assertIn("128 BPM pattern only", doc)

    def test_require_worst_case_and_variant_selection(self):
        qualified, doc = self.run_report(self.events(), "--require-worst-case")
        self.assertEqual(sorted(q["id"] for q in qualified["qualified"]), ["aaaaaa", "dddddd"])
        self.assertIn("plasma", qualified["not_measured"])
        self.assertIn("ONE_PATTERN", doc)
        qualified, _ = self.run_report(self.events(), "--variants", "react,react_lite")
        self.assertEqual([(q["id"], q["variant"]) for q in qualified["qualified"]], [("dddddd", "react")])
        qualified, _ = self.run_report(self.events(), "--prefer", "react,base")
        self.assertEqual({q["id"]: q["variant"] for q in qualified["qualified"]}["dddddd"], "react")


if __name__ == "__main__":
    unittest.main()

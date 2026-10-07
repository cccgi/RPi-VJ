import importlib.util
import unittest


SPEC = importlib.util.spec_from_file_location(
    "import_shadertoy", "engine/tools/import_shadertoy.py"
)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class ShadertoyImportTests(unittest.TestCase):
    def test_wraps_image_pass_with_audio_adapter(self):
        source = "void mainImage(out vec4 c, in vec2 p) { c = vec4(iResolution.xy / iResolution.x, iTime, 1.0); }"
        converted = MODULE.convert(source, "test")
        self.assertIn("mainImage(rp_color, gl_FragCoord.xy)", converted)
        self.assertIn("u_beat_onset", converted)
        self.assertIn("#define rp_iTime u_time", converted)

    def test_rejects_feedback_and_channels(self):
        with self.assertRaises(ValueError):
            MODULE.image_pass({"renderpass": [{"type": "buffer", "code": ""}]})
        with self.assertRaises(ValueError):
            MODULE.image_pass({"renderpass": [{"type": "image", "code": "void mainImage()", "inputs": [{"channel": 0}]}]})


if __name__ == "__main__":
    unittest.main()

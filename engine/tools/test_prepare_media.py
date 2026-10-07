import unittest
from unittest.mock import patch
import prepare_media


class PreparationTests(unittest.TestCase):
    def test_encoder_keeps_sources_and_paths_literal(self):
        source = 'original clips/a;$(do-not-execute).mp4'
        args = prepare_media.encode_command(source, 'new clips/output.mp4')
        self.assertEqual(args[args.index('-i') + 1], source)
        self.assertIn('-n', args)
        self.assertNotIn('-y', args)
        self.assertEqual(args[args.index('-c:v') + 1], 'libx265')
        self.assertIn('open-gop=0', args[args.index('-x265-params') + 1])

    @patch('prepare_media.subprocess.run')
    def test_software_h264_asset_rejected_by_profile(self, run):
        run.return_value.stdout = '{"streams":[{"codec_name":"h264","width":1920,"height":1080,"pix_fmt":"yuv420p","avg_frame_rate":"60/1"}]}'
        with self.assertRaises(ValueError):
            prepare_media.verify('bad.mp4')
        self.assertEqual(run.call_count, 1)

    @patch('prepare_media.subprocess.run')
    def test_good_metadata_still_requires_full_decode(self, run):
        run.return_value.stdout = '{"streams":[{"codec_name":"hevc","width":1920,"height":1080,"pix_fmt":"yuv420p","avg_frame_rate":"60/1"}]}'
        prepare_media.verify('good.mp4')
        self.assertEqual(run.call_count, 2)
        self.assertIn('-xerror', run.call_args.args[0])


if __name__ == '__main__':
    unittest.main()

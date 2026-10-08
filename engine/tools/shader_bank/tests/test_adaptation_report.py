import copy
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from adaptation_report import candidate_tier


class CandidateGate(unittest.TestCase):
    def setUp(self):
        self.r = {'status': 'ok', 'variant': 'pi75', 'compile_ms': 12.0,
                  'tiers': {'720': {'frames': 90, 'p95_ms': 12.0, 'first_ms': 15.0},
                            '1080': {'frames': 90, 'p95_ms': 20.0, 'first_ms': 22.0}},
                  'pixels': {'mean_luma': 0.4, 'spatial_std': 0.2, 'temporal_diff': 0.1}}
        self.r['tiers_max'] = copy.deepcopy(self.r['tiers'])
        self.r['pixels_max'] = copy.deepcopy(self.r['pixels'])

    def test_requires_both_audio_patterns(self):
        self.assertEqual(candidate_tier(self.r), 720)
        del self.r['tiers_max']
        self.assertIsNone(candidate_tier(self.r))

    def test_loud_black_cannot_enter_candidate(self):
        self.r['pixels_max'] = {'mean_luma': 0, 'spatial_std': 0, 'temporal_diff': 0}
        self.assertIsNone(candidate_tier(self.r))

    def test_first_frame_and_compile_limits(self):
        self.r['tiers_max']['1080']['first_ms'] = 1501
        self.assertIsNone(candidate_tier(self.r))
        self.r['tiers_max']['1080']['first_ms'] = 22
        self.r['compile_ms'] = 1501
        self.assertIsNone(candidate_tier(self.r))

    def test_worst_case_over_budget(self):
        self.r['tiers_max']['720']['p95_ms'] = 13.1
        self.assertIsNone(candidate_tier(self.r))

    def test_missing_startup_evidence(self):
        del self.r['tiers_max']['720']['first_ms']
        self.assertIsNone(candidate_tier(self.r))


if __name__ == '__main__':
    unittest.main()

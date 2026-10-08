import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import unittest
from common import strip_comments
from pi_loop_rules import rule_pi75, rule_pi50, rule_pi28


class PiLoopRules(unittest.TestCase):
    def run_rule(self, source, rule=rule_pi75):
        return rule(source, strip_comments(source))

    def test_changes_one_dominant_loop_and_preserves_normalization(self):
        source = '#define N 100\nvoid mainImage(){for(int i=0;i<N;i++){x+=u_bass;} for(int j=0;j<8;j++)x++;x/=float(N);}'
        changed, edits = self.run_rule(source)
        self.assertIn('i<75', changed)
        self.assertIn('j<8', changed)
        self.assertIn('#define N 100', changed)
        self.assertIn('x/=float(N)', changed)
        self.assertEqual(len(edits), 1)

    def test_reviewed_settings_preserve_wrapper_and_reject_source_drift(self):
        from pi_tuned_rules import rule_pi_tuned
        source = 'float maxIter = 100.0;'
        changed, edits = rule_pi_tuned(source, source, {'id': 'l3GGRd'})
        self.assertEqual(changed, 'float maxIter = 64.0;')
        self.assertEqual(len(edits), 1)
        with self.assertRaises(ValueError):
            rule_pi_tuned('float maxIter = 120.0;', 'float maxIter = 120.0;', {'id': 'l3GGRd'})
        self.assertEqual(rule_pi_tuned(source, source, {'id': 'other'}), (source, []))

    def test_noise_rule_only_changes_octaves_in_fbm_function(self):
        from pi_loop_rules import rule_pi_fbm2
        source = 'float fbm(vec2 p){for(int i=0;i<5;i++) v+=noise(p);return v;} void mainImage(){for(int i=0;i<9;i++)x++;}'
        changed, edits = rule_pi_fbm2(source, source)
        self.assertIn('i<2', changed)
        self.assertIn('i<9', changed)
        self.assertEqual(len(edits), 1)

    def test_does_not_use_larger_constant_from_another_scope(self):
        source = 'void a(){const int N=10;for(int i=0;i<N;i++)x++;} void b(){const int N=100;for(int j=0;j<N;j++)y++;}'
        self.assertEqual(self.run_rule(source), (source, []))

    def test_absolute_cap_does_not_raise_small_loop_bounds(self):
        changed, edits = self.run_rule('for(int i=0;i<100;i++) x++;', rule_pi28)
        self.assertIn('i<28', changed)
        source = 'for(int i=0;i<20;i++) x++;'
        self.assertEqual(self.run_rule(source, rule_pi28), (source, []))

    def test_nonzero_start_and_float_typing(self):
        changed, edits = self.run_rule('for(float i=1.; i<17.; i+=1.) x+=u_mid;', rule_pi50)
        self.assertIn('i<9.0', changed)
        self.assertEqual(edits[0]['iterations_after'], 8)

    def test_rejects_ambiguous_and_nonunit_loops(self):
        for header in ['int i=0;i<64;i+=2', 'int i=0;i<u_bass;i++',
                       'int i=0;i++<64;', 'int i=0;i<64 && x<1.;i++',
                       'int i=64;i>0;i--', 'int i=0;i<7;i++']:
            source = 'for('+header+') x++;'
            self.assertEqual(self.run_rule(source), (source, []))

    def test_wrapper_and_comments_are_unchanged(self):
        body = 'uniform vec3 iChannelResolution[4];\n// for(int i=0;i<500;i++) x++;\nvoid mainImage(){for(int i=0;i<64;i++) x+=u_volume;}'
        wrapper = '\nvoid main(){\n    vec2 center = 0.5 * u_resolution;for(int i=0;i<1000;i++) x++;}'
        changed, edits = self.run_rule(body+wrapper)
        self.assertTrue(changed.endswith(wrapper))
        self.assertIn('i<500', changed)
        self.assertIn('i<48', changed)
        self.assertEqual(len(edits), 1)


if __name__ == '__main__':
    unittest.main()

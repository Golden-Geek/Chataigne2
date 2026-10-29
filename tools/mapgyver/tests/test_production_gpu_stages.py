import copy
from pathlib import Path
import runpy
import unittest

ANALYZE = runpy.run_path(str(Path(__file__).resolve().parents[1]/'production_gpu_stages.py'))['analyze_renderer_stages']


def token(generation=1, serial=1):
    return {'device': {'runtime': 'owned-runtime', 'generation': generation}, 'serial': serial}


def gpu(**changes):
    return {'token': token(), 'context': 'Renderer', 'status': 'Valid', 'begin': 100, 'end': 200,
            'frequency': 1000, 'renderer_inputs_end': 120, 'renderer_render_end': 170, **changes}


def analyze(rows, frames=None):
    return ANALYZE(frames if frames is not None else [{'token': token()}], rows, lambda v: v or None)


class RendererGpuStageTests(unittest.TestCase):
    def test_partitions_exact_whole_span_and_reports_largest_interval(self):
        report = analyze([gpu()])
        self.assertTrue(report['coverage_complete'])
        self.assertEqual(report['stages'], {'inputs': [20.0], 'render': [50.0], 'outputs': [30.0]})
        self.assertEqual(report['largest_stage_counts'], {'render': 1})
        self.assertEqual(sum(report['records'][0]['stage_ms'].values()), 100.0)

    def test_missing_invalid_and_partial_rows_keep_the_whole_denominator(self):
        for rows, reason in [([], 'GPU row missing'), ([gpu(status='Uncollected')], 'whole GPU span unavailable'),
                             ([gpu(renderer_inputs_end=None)], 'renderer stage markers unavailable')]:
            with self.subTest(reason=reason):
                report = analyze(rows)
                self.assertFalse(report['coverage_complete'])
                self.assertEqual(report['expected_batches'], 1)
                self.assertEqual(report['coverage'], {reason: 1})
                self.assertEqual(report['covered_batches'], 0)
                self.assertTrue(all(v is None for v in report['stages'].values()))

    def test_foreign_device_and_other_context_cannot_supply_markers(self):
        report = analyze([gpu(token=token(generation=2)), gpu(context={'Source': {'id': 1}})])
        self.assertEqual(report['coverage'], {'GPU row missing': 1})

    def test_unordered_outside_or_invalid_clocks_are_rejected(self):
        for changes in [dict(renderer_inputs_end=99), dict(renderer_render_end=119),
                        dict(renderer_render_end=201), dict(end=99), dict(frequency=0),
                        dict(renderer_inputs_end=True), dict(renderer_render_end=1 << 64)]:
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                analyze([gpu(**changes)])

    def test_duplicate_rows_or_frame_denominators_are_rejected(self):
        with self.assertRaises(ValueError):
            analyze([gpu(), copy.deepcopy(gpu())])
        with self.assertRaises(ValueError):
            analyze([gpu()], [{'token': token()}, {'token': token()}])

    def test_zero_intervals_are_observed_zero_and_ties_are_not_arbitrary(self):
        report = analyze([gpu(renderer_inputs_end=100, renderer_render_end=100, end=100)])
        self.assertTrue(report['coverage_complete'])
        self.assertEqual(report['largest_stage_counts'], {'tied': 1})
        self.assertEqual(report['stages']['inputs'], [0.0])

    def test_empty_cohort_cannot_qualify_coverage(self):
        report = analyze([], [])
        self.assertFalse(report['coverage_complete'])
        self.assertEqual(report['expected_batches'], 0)


if __name__ == '__main__':
    unittest.main()

import copy
from pathlib import Path
import runpy
import unittest

METRICS = runpy.run_path(str(Path(__file__).parents[1] / "import_metrics.py"))
validate_imports = METRICS["validate_imports"]


def distribution(samples):
    if samples == 0:
        return None
    return dict(samples=samples, mean_ms=0.2, median_ms=0.15,
                p95_ms=0.3, p99_ms=0.4, maximum_ms=0.5)


def report():
    counts = dict(polls=12, metadata_busy=1, no_publication=1, same_surface=2,
                  cache_hits=5, cache_misses=3, opens=3, evictions=0, pruned=0,
                  cleared=3, acquire_timeouts=2, reader_changes=6, reader_clears=2)
    sampled = dict(polls=6, metadata_busy=1, no_publication=0, same_surface=1,
                   cache_hits=4, cache_misses=0, opens=0, evictions=0, pruned=0,
                   cleared=0, acquire_timeouts=1, reader_changes=3, reader_clears=0)
    all_samples = [1, 12, 8, 8, 3, 3, 3, 8, 6, 2]
    sampled_samples = [0, 6, 4, 4, 0, 0, 0, 4, 3, 0]
    return dict(scope="renderer-local aliases", capacity=3, peak_cached=3,
                cached_after_clear=0, live_imports_after_clear=0, peak_live_imports=4,
                all=counts, sampled=sampled, stages=[
                    dict(stage=name, all=distribution(total), sampled=distribution(sample),
                         truncated_all=0, truncated_sampled=0)
                    for name, total, sample in zip(METRICS["STAGES"], all_samples, sampled_samples)])


class ImportMetricsTests(unittest.TestCase):
    def test_accepts_successful_reports_and_preserves_input(self):
        reports = [report(), report()]
        before = copy.deepcopy(reports)
        self.assertIs(validate_imports(reports), reports)
        self.assertEqual(reports, before)
        self.assertEqual(validate_imports([], expected_sources=0), [])

    def test_requires_exact_source_count(self):
        for value in (None, {}, [], [report()], [report(), report(), report()]):
            with self.subTest(value=value), self.assertRaises(ValueError):
                validate_imports(value)
        for value in (True, 2.0, -1):
            with self.subTest(expected=value), self.assertRaises(ValueError):
                validate_imports([report(), report()], value)

    def test_counter_fields_are_exact_nonnegative_integers(self):
        for name in METRICS["COUNTERS"]:
            for value in (True, -1, 1.0, None):
                broken = report()
                broken["all"][name] = value
                with self.subTest(name=name, value=value), self.assertRaises(ValueError):
                    validate_imports([broken], 1)
        for mutation in (lambda c: c.pop("polls"), lambda c: c.update(unknown=0)):
            broken = report()
            mutation(broken["all"])
            with self.assertRaises(ValueError):
                validate_imports([broken], 1)

    def test_rejects_broken_poll_acquisition_and_open_partitions(self):
        for window, counter in (("all", "polls"), ("sampled", "polls"),
                                ("all", "reader_changes"), ("sampled", "acquire_timeouts"),
                                ("all", "opens"), ("sampled", "opens")):
            broken = report()
            broken[window][counter] += 1
            with self.subTest(window=window, counter=counter), self.assertRaises(ValueError):
                validate_imports([broken], 1)

    def test_rejects_sampled_counts_above_all_even_when_partitions_reconcile(self):
        broken = report()
        broken["sampled"]["reader_clears"] = 3
        with self.assertRaisesRegex(ValueError, "sampled count exceeds"):
            validate_imports([broken], 1)

    def test_requires_complete_cleanup_and_bounded_aliases(self):
        for name, value in (("capacity", 4), ("peak_cached", 4),
                            ("cached_after_clear", 1), ("live_imports_after_clear", 1),
                            ("peak_live_imports", 5), ("capacity", True)):
            broken = report()
            broken[name] = value
            with self.subTest(name=name, value=value), self.assertRaises(ValueError):
                validate_imports([broken], 1)
        broken = report()
        broken["all"]["cleared"] = 2
        with self.assertRaisesRegex(ValueError, "removal"):
            validate_imports([broken], 1)

    def test_accepts_pruning_and_eviction_without_changing_removal_total(self):
        valid = report()
        valid["all"].update(pruned=1, evictions=1, cleared=1)
        validate_imports([valid], 1)

    def test_allows_importing_a_new_surface_inside_the_sample_window(self):
        valid = report()
        valid["sampled"].update(cache_hits=3, cache_misses=1, opens=1)
        for index in (4, 5, 6):
            valid["stages"][index]["sampled"] = distribution(1)
        validate_imports([valid], 1)

    def test_requires_exact_unique_stage_set_and_no_truncation(self):
        for mutation in (lambda s: s.pop(), lambda s: s.append(copy.deepcopy(s[0])),
                         lambda s: s[0].update(stage="metadata_lock"),
                         lambda s: s[1].update(stage=s[0]["stage"]),
                         lambda s: s[0].update(truncated_all=1),
                         lambda s: s[0].update(truncated_sampled=True)):
            broken = report()
            mutation(broken["stages"])
            with self.assertRaises(ValueError):
                validate_imports([broken], 1)

    def test_checks_stage_counts_against_counter_scopes(self):
        for index in range(len(METRICS["STAGES"])):
            broken = report()
            broken["stages"][index]["all"]["samples"] += 1
            with self.subTest(stage=index), self.assertRaises(ValueError):
                validate_imports([broken], 1)
        broken = report()
        broken["stages"][9]["sampled"] = distribution(1)
        with self.assertRaisesRegex(ValueError, "sample count"):
            validate_imports([broken], 1)

    def test_zero_operations_require_null_and_positive_operations_require_samples(self):
        for index, value in ((0, distribution(0)), (4, distribution(1))):
            broken = report()
            broken["stages"][index]["all" if index == 0 else "sampled"] = value
            with self.assertRaises(ValueError):
                validate_imports([broken], 1)
        broken = report()
        broken["stages"][0]["sampled"] = dict(distribution(1), samples=0)
        with self.assertRaisesRegex(ValueError, "null distribution"):
            validate_imports([broken], 1)

    def test_sampled_reader_release_can_cross_the_window_boundary(self):
        valid = report()
        valid["sampled"].update(no_publication=1, same_surface=0, reader_clears=1)
        valid["stages"][9]["sampled"] = distribution(1)
        valid["stages"][8]["sampled"] = distribution(4)
        validate_imports([valid], 1)
        valid["stages"][8]["sampled"] = distribution(7)
        with self.assertRaises(ValueError):
            validate_imports([valid], 1)

    def test_sampled_release_count_conserves_at_most_one_boundary_reader(self):
        for samples in (2, 3, 4):
            valid = report()
            valid["stages"][8]["sampled"] = distribution(samples)
            with self.subTest(accepted_samples=samples):
                validate_imports([valid], 1)
        for value in (None, dict(distribution(1), samples=0), distribution(1), distribution(5)):
            broken = report()
            broken["stages"][8]["sampled"] = value
            with self.subTest(rejected_distribution=value), self.assertRaises(ValueError):
                validate_imports([broken], 1)

    def test_accepts_native_floating_point_timing_values(self):
        valid = report()
        valid["stages"][0]["all"].update(
            mean_ms=0.00012224999999999998, median_ms=0.00010000000000000002,
            p95_ms=0.0002, p99_ms=0.00020000000000000004, maximum_ms=0.0003)
        validate_imports([valid], 1)

    def test_rejects_nonfinite_negative_boolean_or_misordered_timings(self):
        for name, value in (("mean_ms", float("nan")), ("maximum_ms", float("inf")),
                            ("median_ms", -0.1), ("p95_ms", True), ("samples", 1.0),
                            ("p95_ms", 0.6), ("p99_ms", 0.6), ("mean_ms", 0.6)):
            broken = report()
            broken["stages"][0]["all"][name] = value
            with self.subTest(name=name, value=value), self.assertRaises(ValueError):
                validate_imports([broken], 1)


if __name__ == "__main__":
    unittest.main()

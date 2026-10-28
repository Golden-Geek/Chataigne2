import copy
import csv
import io
from pathlib import Path
import runpy
import unittest

ROLES = runpy.run_path(str(Path(__file__).parents[1] / "presentmon_roles.py"))
HEADERS = ["Application", "ProcessID", "SwapChainAddress", "QPCTime", "Dropped",
           "MsUntilDisplayed", "MsBetweenDisplayChange", "MsInPresentAPI"]
CONFIG = dict(start_qpc=1000, frequency=1000, warmup_seconds=1, maximum_seconds=2)


def text(rows, headers=HEADERS):
    output = io.StringIO(newline="")
    writer = csv.writer(output)
    writer.writerow(headers)
    writer.writerows(rows)
    return output.getvalue()


def row(qpc, chain="0xAA", pid=7, until="10"):
    return ["probe.exe", str(pid), chain, str(qpc), "0", until, "20", "0.02"]


def chain(role, address, begin=1000, end=4000):
    return dict(role=role, begin_qpc=begin, end_qpc=end,
                addresses=dict(idxgi_swap_chain=address, idxgi_swap_chain1=address,
                               idxgi_swap_chain2=address))


def report():
    return dict(render_process_id=7, generations=[dict(generation=0, swapchains=[
        chain("editor", "0xaa"), chain("native_output", "0xbb")])])


def correlate(rows, native=None, config=None):
    return ROLES["correlate"](text(rows), 7, report() if native is None else native,
                              CONFIG if config is None else config)


class PresentMonRoleTests(unittest.TestCase):
    def test_exact_aliases_assign_roles_without_address_or_row_order_inference(self):
        native = report()
        native["generations"][0]["swapchains"].reverse()
        result = correlate([row(1980), row(2000), row(2020), row(2001, "0xBB"),
                            row(2021, "0xBB")], native)
        output, editor = result["roles"]
        self.assertEqual([output["role"], editor["role"]], ["native_output", "editor"])
        self.assertEqual([output["raw_rows"], editor["raw_rows"]], [2, 3])
        self.assertEqual(editor["chains"][0]["display_window"]["intervals_using_before_window_predecessor"], 1)
        self.assertEqual(editor["chains"][0]["matched_interface_keys"],
                         ["idxgi_swap_chain", "idxgi_swap_chain1", "idxgi_swap_chain2"])
        self.assertTrue(result["all_rows_correlated"])
        self.assertTrue(result["all_native_records_observed"])
        self.assertEqual(result["raw_rows"], 5)
        self.assertNotIn("scanout_verified", result)

    def test_different_interface_aliases_remain_separate_observed_chains(self):
        native = report()
        native["generations"][0]["swapchains"][0]["addresses"]["idxgi_swap_chain2"] = "0x00AC"
        result = correlate([row(2000), row(2020, "0X0000ac"), row(2001, "0xbb")], native)
        editor = result["roles"][0]
        self.assertEqual([c["swapchain_address"] for c in editor["chains"]], ["0xaa", "0xac"])
        self.assertEqual(editor["chains"][1]["matched_interface_keys"], ["idxgi_swap_chain2"])
        self.assertEqual(editor["raw_rows"], 2)

    def test_disjoint_generation_address_reuse_has_no_cross_generation_display_predecessor(self):
        native = report()
        for item in native["generations"][0]["swapchains"]:
            item["end_qpc"] = 2100
        native["generations"].append(dict(generation=1, swapchains=[
            chain("editor", "0xbb", 2100, 4000), chain("native_output", "0xaa", 2100, 4000)]))
        result = correlate([row(2099), row(2099, "0xbb"), row(2100), row(2100, "0xbb")], native)
        self.assertEqual([(r["generation"], r["role"]) for r in result["roles"]],
                         [(0, "editor"), (0, "native_output"), (1, "editor"), (1, "native_output")])
        for role in result["roles"]:
            self.assertEqual(role["raw_rows"], 1)
            self.assertEqual(role["chains"][0]["display_window"]["missing_predecessor"], 1)

    def test_qpc_values_above_float_precision_preserve_exact_half_open_boundaries(self):
        base = (1 << 63) + 11
        native = report()
        for item in native["generations"][0]["swapchains"]:
            item.update(begin_qpc=base, end_qpc=base + 2)
        config = dict(start_qpc=base, frequency=1, warmup_seconds=0, maximum_seconds=2)
        result = correlate([row(base), row(base + 1, "0xbb")], native, config)
        self.assertEqual(result["roles"][1]["chains"][0]["first_present_qpc"], base + 1)
        with self.assertRaisesRegex(ValueError, "outside its native lifetime"):
            correlate([row(base + 2), row(base + 1, "0xbb")], native, config)

    def test_display_tail_after_native_end_keeps_valid_present_identity(self):
        native = report()
        for item in native["generations"][0]["swapchains"]:
            item["end_qpc"] = 2100
        result = correlate([row(2099, until="50"), row(2099, "0xbb", until="20")], native)
        self.assertEqual(result["raw_rows"], 2)
        for role, display_qpc in zip(result["roles"], ("2149", "2119")):
            summary = role["chains"][0]
            self.assertEqual(summary["last_present_qpc"], 2099)
            self.assertEqual(summary["present_window"]["presents"], 1)
            self.assertEqual(summary["display_window"]["events"], 1)
            self.assertEqual(summary["display_window"]["first_display_qpc_exact"], display_qpc)
            self.assertGreater(int(display_qpc), role["end_qpc"])

    def test_unknown_addresses_missing_roles_and_missing_observations_are_rejected(self):
        for rows in ([row(2000, "0xcc"), row(2000, "0xbb")], [row(2000)]):
            with self.subTest(rows=rows), self.assertRaises(ValueError):
                correlate(rows)
        missing = report()
        missing["generations"][0]["swapchains"].pop()
        with self.assertRaises(ValueError):
            correlate([row(2000)], missing)
        for native in ({}, dict(render_process_id=7), dict(render_process_id=7, generations=[])):
            with self.subTest(native=native), self.assertRaises(ValueError):
                correlate([row(2000)], native)

    def test_rows_before_after_or_between_native_lifetimes_are_rejected(self):
        for qpc in (999, 4000, 5000):
            with self.subTest(qpc=qpc), self.assertRaisesRegex(ValueError, "outside its native lifetime"):
                correlate([row(qpc), row(2000, "0xbb")])
        native = report()
        native["generations"][0]["swapchains"][0]["end_qpc"] = 2000
        native["generations"].append(dict(generation=1, swapchains=[
            chain("editor", "0xaa", 3000, 4000), chain("native_output", "0xcc", 3000, 4000)]))
        with self.assertRaisesRegex(ValueError, "outside its native lifetime"):
            correlate([row(1500), row(2500), row(3500), row(1500, "0xbb"), row(3500, "0xcc")], native)

    def test_alias_overlap_is_rejected_even_when_csv_does_not_exercise_shared_alias(self):
        native = report()
        native["generations"][0]["swapchains"][1]["addresses"]["idxgi_swap_chain1"] = "0xaa"
        with self.assertRaisesRegex(ValueError, "overlapping native swapchain aliases"):
            correlate([row(2000), row(2000, "0xbb")], native)
        native = report()
        native["generations"].append(dict(generation=1, swapchains=[
            chain("editor", "0xaa", 3999, 5000), chain("native_output", "0xcc", 3999, 5000)]))
        with self.assertRaisesRegex(ValueError, "overlapping native swapchain aliases"):
            correlate([row(2000), row(2000, "0xbb")], native)

    def test_duplicate_generations_roles_wrong_pid_and_malformed_metadata_are_rejected(self):
        mutations = [
            lambda n: n.update(render_process_id=8),
            lambda n: n.update(render_process_id=True),
            lambda n: n["generations"].append(copy.deepcopy(n["generations"][0])),
            lambda n: n["generations"][0].update(generation=True),
            lambda n: n["generations"][0].update(generation=-1),
            lambda n: n["generations"][0].update(generation=1 << 32),
            lambda n: n["generations"][0]["swapchains"][0].update(role="unknown"),
            lambda n: n["generations"][0]["swapchains"][0].update(role="native_output"),
            lambda n: n["generations"][0]["swapchains"][0].update(begin_qpc=1000.0),
            lambda n: n["generations"][0]["swapchains"][0].update(begin_qpc=0),
            lambda n: n["generations"][0]["swapchains"][0].update(end_qpc=1000),
            lambda n: n["generations"][0]["swapchains"][0].update(end_qpc=1 << 64),
            lambda n: n["generations"][0]["swapchains"][0]["addresses"].pop("idxgi_swap_chain1"),
            lambda n: n["generations"][0]["swapchains"][0]["addresses"].update(extra="0xaa"),
        ]
        for mutation in mutations:
            native = report()
            mutation(native)
            with self.subTest(native=native), self.assertRaises(ValueError):
                correlate([row(2000), row(2000, "0xbb")], native)
        for address in (0xaa, "0x0", "0x", "aa", "0x10000000000000000", "0x aa"):
            native = report()
            native["generations"][0]["swapchains"][0]["addresses"]["idxgi_swap_chain"] = address
            with self.subTest(address=address), self.assertRaises(ValueError):
                correlate([row(2000), row(2000, "0xbb")], native)

    def test_invalid_csv_and_clock_config_still_use_strict_unlabeled_parser(self):
        native = report()
        for source in (text([row(2000, pid=8)]), text([row("2000.0")]), text([row(2000), row(2000)])):
            with self.subTest(source=source), self.assertRaises(ValueError):
                ROLES["correlate"](source, 7, native, CONFIG)
        with self.assertRaises(ValueError):
            ROLES["correlate"](text([row(2000)]), True, native, CONFIG)
        with self.assertRaises(ValueError):
            correlate([row(2000), row(2000, "0xbb")], config=dict(CONFIG, frequency=0))
        unlabeled = ROLES["DATA"]["analyze"](text([row(2000), row(2000, "0xbb")]), 7, CONFIG)
        self.assertTrue(all(chain["label"] == "unlabeled" for chain in unlabeled["chains"]))

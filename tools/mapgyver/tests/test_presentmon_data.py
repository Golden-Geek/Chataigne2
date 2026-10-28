import csv
import io
from pathlib import Path
import runpy
import unittest

DATA = runpy.run_path(str(Path(__file__).parents[1] / "presentmon_data.py"))
HEADERS = ["Application", "ProcessID", "SwapChainAddress", "QPCTime", "Dropped", "MsUntilDisplayed", "MsBetweenDisplayChange", "MsInPresentAPI"]
CONFIG = dict(start_qpc=1000, frequency=1000, warmup_seconds=1, maximum_seconds=2)


def text(rows, headers=HEADERS):
    output = io.StringIO(newline="")
    writer = csv.writer(output)
    writer.writerow(headers)
    writer.writerows(rows)
    return output.getvalue()


def row(qpc, dropped=0, until="10", interval="20", pid=7, chain="0xAB"):
    return ["probe.exe", str(pid), chain, str(qpc), str(dropped), until, interval, "0.02"]


class PresentMonDataTests(unittest.TestCase):
    def test_exact_window_distinguishes_presents_displays_tails_and_previous_display(self):
        source = text([row(1980), row(1995), row(2000), row(2020, 1, "0"),
                       row(2050, until="NA"), row(2080, until="0"), row(2995), row(3000)])
        result = DATA["analyze"](source, 7, CONFIG)
        chain = result["chains"][0]
        self.assertEqual(result["raw_rows"], 8)
        self.assertEqual(chain["label"], "unlabeled")
        self.assertEqual(chain["present_window"], dict(presents=5, dropped=1, non_dropped=4,
            unavailable_display_timing=2, usable_display_timestamps=2, display_tails_after_window=1))
        display = chain["display_window"]
        self.assertEqual(display["events"], 2)
        self.assertEqual(display["arrivals_from_before_present_window"], 1)
        self.assertEqual(display["intervals_using_before_window_predecessor"], 1)
        self.assertEqual(display["observed_display_intervals_ms"]["samples"], 2)
        self.assertEqual(display["observed_display_intervals_ms"]["maximum_ms"], 15)
        self.assertEqual(display["last_display_qpc_exact"], "2010")

    def test_large_integer_qpc_and_fractional_display_ticks_never_round_through_float(self):
        base = (1 << 63) + 11
        config = dict(start_qpc=base, frequency=3, warmup_seconds=0, maximum_seconds=2)
        result = DATA["analyze"](text([row(base, until="0.5"), row(base+1, until="0.5")]), 7, config)
        chain = result["chains"][0]
        self.assertEqual(chain["first_present_qpc"], base)
        self.assertEqual(chain["display_window"]["first_display_qpc_exact"], str(DATA["Fraction"](base) + DATA["Fraction"](3,2000)))
        self.assertAlmostEqual(chain["display_window"]["observed_display_intervals_ms"]["mean_ms"], 1000/3)

    def test_unknown_zero_and_dropped_timing_stays_unavailable_and_missing_not_zero(self):
        result = DATA["analyze"](text([row(2000, until=""), row(2020, until="NA"), row(2040, until="0"), row(2060,1,"0")]),7,CONFIG)
        chain = result["chains"][0]
        self.assertEqual(chain["present_window"]["unavailable_display_timing"],3)
        self.assertEqual(chain["display_window"]["events"],0)
        self.assertIsNone(chain["display_window"]["observed_display_intervals_ms"])
        result = DATA["analyze"](text([row(2000), row(2020,until="1")]),7,CONFIG)
        self.assertEqual(result["chains"][0]["display_window"]["missing_predecessor"],1)

    def test_case_insensitive_schema_preserves_columns_and_two_chains_have_no_roles(self):
        headers = [h.lower() for h in HEADERS] + ["UnknownVendorColumn"]
        source = text([row(2000,chain="0xFF")+["opaque, value"], row(2010,chain="0xaa")+["all original text"]],headers)
        result = DATA["analyze"](source,7,CONFIG)
        self.assertEqual(result["columns"],headers)
        self.assertEqual([c["swapchain_address"] for c in result["chains"]],["0xff","0xaa"])
        self.assertTrue(all(c["label"] == "unlabeled" for c in result["chains"]))
        self.assertNotIn("editor", [c["label"] for c in result["chains"]])

    def test_malformed_identity_qpc_headers_rows_and_nonfinite_metrics_are_rejected(self):
        bad_sources = [text([row(2000,pid=8)]), text([row("2000.0")]), text([row("2e3")]),
            text([row(2000),row(2000)]), text([row(2020),row(2000)]), text([row(2000,chain="0x0")]),
            text([row(2000,dropped=2)]), text([row(2000,until="NaN")]), text([row(2000,until="-1")]),
            text([row(2000,interval="Infinity")]), text([row(2000)+["extra"]]),
            text([row(2000)],HEADERS[:-1]+["ProcessID"]), text([row(2000)],HEADERS[:-1]+["QPCTIME"]),
            text([],HEADERS), text([row(2000)])+'\n', '"unterminated']
        for source in bad_sources:
            with self.subTest(source=source[:90]), self.assertRaises(ValueError): DATA["analyze"](source,7,CONFIG)
        invalid = dict(CONFIG, frequency=0)
        with self.assertRaises(ValueError): DATA["analyze"](text([row(2000)]),7,invalid)

    def test_reversed_display_timestamps_and_inconsistent_drop_timing_are_counted(self):
        result = DATA["analyze"](text([row(2000,until="100"),row(2020,until="1"),row(2040,1,"4")]),7,CONFIG)
        chain = result["chains"][0]
        self.assertEqual(chain["non_increasing_display_pairs_all_rows"],1)
        self.assertEqual(chain["dropped_with_positive_display_time_all_rows"],1)
        self.assertIsNone(chain["display_window"]["observed_display_intervals_ms"])

    def test_loss_warning_absence_never_becomes_verified_zero(self):
        observed = DATA["loss_observation"]("csv data", "")
        self.assertTrue(observed["no_loss_reported"])
        self.assertFalse(observed["zero_loss_independently_verified"])
        self.assertEqual(observed["reported_event_loss_counts"],[])
        observed = DATA["loss_observation"]("warning: 2 ETW buffers were lost.\n", "warning: 17 ETW events were lost.\n")
        self.assertFalse(observed["no_loss_reported"])
        self.assertEqual(observed["reported_event_loss_counts"],[17])
        self.assertEqual(observed["reported_buffer_loss_counts"],[2])
        self.assertEqual(observed["loss_warnings"][0]["text"],"warning: 2 ETW buffers were lost.")
        self.assertFalse(DATA["loss_observation"]("", "warning: ETW present queue overflow") ["no_loss_reported"])

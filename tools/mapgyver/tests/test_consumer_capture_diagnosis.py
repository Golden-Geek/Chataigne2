import copy
import csv
import io
from pathlib import Path
import runpy
import unittest


DIAGNOSIS = runpy.run_path(str(Path(__file__).parents[1] / "diagnose-consumer-capture.py"))
diagnose_csv = DIAGNOSIS["diagnose_csv"]
WINDOW = dict(start_qpc=100, frequency=10, warmup_seconds=1, maximum_seconds=3)


def chains():
    return [dict(role=role, begin_qpc=90, end_qpc=140,
                 addresses=dict(idxgi_swap_chain=address, idxgi_swap_chain1=address))
            for role, address in (("editor", "0xaa"), ("native_output", "0xbb"))]


def source(rows, headers=None):
    output = io.StringIO(newline="")
    writer = csv.writer(output)
    writer.writerow(headers or ["ProcessID", "SwapChainAddress", "QPCTime", "Dropped",
                                "msUntilDisplayed", "msBetweenDisplayChange"])
    writer.writerows(rows)
    return output.getvalue()


def row(address, qpc, dropped="0", pid="71"):
    return [pid, address, str(qpc), dropped, "1.25", "16.67"]


class ConsumerCaptureDiagnosisTests(unittest.TestCase):
    def test_cross_chain_interleave_preserves_all_rows_and_separate_count_windows(self):
        rows = [row("0X0000AA", 109), row("0xbb", 105, "1"), row("0xaa", 110),
                row("0xbb", 129, "1"), row("0xaa", 130)]
        text = source(rows)
        identities = chains()
        before = copy.deepcopy(identities)
        result = diagnose_csv(text, 71, identities, WINDOW)
        self.assertEqual(identities, before)
        self.assertEqual(result["raw_rows"], 5)
        self.assertTrue(result["original_order_preserved"])
        self.assertTrue(result["present_qpc_order_valid"])
        self.assertEqual(result["present_qpc_transitions"], [])
        self.assertEqual(result["duplicate_present_keys"], [])
        self.assertEqual(result["roles"]["editor"], dict(raw_rows=3, sampled_rows=1, dropped_all=0,
                                                        dropped_sampled=0, rows_before_sample=1,
                                                        rows_after_sample=1))
        self.assertEqual(result["roles"]["native_output"]["dropped_all"], 2)
        self.assertEqual(result["roles"]["native_output"]["dropped_sampled"], 1)
        self.assertIsNone(result["order_dependent_metrics"])

    def test_backward_equal_and_duplicate_rows_remain_counted_in_original_line_order(self):
        text = source([row("0xaa", 120), row("0xbb", 111), row("0xaa", 110, "1"),
                       row("0X000AA", 110), row("0xaa", 121)])
        result = diagnose_csv(text, 71, chains(), WINDOW)
        self.assertEqual(result["raw_rows"], 5)
        self.assertEqual(result["roles"]["editor"]["raw_rows"], 4)
        self.assertEqual(result["roles"]["editor"]["sampled_rows"], 4)
        self.assertEqual(result["roles"]["editor"]["dropped_sampled"], 1)
        self.assertFalse(result["present_qpc_order_valid"])
        self.assertEqual((result["backward_transitions"], result["equal_transitions"]), (1, 1))
        backward, equal = result["present_qpc_transitions"]
        self.assertEqual((backward["previous"]["csv_line"], backward["current"]["csv_line"]), (2, 4))
        self.assertEqual((backward["previous"]["qpc"], backward["current"]["qpc"]), (120, 110))
        self.assertEqual(backward["delta_qpc"], -10)
        self.assertEqual(backward["delta_ms"], -1000)
        self.assertEqual((equal["previous"]["csv_line"], equal["current"]["csv_line"]), (4, 5))
        self.assertEqual(equal["delta_qpc"], 0)
        duplicate = result["duplicate_present_keys"][0]
        self.assertEqual((duplicate["address"], duplicate["qpc"]), ("0xaa", 110))
        self.assertEqual([record["csv_line"] for record in duplicate["rows"]], [4, 5])
        self.assertEqual([record["data_index"] for record in duplicate["rows"]], [2, 3])

    def test_nonadjacent_duplicate_identity_is_reported_without_deduplication(self):
        result = diagnose_csv(source([row("0xaa", 110), row("0xaa", 120), row("0xaa", 110)]),
                              71, chains(), WINDOW)
        self.assertEqual(result["raw_rows"], 3)
        self.assertEqual(result["roles"]["editor"]["sampled_rows"], 3)
        self.assertEqual([record["csv_line"] for record in result["duplicate_present_keys"][0]["rows"]], [2, 4])

    def test_unknown_pid_address_and_outside_lifetime_rows_are_rejected(self):
        for fields in (row("0xaa", 110, pid="72"), row("0xcc", 110),
                       row("0xaa", 89), row("0xaa", 140), row("+0xaa", 110),
                       row("0x0", 110), row("0xaa", 110, dropped="2")):
            with self.subTest(fields=fields), self.assertRaises(ValueError):
                diagnose_csv(source([fields]), 71, chains(), WINDOW)
        for pid in (True, 71.0, 0, 1 << 32):
            with self.subTest(pid=pid), self.assertRaises(ValueError):
                diagnose_csv(source([row("0xaa", 110)]), pid, chains(), WINDOW)

    def test_ambiguous_native_role_address_lifetime_is_rejected(self):
        identities = chains()
        identities[1]["addresses"] = identities[0]["addresses"]
        with self.assertRaises(ValueError):
            diagnose_csv(source([row("0xaa", 110)]), 71, identities, WINDOW)

    def test_schema_rejects_missing_duplicate_headers_bad_width_and_nonfinite_cells(self):
        for text in ("", source([]), source([row("0xaa", 110)[:-1]]),
                     source([row("0xaa", 110)], headers=["ProcessID", "ProcessID", "QPCTime", "Dropped",
                                                         "msUntilDisplayed", "msBetweenDisplayChange"]),
                     source([["71", "0xaa", "110", "0", "nan", "16.67"]])):
            with self.subTest(text=text), self.assertRaises(ValueError):
                diagnose_csv(text, 71, chains(), WINDOW)


if __name__ == "__main__":
    unittest.main()

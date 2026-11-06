import copy
import importlib.util
import json
from pathlib import Path
import unittest
import zipfile

ROOT = Path(__file__).resolve().parents[5]
SPEC = importlib.util.spec_from_file_location(
    "desktop_latency", Path(__file__).resolve().parents[1] / "desktop_latency.py")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class DesktopLatencyReplay(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        archive = ROOT / "docs/progress/evidence/mapgyver-m05-latency-2026-11-06/native-latency.zip"
        with zipfile.ZipFile(archive) as source:
            cls.result = json.loads(source.read("fixed-screen-100/latency.json"))
            cls.witnesses = source.read("fixed-screen-100/latency-witnesses.bgra")

    def test_complete_observed_cohort_does_not_prove_latency_budget(self):
        result = MODULE.audit(self.result, self.witnesses)
        self.assertTrue(result["complete"])
        self.assertEqual(result["actions"], 30)
        self.assertFalse(result["within_50_ms"])

    def test_changed_clock_or_untrusted_input_is_rejected(self):
        result = copy.deepcopy(self.result)
        result["observations"][0]["dispatch_ns"] = str(
            int(result["observations"][0]["response_arrival_ns"]) + 1)
        with self.assertRaises(AssertionError):
            MODULE.audit(result, self.witnesses)
        result = copy.deepcopy(self.result)
        result["events"][0]["trusted"] = False
        with self.assertRaises(AssertionError):
            MODULE.audit(result, self.witnesses)

    def test_missing_pixels_or_forged_edge_metadata_is_rejected(self):
        with self.assertRaises(AssertionError):
            MODULE.audit(self.result, bytes(len(self.witnesses)))
        result = copy.deepcopy(self.result)
        result["observations"][0]["after"]["blue"] += 3
        with self.assertRaises(AssertionError):
            MODULE.audit(result, self.witnesses)


if __name__ == "__main__":
    unittest.main()

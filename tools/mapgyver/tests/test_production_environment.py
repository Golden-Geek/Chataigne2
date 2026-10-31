import copy
import json
from pathlib import Path
import runpy
import unittest
from unittest import mock

ENV = runpy.run_path(str(Path(__file__).resolve().parents[1] / "production_environment.py"))
HASH = "a" * 64
ADAPTER = {"low": 123, "high": 0}


def pair():
    gpu = {"id": ADAPTER, "vendor_id": 0x10de, "device_id": 123, "software": False,
           "outputs": {"Observed": [{"name": "DISPLAY2", "attached_to_desktop": True,
                                      "desktop_coordinates": [0, 0, 1920, 1080]}]}}
    build = {"executable_sha256": HASH, "source_set_sha256": HASH, "link_stdout_sha256": HASH,
             "build_stderr_sha256": HASH, "rustc": "rustc selected",
             "selected": {"linker_sha256": HASH, "msvc_version": "14.44", "sdk_libraries": [
                 {"kind": kind, "version": "10.0.26100.0", "anchor_sha256": HASH} for kind in ("um", "ucrt")]}}
    before = {"clock": {"begin_qpc": 100, "end_qpc": 200, "frequency": 1000,
                        "utc_begin": "2026-10-31T10:00:00+00:00"},
              "identity": {"build": build, "os": ["Windows"], "drivers": [
                  {"PNPDeviceID": "PCI\\VEN_10DE&DEV_007B", "DriverVersion": "1", "DriverDate": "2026-01-01"}],
                  "webview": ["1.0"], "rustc": ["selected"], "power_scheme": ["scheme"],
                  "hardware": {"adapters": [gpu], "displays": [{"bounds": {"extent": {
                      "width": 1920, "height": 1080}}}], "ac_line_status": 1,
                      "webview_default_runtime_version": "1.0"},
                  "overlay_state": "Off", "webview_selection_environment": {}},
              "overlay_observation": {"state": "Off", "control_text": "Enable and open overlay",
                  "method": "native UI", "observed_at_utc": "2026-10-31T09:59:59+00:00"}}
    after = copy.deepcopy(before)
    after["clock"] = {"begin_qpc": 1000, "end_qpc": 1100, "frequency": 1000,
                      "utc_begin": "2026-10-31T10:00:00.9+00:00"}
    after["overlay_observation"]["observed_at_utc"] = "2026-10-31T10:00:01+00:00"
    return before, after


def analyze(before, after, **overrides):
    args = dict(adapter=ADAPTER, monitor="DISPLAY2", executable_sha256=HASH,
                begin_qpc=300, end_qpc=900, frequency=1000, webview_runtime="1.0")
    args.update(overrides)
    return ENV["analyze"](before, after, **args)


class EnvironmentEvidenceTests(unittest.TestCase):
    def test_complete_paired_observations_pin_the_exact_route(self):
        result = analyze(*pair())
        self.assertIs(result["observed_within_target"], True)
        self.assertTrue(result["same_adapter_display"])

    def test_missing_metadata_never_becomes_a_pass(self):
        before, after = pair()
        self.assertIsNone(analyze(None, after)["observed_within_target"])
        for key in ("build", "drivers", "power_scheme", "hardware", "webview", "rustc"):
            with self.subTest(key=key):
                b, a = pair()
                del b["identity"][key]
                del a["identity"][key]
                self.assertIsNone(analyze(b, a)["observed_within_target"])

    def test_changed_environment_or_wrong_executable_or_nonbracketing_time_is_a_failure(self):
        before, after = pair()
        after["identity"]["drivers"][0]["DriverVersion"] = "2"
        self.assertIs(analyze(before, after)["observed_within_target"], False)
        self.assertIs(analyze(*pair(), executable_sha256="b" * 64)["observed_within_target"], False)
        self.assertIs(analyze(*pair(), begin_qpc=150)["observed_within_target"], False)

    def test_stale_or_reused_overlay_evidence_and_custom_webview_stay_unqualified(self):
        for mode in ("missing", "stale", "reused", "unproven", "webview"):
            with self.subTest(mode=mode):
                b, a = pair()
                if mode == "missing": a["overlay_observation"] = None
                if mode == "stale": a["overlay_observation"]["observed_at_utc"] = "2026-10-30T10:00:00+00:00"
                if mode == "reused": a["overlay_observation"] = copy.deepcopy(b["overlay_observation"])
                if mode == "unproven": a["overlay_observation"]["control_text"] = "unknown"
                if mode == "webview":
                    b["identity"]["webview_selection_environment"] = {"folder": "override"}
                    a["identity"] = copy.deepcopy(b["identity"])
                self.assertIsNone(analyze(b, a)["observed_within_target"])

    def test_nvidia_overlay_on_fails_and_cross_adapter_route_is_explicit(self):
        b, a = pair()
        for item in (b, a):
            item["identity"]["overlay_state"] = "On"
            item["overlay_observation"]["state"] = "On"
        self.assertIs(analyze(b, a)["observed_within_target"], False)
        b, a = pair()
        for item in (b, a):
            item["identity"]["hardware"]["adapters"][0]["outputs"] = {"Observed": []}
        self.assertIsNone(analyze(b, a)["observed_within_target"])
        for item in (b, a):
            item["identity"]["hardware"]["adapters"].append({
                "id": {"low": 124, "high": 0}, "vendor_id": 0x8086, "device_id": 1,
                "software": False, "outputs": {"Observed": [
                    {"name": "DISPLAY2", "attached_to_desktop": True,
                     "desktop_coordinates": [0, 0, 1920, 1080]}]}})
        result = analyze(b, a)
        self.assertIs(result["observed_within_target"], True)
        self.assertFalse(result["same_adapter_display"])

    def test_nested_powershell_stringification_never_qualifies_hardware_metadata(self):
        b, a = pair()
        for item in (b, a):
            item["identity"]["hardware"]["adapters"][0]["outputs"]["Observed"][0]["desktop_coordinates"] = "0 0 1920 1080"
        self.assertIsNone(analyze(b, a)["observed_within_target"])

    def test_selected_linker_and_sdk_come_from_actual_printed_arguments(self):
        tokens = [r"C:\Program Files\VS\VC\Tools\MSVC\14.44.35207\bin\Hostx64\x64\link.exe",
                  r"/LIBPATH:C:\Program Files (x86)\Windows Kits\10\Lib\10.0.26100.0\um\x64",
                  r"/LIBPATH:C:\Program Files (x86)\Windows Kits\10\Lib\10.0.26100.0\ucrt\x64"]
        text = " ".join(json.dumps(token) for token in tokens)
        with mock.patch.object(Path, "is_file", return_value=True), \
                mock.patch.dict(ENV["link_identity"].__globals__, {"digest": lambda _: HASH}):
            selected = ENV["link_identity"](text)
            self.assertEqual(selected["msvc_version"], "14.44.35207")
            self.assertEqual({row["kind"] for row in selected["sdk_libraries"]}, {"um", "ucrt"})
            with self.assertRaises(ValueError):
                ENV["link_identity"](json.dumps(tokens[0]))


if __name__ == "__main__":
    unittest.main()

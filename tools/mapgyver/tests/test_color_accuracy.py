import copy
import contextlib
import io
import json
from pathlib import Path
import runpy
import subprocess
import unittest
from unittest import mock

RUNNER = runpy.run_path(str(Path(__file__).parents[1] / "run-color-accuracy.py"))


def report(scenario="q01"):
    extent = [3840, 2160] if scenario == "q02" else [1920, 1080]
    pixels = extent[0] * extent[1]
    devices = []
    for role in RUNNER["ROLES"]:
        consumer = role == "consumer"
        devices.append(dict(role=role, retirement_ms=1.2,
            after_fence=dict(live_bytes_by_owner=[0] * 4, retiring_bytes_by_owner=[0] * 4,
                             live_textures=0, retiring_textures=0),
            operations=dict(uniform_upload_bytes=0, diagnostic_readback_pixels=pixels if consumer else 0,
                            explicit_gpu_copy_calls=int(consumer), explicit_gpu_copy_bytes=pixels * 4 if consumer else 0,
                            shader_draws=2 if role == "renderer" else 0, clears=int(role == "renderer"),
                            sdk_receive_calls=0,
                            sdk_send_calls=int(not consumer), sdk_texture_observations=2 if role == "renderer" else int(consumer))))
    return dict(passed=True, evidence_kind=RUNNER["KIND"], scenario=scenario, adapter_luid="intel",
                adapter_description="Intel Graphics", source_extents=[extent, [1280, 720]],
                initial_pixel_upload_calls=2, initial_pixel_upload_bytes=(pixels + 1280 * 720) * 4,
                full_image_readbacks=1, senders_unregistered=True, devices=devices, elapsed_seconds=0.5,
                comparison=dict(extent=extent, compared_pixels=pixels, tolerance_per_component=2,
                                maximum_error_bgra=[1, 2, 0, 1], differing_components_bgra=[0] * 4,
                                mismatched_pixels=0, first_differences=[]))


def process_report(scenario="q01", procedural=False):
    sample = report(scenario)
    sample.update(evidence_kind=RUNNER["PROCEDURAL_KIND"] if procedural else RUNNER["PROCESS_KIND"],
                  fixture="procedural" if procedural else "authored", process_ids=[101,102,100,103],
                  children_reaped=True, source_workers=[])
    for extent in sample["source_extents"]:
        sample["source_workers"].append(dict(adapter_luid="intel",maximum_surfaces=3,peak_surfaces=2,
            publications=2,receive_calls=3,resources=dict(after_fence=copy.deepcopy(sample["devices"][0]["after_fence"]),
            operations=dict(diagnostic_readback_pixels=0,uniform_upload_bytes=0,shader_draws=0,
                sdk_receive_calls=3,sdk_texture_observations=2,
                explicit_gpu_copy_calls=2,explicit_gpu_copy_bytes=2*extent[0]*extent[1]*4)),
            gpu_admission=dict(mechanism="timeline_fence",maximum_in_flight=2,peak_in_flight=2,submitted=3,completed=3,pending=0,reserved=False)))
    sample["devices"][2]["operations"]["sdk_texture_observations"] = 0
    if procedural:
        sample.update(initial_pixel_upload_calls=0,initial_pixel_upload_bytes=0)
        for device in sample["devices"][:2]:
            device["operations"].update(uniform_upload_bytes=32,shader_draws=1)
    return sample


class ColorAccuracyTests(unittest.TestCase):
    def test_process_route_requires_distinct_reaped_children_and_real_drained_bridge_work(self):
        sample = process_report()
        self.assertTrue(RUNNER["validate_report"](sample,"q01",processes=True)["passed"])
        mutations = [lambda r:r.__setitem__("process_ids",[101,101,100,103]),
            lambda r:r.__setitem__("children_reaped",False),
            lambda r:r["source_workers"].pop(),
            lambda r:r["source_workers"][0].__setitem__("maximum_surfaces",4),
            lambda r:r["source_workers"][0]["resources"]["operations"].__setitem__("explicit_gpu_copy_bytes",4),
            lambda r:r["source_workers"][1]["resources"]["after_fence"].__setitem__("live_textures",1),
            lambda r:r["devices"][2]["operations"].__setitem__("sdk_receive_calls",1),
            lambda r:r["source_workers"][0]["resources"]["operations"].__setitem__("sdk_receive_calls",0),
            lambda r:r["source_workers"][0]["gpu_admission"].__setitem__("pending",1)]
        for mutation in mutations:
            bad = copy.deepcopy(sample); mutation(bad)
            with self.assertRaises(ValueError): RUNNER["validate_report"](bad,"q01",processes=True)
        with self.assertRaises(ValueError): RUNNER["validate_report"](sample,"q01")

    def test_procedural_route_requires_shader_work_and_zero_pixel_uploads(self):
        for scenario in ("q01","q02"):
            sample = process_report(scenario,procedural=True)
            self.assertTrue(RUNNER["validate_report"](sample,scenario,processes=True,procedural=True)["passed"])
            for mutate in (lambda r:r.__setitem__("fixture","authored"),
                lambda r:r.__setitem__("initial_pixel_upload_calls",2),
                lambda r:r.__setitem__("initial_pixel_upload_bytes",32),
                lambda r:r["devices"][0]["operations"].__setitem__("shader_draws",0),
                lambda r:r["devices"][1]["operations"].__setitem__("uniform_upload_bytes",0)):
                bad=copy.deepcopy(sample);mutate(bad)
                with self.assertRaises(ValueError):RUNNER["validate_report"](bad,scenario,processes=True,procedural=True)
        with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            RUNNER["parse_args"](["0","q01","--procedural"])

    def test_both_fixed_extents_require_every_pixel_and_preserve_scope(self):
        for scenario, count in (("q01", 1920 * 1080), ("q02", 3840 * 2160)):
            result = RUNNER["validate_report"](report(scenario), scenario)
            self.assertEqual(result["full_image_pixels_verified"], count)
            self.assertIn("no timing", result["scope"])

    def test_tampered_coverage_pixels_roles_uploads_and_retirement_cannot_pass(self):
        mutations = [
            lambda r: r["comparison"].__setitem__("compared_pixels", 16),
            lambda r: r["comparison"].__setitem__("tolerance_per_component", 3),
            lambda r: r["comparison"].__setitem__("maximum_error_bgra", [3, 0, 0, 0]),
            lambda r: r["comparison"].__setitem__("mismatched_pixels", 1),
            lambda r: r["comparison"].__setitem__("differing_components_bgra", [0, 0, 0, 1]),
            lambda r: r.__setitem__("initial_pixel_upload_bytes", 0),
            lambda r: r.__setitem__("initial_pixel_upload_calls", 3),
            lambda r: r.__setitem__("full_image_readbacks", 2),
            lambda r: r.__setitem__("senders_unregistered", False),
            lambda r: r["devices"][2].__setitem__("role", "source_b"),
            lambda r: r["devices"][2]["operations"].__setitem__("diagnostic_readback_pixels", 1),
            lambda r: r["devices"][2]["operations"].__setitem__("uniform_upload_bytes", 32),
            lambda r: r["devices"][3]["operations"].__setitem__("explicit_gpu_copy_bytes", 4),
            lambda r: r["devices"][3]["operations"].__setitem__("sdk_texture_observations", 0),
            lambda r: r["devices"][0]["after_fence"].__setitem__("retiring_textures", 1),
            lambda r: r["devices"][1].__setitem__("retirement_ms", float("nan")),
        ]
        for mutate in mutations:
            sample = copy.deepcopy(report())
            mutate(sample)
            with self.assertRaises(ValueError):
                RUNNER["validate_report"](sample, "q01")

    def test_nvidia_prerequisite_and_adapter_identity_are_checked(self):
        intel = dict(index=0, software=False, description="Intel Graphics", luid="intel")
        nvidia = dict(index=1, software=False, description="NVIDIA GeForce RTX", luid="nvidia")
        software = dict(index=2, software=True, description="Software", luid="software")
        adapters = [intel, nvidia, software]
        self.assertEqual(RUNNER["admit_adapter"](adapters, 0, "on"), intel)
        self.assertEqual(RUNNER["admit_adapter"](adapters, 1, "off"), nvidia)
        for index, overlay in ((1, "on"), (1, "unknown"), (2, "off"), (3, "off")):
            with self.assertRaises(ValueError):
                RUNNER["admit_adapter"](adapters, index, overlay)
        with self.assertRaises(ValueError):
            RUNNER["validate_report"](report(), "q01", nvidia)

    def test_failed_native_json_and_stderr_are_retained_and_exit_nonzero(self):
        failed = report()
        failed["passed"] = False
        failed["comparison"]["mismatched_pixels"] = 5
        inventory = [dict(index=0, software=False, description="Intel Graphics", luid="intel")]
        results = [subprocess.CompletedProcess([], 0, json.dumps(inventory).encode(), b""),
                   subprocess.CompletedProcess([], 1, json.dumps(failed).encode(), b"native failure")]
        namespace = RUNNER["collect"].__globals__
        with mock.patch.dict(namespace, fingerprints=lambda executable: {"unchanged": True}), \
                mock.patch.object(namespace["platform"], "platform", return_value="test-platform"), \
                mock.patch.object(namespace["platform"], "machine", return_value="test-machine"), \
                mock.patch.object(subprocess, "run", side_effect=results):
            evidence, code = RUNNER["collect"](RUNNER["parse_args"](["0", "q01", "--nvidia-overlay", "on"]))
        self.assertEqual(code, 1)
        self.assertEqual(evidence["run"]["report"]["comparison"]["mismatched_pixels"], 5)
        self.assertEqual(evidence["run"]["stderr"], "native failure")
        self.assertFalse(evidence["validation"]["passed"])

    def test_overlay_rejection_does_not_launch_accuracy_native_process(self):
        inventory = [dict(index=1, software=False, description="NVIDIA RTX", luid="nvidia")]
        namespace = RUNNER["collect"].__globals__
        with mock.patch.dict(namespace, fingerprints=lambda executable: {"unchanged": True}), \
                mock.patch.object(namespace["platform"], "platform", return_value="test-platform"), \
                mock.patch.object(namespace["platform"], "machine", return_value="test-machine"), \
                mock.patch.object(subprocess, "run", return_value=subprocess.CompletedProcess([], 0, json.dumps(inventory).encode(), b"")) as run:
            evidence, code = RUNNER["collect"](RUNNER["parse_args"](["1", "q01", "--nvidia-overlay", "on"]))
        self.assertEqual(code, 1)
        self.assertIn("requires verified overlay Off", evidence["preparation_failure"])
        self.assertEqual(run.call_count, 1)

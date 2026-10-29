import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location(
    "production_fingerprint_test", ROOT / "tools/mapgyver/production_qualification.py"
)
ANALYZER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ANALYZER)


class NativeSourceIdentityTests(unittest.TestCase):
    def test_native_sdk_edit_changes_capture_identity_without_executable_or_lock_change(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            executable = root / "owned-fixture.exe"
            executable.write_bytes(b"fixed executable")
            module = root / "tools/mapgyver/production_qualification.py"
            module.parent.mkdir(parents=True)
            module.write_text("fixture identity", encoding="utf-8")
            vendor = root / "vendor/spout2-sys-0.1.1"
            sdk = vendor / "vendor/Spout2/SPOUTSDK/SpoutDirectX/SpoutDX"
            sdk.mkdir(parents=True)
            source = sdk / "SpoutDX.cpp"
            header = sdk / "SpoutDX.h"
            source.write_text("copy implementation one", encoding="utf-8")
            header.write_text("public native contract", encoding="utf-8")
            (vendor / "UPSTREAM.json").write_text("{}", encoding="utf-8")
            (root / "Cargo.lock").write_text("fixed lock", encoding="utf-8")
            with mock.patch.object(ANALYZER, "ROOT", root), \
                    mock.patch.object(ANALYZER, "__file__", str(module)), \
                    mock.patch.object(ANALYZER.subprocess, "run", return_value=mock.Mock(returncode=0, stdout="fixed")), \
                    mock.patch.object(ANALYZER.platform, "platform", return_value="Windows-test"):
                before = ANALYZER.fingerprints(executable)
                source.write_text("copy implementation two", encoding="utf-8")
                after = ANALYZER.fingerprints(executable)
            source_key = source.relative_to(root).as_posix()
            header_key = header.relative_to(root).as_posix()
            baseline_key = (vendor / "UPSTREAM.json").relative_to(root).as_posix()
            self.assertEqual(before["executable_sha256"], after["executable_sha256"])
            self.assertEqual(before["execution_input_sha256"]["Cargo.lock"],
                             after["execution_input_sha256"]["Cargo.lock"])
            self.assertNotEqual(before["execution_input_sha256"][source_key],
                                after["execution_input_sha256"][source_key])
            self.assertEqual(before["execution_input_sha256"][header_key],
                             after["execution_input_sha256"][header_key])
            self.assertIn(baseline_key, before["execution_input_sha256"])


if __name__ == "__main__":
    unittest.main()

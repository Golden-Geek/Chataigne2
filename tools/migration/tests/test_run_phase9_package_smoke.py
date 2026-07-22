from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from tools.migration.run_phase9_package_smoke import select_package_artifact, validate_browser_report


class Phase9PackageSmokeTests(unittest.TestCase):
    def test_selects_one_linux_appimage(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            artifact = root / "appimage" / "Chataigne2.AppImage"
            artifact.parent.mkdir(parents=True)
            artifact.write_bytes(b"appimage")
            self.assertEqual(select_package_artifact(root, "linux"), artifact.resolve())

    def test_requires_complete_product_workflow(self) -> None:
        steps = [
            "runtime-ready",
            "fixture-loaded",
            "outliner-rename",
            "inspector-mutation",
            "live-value-feedback",
            "formula-interaction",
            "state-machine-interaction",
            "project-save",
            "save-reload-verified",
            "temporary-project-unloaded",
        ]
        validate_browser_report(
            {
                "contract": "chataigne-product-browser-gate-v1",
                "status": "passed",
                "steps": [{"step": step} for step in steps],
            }
        )


if __name__ == "__main__":
    unittest.main()

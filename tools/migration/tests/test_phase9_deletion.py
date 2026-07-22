from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from tools.migration.check_phase9_deletion import collect_violations


class Phase9DeletionTests(unittest.TestCase):
    def test_rejects_retired_production_adapter(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for path in (
                "crates/core/src",
                "apps/chataigne",
                "packages/golden-ui",
                "docs/product/manifests",
            ):
                (root / path).mkdir(parents=True)
            for path in ("README.md", "ARCHITECTURE.md", "CONTRIBUTING.md", "docs/architecture.md", "docs/repo-map.md"):
                target = root / path
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_text("current\n", encoding="utf-8")
            (root / "crates/core/src/runtime.rs").write_text(
                'const KEY: &str = "golden.runtime.domain-node-adapter";\n', encoding="utf-8"
            )
            (root / "docs/product/manifests/phase9-qualification.v1.json").write_text(
                json.dumps({"carried_temporary_adapters": []}), encoding="utf-8"
            )
            self.assertTrue(any("domain-node-adapter" in item for item in collect_violations(root)))


if __name__ == "__main__":
    unittest.main()

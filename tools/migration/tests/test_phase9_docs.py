from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from tools.migration.check_phase9_docs import REQUIRED_DOCS, ROOT_LINKS, collect_violations


class Phase9DocumentationTests(unittest.TestCase):
    def test_rejects_broken_required_link(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "README.md").write_text(
                "\n".join(f"[{path}]({path})" for path in ROOT_LINKS), encoding="utf-8"
            )
            for relative in REQUIRED_DOCS:
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("# Doc\n\n" + "content\n" * 7, encoding="utf-8")
            broken = root / REQUIRED_DOCS[0]
            broken.write_text(broken.read_text(encoding="utf-8") + "[missing](missing.md)\n", encoding="utf-8")
            self.assertTrue(any("broken local link" in item for item in collect_violations(root)))


if __name__ == "__main__":
    unittest.main()

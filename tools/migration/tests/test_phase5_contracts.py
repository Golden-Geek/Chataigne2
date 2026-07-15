from __future__ import annotations

import unittest
from pathlib import Path

from tools.migration.check_phase5_contracts import dashboard_violations, implementation_violations


REPOSITORY_ROOT = Path(__file__).resolve().parents[3]


class Phase5ContractTests(unittest.TestCase):
    def test_repository_uses_the_compiled_phase5_runtime_boundaries(self) -> None:
        self.assertEqual(implementation_violations(REPOSITORY_ROOT), [])

    def test_phase5_dashboard_records_the_runnable_checkpoint(self) -> None:
        self.assertEqual(dashboard_violations(REPOSITORY_ROOT), [])


if __name__ == "__main__":
    unittest.main()

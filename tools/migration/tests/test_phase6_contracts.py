from __future__ import annotations

import unittest
from pathlib import Path

from tools.migration.check_phase6_contracts import dashboard_violations, implementation_violations


REPOSITORY_ROOT = Path(__file__).resolve().parents[3]


class Phase6ContractTests(unittest.TestCase):
    def test_repository_uses_the_phase6_runtime_boundaries(self) -> None:
        self.assertEqual(implementation_violations(REPOSITORY_ROOT), [])

    def test_phase6_dashboard_records_declared_validation_state_truthfully(self) -> None:
        self.assertEqual(dashboard_violations(REPOSITORY_ROOT), [])


if __name__ == "__main__":
    unittest.main()

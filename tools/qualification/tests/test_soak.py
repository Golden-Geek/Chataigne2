from __future__ import annotations

import unittest

from tools.qualification.soak import MINIMUM_DURATION_SECONDS, validate_browser_report


class SoakTests(unittest.TestCase):
    def test_accepts_complete_multiclient_report(self) -> None:
        report = {
            "contract": "chataigne-multiclient-soak-v1",
            "status": "passed",
            "durationMs": 1000,
            "clientCount": 2,
            "iterations": 1,
            "clients": [
                {"websocketTotals": {"receivedFrames": 2, "sentFrames": 1}},
                {"websocketTotals": {"receivedFrames": 2, "sentFrames": 1}},
            ],
        }
        validate_browser_report(report, 1000, 2)

    def test_rejects_client_without_bidirectional_traffic(self) -> None:
        report = {
            "contract": "chataigne-multiclient-soak-v1",
            "status": "passed",
            "durationMs": 1000,
            "clientCount": 2,
            "iterations": 1,
            "clients": [
                {"websocketTotals": {"receivedFrames": 2, "sentFrames": 1}},
                {"websocketTotals": {"receivedFrames": 2, "sentFrames": 0}},
            ],
        }
        with self.assertRaisesRegex(ValueError, "bidirectional"):
            validate_browser_report(report, 1000, 2)

    def test_qualification_duration_requires_memory_and_queue_evidence(self) -> None:
        report = {
            "contract": "chataigne-multiclient-soak-v1",
            "status": "passed",
            "durationMs": MINIMUM_DURATION_SECONDS * 1000,
            "clientCount": 2,
            "iterations": 1,
            "clients": [
                {"websocketTotals": {"receivedFrames": 2, "sentFrames": 1}},
                {"websocketTotals": {"receivedFrames": 2, "sentFrames": 1}},
            ],
        }
        with self.assertRaisesRegex(ValueError, "memory plateau"):
            validate_browser_report(report, MINIMUM_DURATION_SECONDS * 1000, 2)


if __name__ == "__main__":
    unittest.main()

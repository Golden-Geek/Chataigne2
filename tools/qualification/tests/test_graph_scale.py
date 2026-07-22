from __future__ import annotations

import unittest

from tools.qualification.graph_scale import (
    MAX_RENDERED_NODE_COUNT,
    REQUIRED_STEPS,
    validate_browser_report,
)


def passing_browser_report() -> dict[str, object]:
    steps = [{"step": step} for step in sorted(REQUIRED_STEPS)]
    formula = next(step for step in steps if step["step"] == "formula-interaction")
    formula.update(
        {
            "totalNodeCount": 10_000,
            "visibleNodeCount": 180,
            "renderedNodeCount": 180,
        }
    )
    return {
        "contract": "chataigne-product-browser-gate-v1",
        "status": "passed",
        "steps": steps,
        "issues": {
            "consoleErrors": [],
            "pageErrors": [],
            "requestFailures": [],
            "httpErrors": [],
        },
        "network": {"totals": {"receivedFrames": 12, "sentFrames": 3}},
    }


class GraphScaleTests(unittest.TestCase):
    def test_accepts_complete_bounded_full_workbench_report(self) -> None:
        result = validate_browser_report(passing_browser_report(), 10_000)

        self.assertTrue(result["passed"])
        self.assertEqual(result["total_node_count"], 10_000)
        self.assertEqual(result["visible_node_count"], 180)
        self.assertEqual(result["rendered_node_count"], 180)

    def test_rejects_truncated_or_unbounded_graph_projection(self) -> None:
        report = passing_browser_report()
        formula = next(
            step
            for step in report["steps"]
            if step["step"] == "formula-interaction"
        )
        formula["totalNodeCount"] = 9_999
        formula["visibleNodeCount"] = MAX_RENDERED_NODE_COUNT + 1
        formula["renderedNodeCount"] = MAX_RENDERED_NODE_COUNT + 1

        result = validate_browser_report(report, 10_000)

        self.assertFalse(result["passed"])
        self.assertTrue(any("expected 10000" in error for error in result["errors"]))
        self.assertTrue(any("bounded-DOM" in error for error in result["errors"]))

    def test_rejects_missing_product_workflow_steps_and_runtime_traffic(self) -> None:
        report = passing_browser_report()
        report["steps"] = [
            step for step in report["steps"] if step["step"] != "save-reload-verified"
        ]
        report["network"] = {"totals": {"receivedFrames": 0, "sentFrames": 0}}

        result = validate_browser_report(report, 10_000)

        self.assertFalse(result["passed"])
        self.assertTrue(any("missing steps" in error for error in result["errors"]))
        self.assertTrue(any("no runtime WebSocket frames" in error for error in result["errors"]))


if __name__ == "__main__":
    unittest.main()

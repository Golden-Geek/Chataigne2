"""Analyze Screen-only native costs with the existing production timing/coverage oracles."""

import argparse
import json
from pathlib import Path
import runpy

HERE = Path(__file__).resolve().parent
METRICS = runpy.run_path(str(HERE / "production_qualification.py"))
DEADLINES = runpy.run_path(str(HERE / "production_deadlines.py"))


def analyze(directory):
    rows = METRICS["load_rows"](directory / "renderer.jsonl")
    capture = METRICS["one"](rows, "capture")
    finish = METRICS["one"](rows, "finish")
    native = METRICS["events"](rows)
    frequency = capture["qpc_frequency"]
    health = METRICS["health"](finish["diagnostics"], rows)
    samples = []
    for window in [row for row in rows if row["kind"] == "phase" and row["name"].startswith("sample-")]:
        frames, cpu = METRICS["cpu_metrics"](native["FrameCpu"], window, frequency)
        gpu = METRICS["gpu_metrics"](frames, native["Gpu"], renderer=True)
        gpu["stage_partitions"].pop("records", None)
        deadlines = DEADLINES["analyze_deadlines"](native["Schedule"], native["Output"], window, frequency, (1,))
        # Large per-slot joins remain reproducible in the raw stream; retain the verdict/counts.
        deadlines.pop("slots", None)
        execution = gpu["context_local_execution"]
        wall = cpu["inclusive_wall_upper_bound"]
        samples.append({
            "window": window,
            "seconds": (window["end_qpc"] - window["begin_qpc"]) / frequency,
            "cpu": cpu,
            "gpu": gpu,
            "software_deadlines": deadlines,
            "cpu_upper_bound_within_budget": bool(wall and wall["p95_ms"] <= 4 and wall["p99_ms"] <= 8),
            "gpu_execution_within_budget": bool(execution and execution["p95_ms"] <= 12 and execution["p99_ms"] <= 16.667),
        })
    METRICS["require"](samples, "no declared Screen cost samples")
    complete = health["complete"] and all(sample["gpu"]["complete"] for sample in samples)
    return {
        "capture": capture,
        "telemetry": health,
        "samples": samples,
        "complete_cpu_gpu_coverage": complete,
        "scoped_costs_within_budget": complete and all(
            sample["cpu_upper_bound_within_budget"] and sample["gpu_execution_within_budget"] for sample in samples
        ),
        "sustained_series": not capture["diagnostic"] and len(samples) == 3
        and all(sample["seconds"] == 120 for sample in samples),
        "finish": finish,
        "scope": "Screen-only renderer context with no external workers; no Q01/Q02, source age, physical delivery or scanout acceptance",
        "raw_sha256": METRICS["digest"](directory / "renderer.jsonl"),
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("directory", type=Path)
    args = parser.parse_args()
    results = {folder.name: analyze(folder) for folder in sorted(args.directory.iterdir())
               if folder.is_dir() and (folder / "renderer.jsonl").exists()}
    assert results, "no owned Screen cost captures"
    (args.directory / "analysis.json").write_text(json.dumps(results, indent=2) + "\n", encoding="utf8")
    for name, result in results.items():
        print(json.dumps({"case": name, "complete": result["complete_cpu_gpu_coverage"],
                          "costs_within_budget": result["scoped_costs_within_budget"],
                          "samples": [{"cpu": sample["cpu"]["inclusive_wall_upper_bound"],
                                       "gpu": sample["gpu"]["context_local_execution"],
                                       "gpu_coverage": sample["gpu"]["complete"],
                                       "deadline_verdict": sample["software_deadlines"]["submission_verdict"]}
                                      for sample in result["samples"]]}))


if __name__ == "__main__":
    main()

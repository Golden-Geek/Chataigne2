"""Validate headless progress controls without turning them into rendering qualification."""

import argparse
import json
from pathlib import Path


def require(condition, message):
    if not condition:
        raise ValueError(message)


def analyze_run(run):
    if run["exit_code"] != 0 or run.get("report") is None:
        return {"native_success": False, "failure": run}
    r = run["report"]
    before, after = r["before_final_fence"], r["after_final_fence"]
    window = before["sampled"]
    require(r["seconds"] == 8 and r["target_ticks"] == 480, "control window changed")
    require(before["maximum_in_flight"] == after["maximum_in_flight"] == 2
            and before["peak_in_flight"] <= 2, "admission bound changed")
    require(window["attempts"] == window["admitted"] + window["busy"] <= 480, "attempt accounting")
    require(r["tick_work"]["samples"] == window["attempts"], "CPU tick coverage")
    require(before["submitted"] == window["admitted"] == after["submitted"] == after["completed"], "completion accounting")
    require(before["submitted"] - before["completed"] == before["pending"] <= 2, "pending accounting")
    require(not before["reserved"] and not after["reserved"] and after["pending"] == 0, "retained admission")
    require(after["sampled"] == window and after["pending_at_final_poll"] == before["pending"], "drain changed observations")
    ledger = r["allocations_after_fence"]
    require(ledger["live_textures"] == ledger["retiring_textures"] == 0
            and not any(ledger["live_bytes_by_owner"] + ledger["retiring_bytes_by_owner"]), "retained texture ownership")
    mode = r["mode"]
    expected_mechanism = "timeline_fence" if mode.startswith("fence") else "event_query"
    require(after.get("mechanism", "event_query") == expected_mechanism, "wrong admission mechanism")
    ops = r["operations_including_seed"]
    expected_clears = 1 + int(r["drain_clear_after_observation"]) + (window["admitted"] if mode == "clear_warmup" else 0)
    require(ops["clears"] == expected_clears, "unreported GPU padding")
    require(ops["sdk_receive_calls"] == (window["admitted"] if mode.startswith("receiver") else 0), "receive accounting")
    require(all(ops[k] == 0 for k in ("diagnostic_readback_pixels", "uniform_upload_bytes",
                                    "explicit_gpu_copy_calls", "sdk_send_calls", "shader_draws")), "unexpected control work")
    events = r["busy_ticks"]
    require(len(events) == window["busy"], "busy observations missing")
    frequency = r["qpc_frequency"]
    require(frequency > 0, "QPC frequency missing")
    previous = -1
    previous_qpc = r["start_qpc"]
    bursts = []
    for event in events:
        frame, qpc = event["frame"], event["qpc"]
        require(previous < frame < window["attempts"] and previous_qpc <= qpc, "busy event order")
        require(qpc - r["start_qpc"] <= frequency * 8.001, "busy stamp outside observation")
        if not bursts or frame != previous + 1:
            bursts.append({"first_tick": frame, "first_qpc": qpc, "ticks": 0})
        bursts[-1]["last_tick"] = frame
        bursts[-1]["last_qpc"] = qpc
        bursts[-1]["ticks"] += 1
        previous, previous_qpc = frame, qpc
    timing = r["timing"]
    if mode in ("event", "fence", "fence_no_drain"):
        require(timing is None, "unexpected profiling")
    elif mode in ("timestamp", "receiver_timestamp"):
        require(timing["cpu_submission_work"]["samples"] == window["admitted"], "profiling CPU count")
        count = timing["gpu_execution"]["samples"] if timing["gpu_execution"] else 0
        require(count + sum(timing[k] for k in ("skipped_gpu_queries", "invalid_gpu_queries", "pending_gpu_queries"))
                == window["admitted"], "profiling GPU count")
    else:
        require(timing["cpu_submission_work"] is None and timing["gpu_execution"] is None,
                "warmup acquired fabricated distributions")
    return {"native_success": True, "mode": mode, "mechanism": expected_mechanism,
            "attempts": window["attempts"], "admitted": window["admitted"], "busy": window["busy"],
            "busy_bursts": bursts, "tick_work": r["tick_work"], "zero_after_fence": True,
            "drain_clear_after_observation": r["drain_clear_after_observation"]}


def analyze(artifact):
    require(artifact["inputs_unchanged_after_runs"], "control inputs changed")
    return {"scope": "headless progress isolation, not Q01/Q02 qualification",
            "runs": [analyze_run(run) for run in artifact["runs"]]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("artifact", type=Path)
    args = parser.parse_args()
    print(json.dumps(analyze(json.loads(args.artifact.read_text(encoding="utf-8"))), indent=2))


if __name__ == "__main__":
    main()

"""Verify real JS/document interruption and independent output progress, not scanout/FPS."""

import argparse
import json
from pathlib import Path
import runpy

RELEASE = runpy.run_path(str(Path(__file__).with_name("analyze-release.py")))


def require(condition, message):
    if not condition:
        raise ValueError(message)


def verify_windows(proof, age):
    frequency = proof["qpc_frequency"]
    require(frequency > 0 and proof["frontend_busy_elapsed_ms"] >= 65_000, "busy duration missing")
    require(proof["busy_finished_qpc"] - proof["busy_started_qpc"] >= 65 * frequency, "host busy interval too short")
    require([w["kind"] for w in proof["windows"]] == ["javascript_busy", "blank_document"], "interruption phases missing")
    summaries = []
    for index, window in enumerate(proof["windows"]):
        snapshots = window["snapshots"]
        require(len(snapshots) == 62, "sparse interruption observations missing")
        first, last = snapshots[0], snapshots[-1]
        require(last["qpc"] - first["qpc"] >= 60 * frequency, "interruption observation under 60 seconds")
        require(first["layout_session"] > 0 and all(s["layout_session"] == first["layout_session"] and s["editor_requests"] == first["editor_requests"]
                    for s in snapshots), "frontend remained active or document changed")
        if index == 0:
            require(proof["busy_started_qpc"] < first["qpc"] < last["qpc"] < proof["busy_finished_qpc"],
                    "observations are not inside real JS blockage")
            require(all(s["preview_regions"] == 2 and s["layout_revision"] == first["layout_revision"]
                        for s in snapshots), "busy document registrations changed")
        else:
            require(first["layout_session"] > proof["windows"][0]["snapshots"][-1]["layout_session"],
                    "blank document did not invalidate the old session")
            require(first["qpc"] > proof["busy_finished_qpc"] and all(
                s["preview_regions"] == s["layout_revision"] == s["rendered_revision"] == 0
                for s in snapshots), "stale preview registration survived")
        unique = [set(), set()]
        for left, right in zip(snapshots, snapshots[1:]):
            require(left["qpc"] < right["qpc"] and right["output_presents"] > left["output_presents"]
                    and right["spout_sends"] > left["spout_sends"], "output stopped in observation interval")
            samples = [r for r in age["sample_rows"] if left["qpc"] <= r[1] < right["qpc"]]
            require(samples, "consumer coverage missing during interruption")
            for channel in range(2):
                require(all(r[3 + channel * 5] == 0xa671_c300 | channel for r in samples),
                        "source unavailable during interruption")
                for row in samples:
                    frame, magic, low, high, checksum = row[2 + channel*5:7 + channel*5]
                    qpc = low | high << 32
                    expected = RELEASE["rotate"](frame, 7) ^ low ^ RELEASE["rotate"](high, 13) ^ magic ^ 0x9e37_79b9
                    require(checksum == expected and 0 < qpc <= row[1], "invalid marker during interruption")
                frames = {r[2 + channel * 5] for r in samples}
                require(len(frames) > 1, "consumer pixels froze during interruption")
                unique[channel].update(frames)
        summaries.append(dict(kind=window["kind"], observed_seconds=(last["qpc"]-first["qpc"])/frequency,
                              native_output_calls=last["output_presents"]-first["output_presents"],
                              spout_calls=last["spout_sends"]-first["spout_sends"],
                              unique_observed_fixture_frames=list(map(len, unique)), editor_poll_requests=0))
    reconnected = proof["reconnected"]
    require(reconnected["qpc"] > proof["windows"][-1]["snapshots"][-1]["qpc"]
            and reconnected["layout_session"] > proof["windows"][-1]["snapshots"][-1]["layout_session"]
            and reconnected["preview_regions"] == 2
            and reconnected["layout_revision"] > 0
            and reconnected["rendered_revision"] == reconnected["layout_revision"], "fresh document did not render")
    return summaries


def analyze(evidence):
    require(evidence["schema_version"] >= 15 and evidence["inputs_unchanged_after_run"], "stale or changing native inputs")
    require(evidence["run"]["exit_code"] == 0 and not evidence["run"]["stderr"], "native exercise failed")
    report = evidence["run"]["report"]
    require(report["workload"]["exercise"] == "editor_isolation" and len(report["generations"]) == 1,
            "not the dedicated editor exercise")
    g = report["generations"][0]
    require(g["end"] == "editor_isolation_complete" and report["editor_isolation"] is not None, "incomplete exercise")
    age = report["observer"]["frame_age"]
    RELEASE["verify_age"](age)
    RELEASE["verify_operations"](g)
    RELEASE["verify_gpu_admission"](g, report["resources"])
    RELEASE["verify_source_workers"](g)
    require(RELEASE["zero"](g["allocations_after_fence"]) and all(RELEASE["zero"](c["resources"]["after_fence"])
            for c in [*g["source_workers"], *report["sources"], report["observer"]]), "retained texture ownership")
    require(g["operations"]["diagnostic_readback_pixels"] == g["operations"]["uniform_upload_bytes"] == 0,
            "renderer pixel readback or upload")
    return dict(scope="busy JS and blank-document output continuity/reconnect; not displayed cadence or a performance series",
                windows=verify_windows(report["editor_isolation"], age), raw_consumer_marker_accounting_verified=True,
                whole_session_invalid_markers=[c["invalid"] for c in age["channels"]],
                whole_session_absent_markers=[c["absent"] for c in age["channels"]],
                fresh_document_restored=True, zero_tracked_after_fences=True, shutdown_seconds=report["shutdown_seconds"])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("artifact", type=Path)
    args = parser.parse_args()
    artifact = json.loads(args.artifact.read_text(encoding="utf-8"))
    print(json.dumps([analyze(RELEASE["RUNNER"]["decode_evidence"](p)) for p in artifact["packets"]], indent=2))


if __name__ == "__main__":
    main()

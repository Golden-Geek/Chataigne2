"""Audit one Q06 app capture; unknown full-path measurements never become passes."""
from collections import Counter, defaultdict
import json
from pathlib import Path
import runpy
import sys

METRICS = runpy.run_path(str(Path(__file__).with_name("production_qualification.py")))
VIDEO_AGE = runpy.run_path(str(Path(__file__).with_name("video_age.py")))


def key(value):
    return json.dumps(value, sort_keys=True)


def publication_age_lower_bound(upload, output, frequency, *, precedes_publication=False):
    if frequency <= 0:
        raise ValueError("positive QPC frequency required")
    if precedes_publication:
        # Source-worker upload ends before completed GPU publication. Its end is not an
        # upper bound on publication time and must not inflate a source-age failure bound.
        return None
    cpu, api = upload.get("cpu"), output.get("consumer_api")
    if cpu is None or api is None:
        return None
    if api["begin"] < cpu["end"]:
        raise ValueError("upload/output counter order is invalid")
    return (api["begin"] - cpu["end"]) * 1000 / frequency


def observed_source_uploads(uses):
    uploads = {}
    for use in uses:
        upload = use.get("software_upload")
        if upload is None:
            continue
        identity = (upload["stream"], upload["sequence"])
        if identity in uploads and uploads[identity] != upload:
            raise ValueError("source upload identity changed across observers")
        uploads[identity] = upload
    return list(uploads.values())


def analyze(directory):
    report = json.loads((directory / "sample.json").read_text(encoding="utf-8"))
    events, snapshots = defaultdict(list), []
    for capture in report["nativeDiagnostics"]["captures"]:
        with open(capture["file"], encoding="utf-8") as source:
            for line in source:
                row = json.loads(line)
                if row["kind"] == "diagnostic":
                    for kind, value in row["event"].items():
                        events[kind].append(value)
                elif row["kind"] == "snapshot":
                    snapshots.append(row)
    frames = {key(row["token"]): row for row in events["FrameCpu"]}
    uploaded = {(key(row["token"]["device"]), row["texture"], row["stream"], row["sequence"]): row
                for row in events["ImageUpload"]}
    source_uploads = observed_source_uploads(events["SourceUse"])
    all_uploads = events["ImageUpload"] + source_uploads
    layouts = {row["stream"]: (row["row_pitch"] // 4, row["rows"]) for row in all_uploads}
    uses = defaultdict(list)
    for use in events["SourceUse"]:
        if use["stream"] is not None and use["sequence"]:
            uses[key(use["token"])].append(use)
    output_sources, receiver_summaries, receiver_frames, receiver_clocks = {}, [], {}, {}
    for index, root in enumerate(report["consumerDirectories"]):
        handles = set()
        final = None
        copied, clock = [], None
        with open(Path(root) / "consumer.jsonl", encoding="utf-8") as source:
            for line in source:
                row = json.loads(line)
                if row["kind"] == "consumer_frame" and row["frame"]["spout_image"]:
                    handles.add(row["frame"]["spout_image"]["shared_handle"])
                    copied.append(row['frame'])
                elif row['kind'] == 'consumer_config' and row['consumer']['frame_count'] == 'VersionedV1':
                    clock = row.get('qpc_frequency')
                elif row["kind"] == "consumer_final":
                    final = row
        assert final is not None and handles, "missing independent receiver completion/images"
        ids = {row["output"] for row in events["Output"] if row["spout_image"]
               and row["spout_image"]["shared_handle"] in handles}
        assert len(ids) == 1, "ambiguous shared-image owner"
        fixture = report["fixtures"]["cases"][index]
        identity = next(iter(ids))
        output_sources[identity] = (fixture["width"], fixture["height"])
        receiver_frames[identity], receiver_clocks[identity] = copied, clock
        receiver_summaries.append({"directory": root, "outputs": list(ids), "final": final,
                                   "scope": "SDK image lineage/copy completion; legacy CardStamp words are not Video frame-code validation"})
    output_ids = sorted({row["output"] for row in events["Output"] if row["spout_image"]})
    results = []
    for window in report["phases"]:
        if window["name"] == "warmup":
            continue
        begin, end, frequency = window["begin_qpc"], window["end_qpc"], window["frequency"]
        seconds = (end - begin) / frequency
        selected, cpu = METRICS["cpu_metrics"](events["FrameCpu"], window, frequency)
        gpu = METRICS["gpu_metrics"](selected, events["Gpu"], renderer=True)
        deadlines = METRICS["DEADLINES"]["analyze_deadlines"](events["Schedule"], events["Output"], window, frequency, tuple(output_ids))
        deadlines.pop("slots", None)
        if "late_examples" in deadlines:
            deadlines["late_example_count"] = len(deadlines["late_examples"])
            deadlines["late_examples"] = deadlines["late_examples"][:10]
        native = [row for row in snapshots if begin <= row["qpc"] < end]
        source_stats = []
        selected_tokens = {key(row["token"]) for row in selected}
        age = defaultdict(list)
        age_unknown = Counter()
        for output in events["Output"]:
            token = key(output["token"])
            api = output["consumer_api"]
            if output["output"] not in output_sources or output["outcome"] != "Accepted" or api is None or not begin <= api["begin"] < end:
                continue
            dependencies = [use for use in uses[token] if layouts.get(use["stream"]) == output_sources[output["output"]]]
            lower = None
            if len(dependencies) == 1:
                use = dependencies[0]
                upload = use.get("software_upload") or uploaded.get((key(use["token"]["device"]), use["texture"], use["stream"], use["sequence"]))
                if upload is not None:
                    lower = publication_age_lower_bound(upload, output, frequency,
                                                        precedes_publication=use.get("software_upload") is not None)
            if lower is None:
                age_unknown[output["output"]] += 1
            # Unknown ages contribute zero only to this failure lower bound, never a passing age.
            age[output["output"]].append(0.0 if lower is None else lower)
        for fixture in report["fixtures"]["cases"]:
            layout = (fixture["width"], fixture["height"])
            uploads = [row for row in all_uploads if row["cpu"] and begin <= row["cpu"]["begin"] < end
                       and layouts[row["stream"]] == layout]
            visible = [use for token in selected_tokens for use in uses[token] if layouts.get(use["stream"]) == layout]
            distinct = {(use["stream"], use["sequence"]) for use in visible}
            repeated = Counter((key(frames[key(row["token"])]["generation"]), row["stream"], row["sequence"])
                               for row in uploads if "token" in row and key(row["token"]) in frames)
            source_stats.append({"width": layout[0], "height": layout[1], "requested_fps": fixture["rate"],
                                 "visible_publications": len(distinct), "visible_publication_rate": len(distinct) / seconds,
                                 "observed_upload_calls": len(uploads), "upload_bytes": sum(row["bytes"] for row in uploads),
                                 "upload_coverage": "Observed source-worker identities deduplicated across consumers, plus render-thread uploads. Unsampled uploads/final producer counters remain unknown; not complete per-format accounting.",
                                 "upload_bytes_per_second": sum(row["bytes"] for row in uploads) / seconds,
                                 "duplicate_uploads_within_one_resource_generation": sum(count - 1 for count in repeated.values()),
                                 "duplicate_scope": "Render-thread uploads only; source counter observations are deduplicated, and repeated source uploads of one original PTS remain unknown without continuous PTS lineage.",
                                 "uploaded_rows": sorted({row["rows"] for row in uploads}),
                                 "uploaded_row_pitches": sorted({row["row_pitch"] for row in uploads})})
        private = [row["process"]["memory"]["private_bytes"] for row in native if row["process"] and row["process"]["memory"]]
        known = [row["allocations"]["owned_texel_bytes"] for row in native if row["allocations"]["owned_texel_bytes"] is not None]
        results.append({"phase": window["name"], "seconds": seconds, "cpu": cpu, "renderer_gpu": gpu,
                        "software_output_deadlines": deadlines, "sources": source_stats,
                        "source_to_output_age_lower_bound_ms": {str(output): METRICS["distribution"](values) for output, values in age.items()},
                        "unknown_age_rows_zeroed_only_for_failure_bound": dict(age_unknown),
                        "file_publication_age": VIDEO_AGE['analyze_video_age'](
                            events['Output'], events['SourceUse'], layouts, output_sources,
                            receiver_frames, receiver_clocks, window, frames),
                        "process_private_peak": max(private, default=None),
                        "known_native_texel_peak": max(known, default=None),
                        "known_native_scope": "plan/Spout/window inventory, excludes unknown Video decoder/pool accounting",
                        "dxgi": [row["adapter_memory"] for row in native if row["adapter_memory"]]})
    return {"adapter": report["adapter"], "pid": report["pid"], "samples": results,
            "independent_receivers": receiver_summaries,
            "scope": "One warmed sample, not the required three independent samples. Renderer GPU is one context. CPU upload end supplies only an inward failure bound. Timestamped GPU File publication can enclose age outward through an exact independent SDK-copy completion; unmeasured images remain unbounded. This is not continuous decoded Video pixel/PTS or display proof. Unmapped outputs, CPU-only publications, complete decoder/pool and unsampled-upload coverage remain required."}


if __name__ == "__main__":
    root = Path(sys.argv[1])
    result = analyze(root)
    (root / "audit.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"result": "baseline audited", "adapter": result["adapter"],
                      "samples": [{"phase": item["phase"], "sources": item["sources"],
                      "age_lower_bound": item["source_to_output_age_lower_bound_ms"],
                      "deadlines": item["software_output_deadlines"].get("outputs")}
                                  for item in result["samples"]]}))

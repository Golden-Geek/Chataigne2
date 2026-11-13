"""Audit one Q06 app capture; unknown full-path measurements never become passes."""
from collections import Counter, defaultdict
import argparse
import json
from pathlib import Path, PureWindowsPath
import runpy

METRICS = runpy.run_path(str(Path(__file__).with_name("production_qualification.py")))
VIDEO_AGE = runpy.run_path(str(Path(__file__).with_name("video_age.py")))
VIDEO_PIXELS = runpy.run_path(str(Path(__file__).with_name("video_frame_codes.py")))
VIDEO_SOURCES = runpy.run_path(str(Path(__file__).with_name("video_submissions.py")))


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


def streams_by_file(bindings):
    """Windows File identities, never resolution, group immutable stream generations."""
    files, streams = defaultdict(set), {}
    for binding in bindings:
        stream, file = binding['stream'], PureWindowsPath(binding['file'])
        if stream in streams and streams[stream] != binding:
            raise ValueError('one stream changed its File/channel binding')
        streams[stream] = binding
        files[file].add(stream)
    return files


def source_phase_stats(fixture, streams, uploads, visible, frames, seconds):
    uploads = [row for row in uploads if row['stream'] in streams]
    distinct = {(use['stream'], use['sequence']) for use in visible if use['stream'] in streams}
    repeated = Counter((key(frames[key(row['token'])]['generation']), row['stream'], row['sequence'])
                       for row in uploads if 'token' in row and key(row['token']) in frames)
    return {'file': fixture['file'], 'streams': sorted(streams), 'source_binding_available': bool(streams),
            'width': fixture['width'], 'height': fixture['height'], 'requested_fps': fixture['rate'],
            'visible_publications': len(distinct), 'visible_publication_rate': len(distinct) / seconds,
            'observed_upload_calls': len(uploads), 'upload_bytes': sum(row['bytes'] for row in uploads),
            'upload_coverage': 'File-bound observations only; source-worker identities deduplicated across consumers, '
                               'plus render-thread uploads. Missing bindings/unsampled uploads remain unknown.',
            'upload_bytes_per_second': sum(row['bytes'] for row in uploads) / seconds,
            'duplicate_uploads_within_one_resource_generation': sum(count - 1 for count in repeated.values()),
            'duplicate_scope': 'Render-thread uploads only; repeated source uploads of one PTS require continuous lineage.',
            'uploaded_rows': sorted({row['rows'] for row in uploads}),
            'uploaded_row_pitches': sorted({row['row_pitch'] for row in uploads})}


def analyze(directory):
    report = json.loads((directory / "sample.json").read_text(encoding="utf-8"))
    events, snapshots, capture_health = defaultdict(list), [], []
    for capture in report["nativeDiagnostics"]["captures"]:
        observed_events, summary = 0, None
        with open(capture["file"], encoding="utf-8") as source:
            for line in source:
                row = json.loads(line)
                if row["kind"] == "diagnostic":
                    observed_events += 1
                    for kind, value in row["event"].items():
                        events[kind].append(value)
                elif row["kind"] == "snapshot":
                    snapshots.append(row)
                elif row["kind"] == "capture_end":
                    summary = row["summary"]
        capture_health.append({"summary": summary or {}, "observed_events": observed_events})
    bindings = report.get("videoSourceBindings")
    if bindings is None:
        bindings = VIDEO_SOURCES["bindings_from_log"](report.get("backendLog", ""))
    source_attribution = VIDEO_SOURCES["analyze_source_submissions"](
        events, bindings, [row.get("source_gpu") for row in snapshots], capture_health)
    file_streams = streams_by_file(bindings)
    frames = {key(row["token"]): row for row in events["FrameCpu"]}
    uploaded = {(key(row["token"]["device"]), row["texture"], row["stream"], row["sequence"]): row
                for row in events["ImageUpload"]}
    source_uploads = observed_source_uploads(events["SourceUse"])
    all_uploads = events["ImageUpload"] + source_uploads
    layouts = {row["stream"]: (row["row_pitch"] // 4, row["rows"]) for row in all_uploads}
    for use in events['SourceUse']:
        if use.get('stream') is not None and use.get('descriptor') is not None:
            extent = use['descriptor']['extent']
            layout = (extent['width'], extent['height'])
            if use['stream'] in layouts and layouts[use['stream']] != layout:
                raise ValueError('source lifetime changed its immutable descriptor')
            layouts[use['stream']] = layout
    uses = defaultdict(list)
    for use in events["SourceUse"]:
        if use["stream"] is not None and use["sequence"]:
            uses[key(use["token"])].append(use)
    output_sources, receiver_summaries, receiver_frames, receiver_clocks = {}, [], {}, {}
    output_streams = {}
    receiver_markers, numbered_fixtures = {}, {}
    for index, root in enumerate(report["consumerDirectories"]):
        handles = set()
        final = None
        copied, clock, marker = [], None, None
        with open(Path(root) / "consumer.jsonl", encoding="utf-8") as source:
            for line in source:
                row = json.loads(line)
                if row["kind"] == "consumer_frame" and row["frame"]["spout_image"]:
                    handles.add(row["frame"]["spout_image"]["shared_handle"])
                    copied.append(row['frame'])
                elif row['kind'] == 'consumer_config' and row['consumer']['frame_count'] == 'VersionedV1':
                    clock = row.get('qpc_frequency')
                    marker = row['consumer'].get('binary_frame_code')
                elif row["kind"] == "consumer_final":
                    final = row
        assert final is not None and handles, "missing independent receiver completion/images"
        ids = {row["output"] for row in events["Output"] if row["spout_image"]
               and row["spout_image"]["shared_handle"] in handles}
        assert len(ids) == 1, "ambiguous shared-image owner"
        fixture = report["fixtures"]["cases"][index]
        identity = next(iter(ids))
        output_sources[identity] = (fixture["width"], fixture["height"])
        output_streams[identity] = file_streams[PureWindowsPath(fixture['file'])]
        receiver_frames[identity], receiver_clocks[identity] = copied, clock
        receiver_markers[identity] = marker
        spec = report['fixtures']['frame_code']
        numbered_fixtures[identity] = {**fixture, 'streams': output_streams[identity], 'marker': {
            'bits': spec['bits'], 'origin': [spec['x'], spec['y']], 'step': spec['step'],
            'sentinel': [spec['sentinel_x'], spec['y']], 'threshold': spec['threshold']}}
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
            dependencies = [use for use in uses[token] if use['stream'] in output_streams[output['output']]
                            and layouts.get(use["stream"]) == output_sources[output["output"]]]
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
        uploads = [row for row in all_uploads if row['cpu'] and begin <= row['cpu']['begin'] < end]
        visible = [use for token in selected_tokens for use in uses[token]]
        for fixture in report["fixtures"]["cases"]:
            source_stats.append(source_phase_stats(fixture, file_streams[PureWindowsPath(fixture['file'])],
                                                   uploads, visible, frames, seconds))
        private = [row["process"]["memory"]["private_bytes"] for row in native if row["process"] and row["process"]["memory"]]
        known = [row["allocations"]["owned_texel_bytes"] for row in native if row["allocations"]["owned_texel_bytes"] is not None]
        results.append({"phase": window["name"], "seconds": seconds, "cpu": cpu, "renderer_gpu": gpu,
                        "video_pixel_pts": VIDEO_PIXELS['analyze_frame_codes'](
                            events, receiver_frames, numbered_fixtures, receiver_markers, window, frames),
                        "software_output_deadlines": deadlines, "sources": source_stats,
                        "source_to_output_age_lower_bound_ms": {str(output): METRICS["distribution"](values) for output, values in age.items()},
                        "unknown_age_rows_zeroed_only_for_failure_bound": dict(age_unknown),
                        "file_publication_age": VIDEO_AGE['analyze_video_age'](
                            events['Output'], events['SourceUse'], layouts, output_sources,
                            receiver_frames, receiver_clocks, window, frames, output_streams=output_streams,
                            output_routes=events['OutputSource']),
                        "process_private_peak": max(private, default=None),
                        "known_native_texel_peak": max(known, default=None),
                        "known_native_scope": "plan/Spout/window inventory, excludes unknown Video decoder/pool accounting",
                        "dxgi": [row["adapter_memory"] for row in native if row["adapter_memory"]]})
    return {"adapter": report["adapter"], "pid": report["pid"], "samples": results,
            "source_submission_attribution": source_attribution,
            "independent_receivers": receiver_summaries,
            "scope": "One warmed sample, not the required three independent samples. Renderer GPU is one context. CPU upload end supplies only an inward failure bound. Timestamped CPU/GPU File publication can enclose age outward through an exact independent SDK-copy completion; unmeasured images remain unbounded. This is not continuous decoded Video pixel/PTS or display proof. Unmapped outputs, untagged publications and complete decoder/pool coverage remain required. Source CPU payload attribution is evaluated separately; missing rows/bindings remain incomplete."}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--require-source-attribution", action="store_true",
                        help="fail on incomplete CPU upload/PTS/channel attribution; does not qualify performance")
    args = parser.parse_args()
    root = args.directory
    result = analyze(root)
    (root / "audit.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    if args.require_source_attribution:
        VIDEO_SOURCES["require_complete_attribution"](result["source_submission_attribution"])
    if any(o['mismatched_images'] or o['invalid_markers'] for s in result['samples']
           for o in s['video_pixel_pts']['outputs']):
        raise ValueError('decoded Video pixels disagree with retained source PTS; see audit.json')
    print(json.dumps({"result": "baseline audited", "adapter": result["adapter"],
                      "source_upload_attribution_complete": result["source_submission_attribution"]
                          ["source_worker_cpu_uploads"]["complete_cpu_payload_pts_attribution"],
                      "samples": [{"phase": item["phase"], "sources": item["sources"],
                      "age_lower_bound": item["source_to_output_age_lower_bound_ms"],
                      "deadlines": item["software_output_deadlines"].get("outputs")}
                                  for item in result["samples"]]}))

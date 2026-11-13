"""Offline source payload attribution; missing facts never establish complete coverage."""
import json


def integer(value, label, minimum=0, maximum=(1 << 64) - 1):
    if type(value) is not int or not minimum <= value <= maximum:
        raise ValueError(f"invalid {label}")
    return value


def identity(value):
    return json.dumps(value, sort_keys=True)


def bindings_from_log(log):
    records = []
    marker = "[mapgyver-video-source] "
    for line in log.splitlines():
        if marker in line:
            records.append(json.loads(line.split(marker, 1)[1]))
    return records


def inventory(snapshots):
    """Deduplicate coherent lifetime counters, including out-of-order capture files."""
    budgets, unknown = {}, False
    for snapshot in snapshots:
        if snapshot is None or snapshot.get("uploads") is None:
            unknown = True
            continue
        current = {}
        for kind in ("software", "hap"):
            row = snapshot["uploads"][kind]
            calls = integer(row["calls"], "upload calls")
            size = integer(row["bytes"], "upload bytes")
            if type(row["overflowed"]) is not bool or (calls == 0) != (size == 0):
                raise ValueError("incoherent upload counters")
            unknown |= row["overflowed"]
            current[kind] = {"calls": calls, "bytes": size}
        budget = snapshot["id"]
        previous = budgets.get(budget)
        if previous:
            pairs = [(current[k][f], previous["uploads"][k][f])
                     for k in current for f in ("calls", "bytes")]
            if not (all(a >= b for a, b in pairs) or all(a <= b for a, b in pairs)):
                raise ValueError("source budget counters did not advance coherently")
            for kind in current:
                a, b = current[kind], previous["uploads"][kind]
                if (a["calls"] == b["calls"]) != (a["bytes"] == b["bytes"]):
                    raise ValueError("equal upload counts changed their bytes")
            if any(a < b for a, b in pairs):
                continue
        terminal = type(snapshot.get("retained_bytes")) is int and snapshot["retained_bytes"] == 0
        if previous and previous["uploads"] == current:
            terminal |= previous["zero_storage_observed"]
        budgets[budget] = {"id": budget, "uploads": current, "zero_storage_observed": terminal}
    return list(budgets.values()), unknown


def capture_complete(capture):
    if capture is None:
        return False
    summary = capture.get("summary", {})
    health = summary.get("diagnostics", {})
    return (summary.get("duration_limit") is False and health.get("enabled") is True
            and all(type(health.get(k)) is int and health[k] == 0
                    for k in ("queued", "dropped_full", "clock_failures"))
            and all(type(value) is int for value in (health.get("written"), health.get("drained"),
                    summary.get("events"), capture.get("observed_events")))
            and health["written"] == health.get("drained") == summary.get("events")
            == capture.get("observed_events"))


def payload(row):
    value = row["payload"]
    if value == "Resident":
        return "resident", 0, None
    if not isinstance(value, dict) or len(value) != 1:
        raise ValueError("invalid source payload")
    kind, value = next(iter(value.items()))
    if kind == "Hap":
        return "hap", integer(value["bytes"], "HAP bytes", 1), None
    if kind != "Software":
        raise ValueError("unknown source payload")
    size = integer(value["bytes"], "RGBA bytes", 1)
    pitch = integer(value["row_pitch"], "RGBA pitch", 1)
    rows = integer(value["rows"], "RGBA rows", 1)
    extent = row["descriptor"]["extent"]
    if pitch != extent["width"] * 4 or rows != extent["height"] or size != pitch * rows:
        raise ValueError("source RGBA payload disagrees with its immutable descriptor")
    integer(value["sequence"], "source upload sequence", 1)
    return "software", size, value


def analyze_source_submissions(events, bindings, snapshots, captures):
    sources, submissions, upload_ids, renderer_ids = {}, {}, {}, {}

    def source(stream):
        if not isinstance(stream, str) or not stream:
            raise ValueError("missing source stream identity")
        return sources.setdefault(stream, {
            "stream": stream, "binding": None, "submissions": 0,
            "software": {"calls": 0, "bytes": 0}, "hap": {"calls": 0, "bytes": 0},
            "resident_submissions": 0, "render_uploads": {"calls": 0, "bytes": 0},
            "unknown_frame_submissions": 0, "unclocked_submissions": 0,
            "renderer_uses": 0, "_versions": set(), "_pts": set(), "_epochs": set(),
            "_serials": set(), "_device": None, "_descriptor": None, "_render_pts": set(), "_windows": set(),
        })

    for binding in bindings:
        for field in ("stream", "owner", "file"):
            if not isinstance(binding.get(field), str) or not binding[field]:
                raise ValueError("invalid File source binding")
        if not all(isinstance(binding.get("reference", {}).get(k), str) and binding["reference"][k]
                   for k in ("media", "channel")):
            raise ValueError("invalid File channel binding")
        item = source(binding["stream"])
        if item["binding"] is not None and item["binding"] != binding:
            raise ValueError("one source stream changed its channel/File/worker binding")
        item["binding"] = binding

    observed = {k: {"calls": 0, "bytes": 0} for k in ("software", "hap")}
    unknown_cpu_frames = 0
    for row in events.get("SourceSubmission", []):
        stream, token = row["stream"], row["token"]
        serial = integer(token["serial"], "source token serial", 1)
        key = (stream, identity(token))
        if key in submissions:
            if submissions[key] != row:
                raise ValueError("source submission identity changed across captures")
            continue
        submissions[key] = row
        item = source(stream)
        device = identity(token["device"])
        if item["_device"] is not None and item["_device"] != device:
            raise ValueError("source producer device changed without a new stream")
        item["_device"] = device
        item["_serials"].add(serial)
        descriptor = row["descriptor"]
        for field in ("width", "height"):
            integer(descriptor["extent"][field], "source extent", 1, (1 << 32) - 1)
        if item["_descriptor"] is not None and item["_descriptor"] != descriptor:
            raise ValueError("source lifetime changed its immutable descriptor")
        item["_descriptor"] = descriptor
        kind, size, upload = payload(row)
        if upload is not None:
            upload_key = (upload["stream"], upload["sequence"])
            if upload_key in upload_ids:
                raise ValueError("one upload identity belongs to multiple source submissions")
            upload_ids[upload_key] = row
        item["submissions"] += 1
        if kind == "resident":
            item["resident_submissions"] += 1
        else:
            for target in (item[kind], observed[kind]):
                target["calls"] += 1
                target["bytes"] += size
        frame = row.get("frame")
        if frame is None:
            item["unknown_frame_submissions"] += 1
            unknown_cpu_frames += kind != "resident"
        else:
            for field in ("pts_nanos", "start_nanos"):
                integer(frame[field], field, -(1 << 63), (1 << 63) - 1)
            if frame["end_nanos"] is not None:
                integer(frame["end_nanos"], "end nanos", -(1 << 63), (1 << 63) - 1)
                if frame["end_nanos"] <= frame["start_nanos"]:
                    raise ValueError("empty submitted PTS window")
            integer(frame["epoch_generation"], "epoch generation")
            if not isinstance(frame["epoch_authority"], str) or not frame["epoch_authority"]:
                raise ValueError("missing epoch authority")
            item["_pts"].add(frame["pts_nanos"])
            item["_epochs"].add((frame["epoch_authority"], frame["epoch_generation"], frame["pts_nanos"]))
            item["_windows"].add((frame["epoch_authority"], frame["epoch_generation"], frame["pts_nanos"],
                                  frame["start_nanos"], frame["end_nanos"]))
        if row.get("observed_qpc") is None:
            item["unclocked_submissions"] += 1
        else:
            integer(row["observed_qpc"], "submission QPC")

    missing_witnesses, witnesses, image_pts = set(), {}, {}
    for row in events.get("SourceUse", []):
        if row.get("stream") is None or not row.get("sequence"):
            continue
        item = source(row["stream"])
        item["renderer_uses"] += 1
        item["_versions"].add(row["sequence"])
        if row.get("frame") is not None:
            pts = integer(row["frame"]["pts_nanos"], "renderer PTS", -(1 << 63), (1 << 63) - 1)
            image_key = (row["stream"], row["sequence"])
            if image_key in image_pts and image_pts[image_key] != pts:
                raise ValueError("one retained source version changed its original PTS")
            image_pts[image_key] = pts
            item["_render_pts"].add(pts)
        if row.get("descriptor") is not None:
            if item["_descriptor"] is not None and row["descriptor"] != item["_descriptor"]:
                raise ValueError("renderer source descriptor disagrees with submission")
            item["_descriptor"] = row["descriptor"]
        upload = row.get("software_upload")
        if upload is not None:
            key = (upload["stream"], upload["sequence"])
            if key in witnesses and witnesses[key] != upload:
                raise ValueError("renderer upload witness changed across consumers")
            witnesses[key] = upload
            submitted = upload_ids.get(key)
            if submitted is None:
                missing_witnesses.add(key)
            elif submitted["payload"]["Software"] != upload or submitted["stream"] != row["stream"]:
                raise ValueError("renderer upload witness disagrees with source submission")
            elif submitted.get("frame") and row.get("frame"):
                if submitted["frame"]["pts_nanos"] != row["frame"]["pts_nanos"]:
                    raise ValueError("renderer upload witness changed its decoded PTS")

    unknown_render_pts = 0
    for row in events.get("ImageUpload", []):
        key = (identity(row["token"]), row["texture"], row["stream"], row["sequence"])
        if key in renderer_ids:
            if renderer_ids[key] != row:
                raise ValueError("renderer upload identity changed across captures")
            continue
        renderer_ids[key] = row
        size = integer(row["bytes"], "renderer upload bytes", 1)
        if size != integer(row["row_pitch"], "renderer pitch", 1) * integer(row["rows"], "renderer rows", 1):
            raise ValueError("renderer upload byte layout disagrees")
        item = source(row["stream"])["render_uploads"]
        item["calls"] += 1
        item["bytes"] += size
        unknown_render_pts += (row["stream"], row["sequence"]) not in image_pts

    budgets, unmeasured = inventory(snapshots)
    expected = {k: {f: sum(b["uploads"][k][f] for b in budgets) for f in ("calls", "bytes")}
                for k in observed} if budgets and not unmeasured else None
    reasons = []
    if not captures or not all(capture_complete(c) for c in captures):
        reasons.append("incomplete_capture_health")
    if unmeasured or not budgets:
        reasons.append("unmeasured_or_overflowed_source_budget")
    if len(budgets) > 1:
        reasons.append("multiple_budgets_without_per_submission_budget_identity")
    if budgets and not all(b["zero_storage_observed"] for b in budgets):
        reasons.append("source_storage_not_retired_at_latest_counters")
    if any(s["binding"] is None and s["submissions"] for s in sources.values()):
        reasons.append("unbound_source_submissions")
    if any(s["_serials"] and max(s["_serials"]) != len(s["_serials"]) for s in sources.values()):
        reasons.append("missing_source_submission_serials")
    if missing_witnesses:
        reasons.append("renderer_witnessed_uploads_missing_source_submissions")
    if not any(s["submissions"] or s["render_uploads"]["calls"] for s in sources.values()):
        reasons.append("no_source_upload_or_conversion_facts")
    if any(s["renderer_uses"] and not s["submissions"] and not s["render_uploads"]["calls"]
           for s in sources.values()):
        reasons.append("renderer_sources_without_submission_or_upload_facts")
    missing = None
    if expected is not None:
        missing = {k: {f: expected[k][f] - observed[k][f] for f in ("calls", "bytes")}
                   for k in observed}
        if not unmeasured and any(v < 0 for k in missing.values() for v in k.values()):
            raise ValueError("source submissions exceed actual lifetime budget counters")
        if any(v != 0 for k in missing.values() for v in k.values()):
            reasons.append("source_payload_totals_do_not_reconcile")
    clean_health = bool(captures) and all(capture_complete(c) for c in captures)
    render_bound = all(source(row["stream"])["binding"] is not None for row in renderer_ids.values())
    for item in sources.values():
        known = item["submissions"] - item["unknown_frame_submissions"]
        item["distinct_observed_pts"] = len(item.pop("_pts"))
        item["repeated_pts_submissions"] = known - item["distinct_observed_pts"]
        item["distinct_observed_epoch_pts"] = len(item.pop("_epochs"))
        item["duplicate_submitted_windows"] = known - len(item.pop("_windows"))
        item["rendered_publication_versions"] = len(item.pop("_versions"))
        item["distinct_rendered_pts"] = len(item.pop("_render_pts"))
        for field in ("_serials", "_device", "_descriptor"):
            item.pop(field)
    channels = {}
    for item in sources.values():
        binding = item["binding"]
        if binding is None:
            continue
        reference = binding["reference"]
        key = (reference["media"], reference["channel"])
        channel = channels.setdefault(key, {"reference": reference, "streams": [], "files": [],
            "submissions": 0, "software": {"calls": 0, "bytes": 0}, "hap": {"calls": 0, "bytes": 0},
            "render_uploads": {"calls": 0, "bytes": 0}, "rendered_publication_versions": 0,
            "duplicate_submitted_windows": 0})
        channel["streams"].append(item["stream"])
        if binding["file"] not in channel["files"]:
            channel["files"].append(binding["file"])
        for field in ("submissions", "rendered_publication_versions", "duplicate_submitted_windows"):
            channel[field] += item[field]
        for kind in ("software", "hap", "render_uploads"):
            for field in ("calls", "bytes"):
                channel[kind][field] += item[kind][field]
    for channel in channels.values():
        channel["streams"].sort()
        channel["files"].sort()
    return {
        "source_worker_cpu_uploads": {"observed": observed, "measured_lifetime_totals": expected,
            "missing": missing, "budget_lifetimes": len(budgets),
            "complete_cpu_payload_attribution": not reasons,
            "complete_cpu_payload_pts_attribution": not reasons and unknown_cpu_frames == 0,
            "unknown_cpu_pts_rows": unknown_cpu_frames, "unknown_reasons": reasons},
        "render_thread_uploads": {"calls": len(renderer_ids),
            "bytes": sum(row["bytes"] for row in renderer_ids.values()),
            "complete_channel_attribution": clean_health and render_bound,
            "unknown_pts_upload_calls": unknown_render_pts,
            "complete_pts_attribution": clean_health and render_bound and unknown_render_pts == 0},
        "sources": sorted(sources.values(), key=lambda row: row["stream"]),
        "channels": [channels[key] for key in sorted(channels)],
        "scope": "Actual submitted CPU payload, not decoder work, GPU completion/publication, unique presented "
                 "frames or scanout. Repeated PTS can be intentional seeks/loops. Resident conversion counts "
                 "are observed facts without independent lifetime counters. Missing clocks cannot establish phase rates."
    }


def require_complete_attribution(result):
    source = result["source_worker_cpu_uploads"]
    if (not source["complete_cpu_payload_pts_attribution"]
            or not result["render_thread_uploads"]["complete_pts_attribution"]
            or any(s["unknown_frame_submissions"] for s in result["sources"])):
        raise ValueError("complete source upload/PTS/channel attribution unavailable: "
                         + ", ".join(source["unknown_reasons"])
                         + f"; unknown_cpu_pts={source['unknown_cpu_pts_rows']}"
                         + f"; render_attribution={result['render_thread_uploads']}")

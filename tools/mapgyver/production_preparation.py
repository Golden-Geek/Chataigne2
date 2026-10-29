"""Native preparation and first-use joins, without inventing publication or display age."""

import json


def _uint(value, label, minimum=0):
    if type(value) is not int or not minimum <= value < (1 << 64):
        raise ValueError(f"invalid {label}")
    return value


def _generation(info):
    device = info["device"]
    project = info["identity"]["project"]
    if not isinstance(device["runtime"], str) or not device["runtime"] \
            or project["runtime"] != device["runtime"]:
        raise ValueError("foreign preparation project/device authority")
    _uint(device["generation"], "preparation device generation")
    _uint(project["generation"], "preparation project generation")
    _uint(info["identity"]["revision"], "preparation revision", 1)
    _uint(info["bytes"], "preparation reservation")
    _uint(info["output_count"], "preparation output count")
    return json.dumps(device, sort_keys=True), _uint(info["generation"], "resource generation", 1)


def _wall(value):
    if value is None:
        return None
    begin = _uint(value["begin"], "preparation begin")
    end = _uint(value["end"], "preparation end", begin)
    return begin, end


def analyze_preparation(preparations, frames, frequency, distribution):
    _uint(frequency, "QPC frequency", 1)
    ready, records = {}, []
    created = reused = failed = 0
    for row in preparations:
        key = _generation(row["generation"])
        if type(row["succeeded"]) is not bool:
            raise ValueError("invalid preparation outcome")
        wall = _wall(row["wall"])
        created += _uint(row["created_textures"], "created textures")
        reused += _uint(row["reused_textures"], "reused textures")
        _uint(row["cache_entries"], "weak cache entries")
        record = {**row, "first_submit_wait_ms": None, "submitted_frames": 0}
        records.append(record)
        if row["succeeded"]:
            if key in ready:
                raise ValueError("duplicate successful resource generation")
            ready[key] = record
        else:
            failed += 1
    missing, first = 0, {}
    for frame in frames:
        info = frame["generation"]
        key = _generation(info)
        if info["device"] != frame["token"]["device"]:
            raise ValueError("frame uses foreign resource device")
        record = ready.get(key)
        if record is None:
            missing += 1
            continue
        if info != record["generation"]:
            raise ValueError("frame resource identity/reservation differs from preparation")
        record["submitted_frames"] += 1
        wall = _wall(frame.get("wall"))
        if wall:
            first[key] = min(first.get(key, wall[0]), wall[0])
    for key, begin in first.items():
        record = ready[key]
        prepared = _wall(record["wall"])
        if prepared:
            if begin < prepared[1]:
                raise ValueError("frame submitted before resource preparation completed")
            record["first_submit_wait_ms"] = (begin - prepared[1]) * 1000 / frequency
    complete = bool(preparations) and missing == 0 and all(
        record["wall"] is not None and (record["submitted_frames"] == 0
                                       or record["first_submit_wait_ms"] is not None)
        for record in records
    ) and all(frame.get("wall") is not None for frame in frames)
    return {
        "coverage_complete": complete,
        "attempts": len(records), "failed_attempts": failed,
        "created_textures": created, "reused_textures": reused,
        "unjoined_frame_rows": missing,
        "native_preparation_wall": distribution([
            (record["wall"]["end"] - record["wall"]["begin"]) * 1000 / frequency
            for record in records if record["wall"] is not None
        ]),
        "prepared_to_first_submit_wait": distribution([
            record["first_submit_wait_ms"] for record in records if record["first_submit_wait_ms"] is not None
        ]),
        "records": records,
        "scope": "native preparation and prepared-to-first-submit wall time; publication/receipt and output queue age remain unmeasured",
    }

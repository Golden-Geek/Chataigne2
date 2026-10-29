"""Exact copied-image joins for the fixed Q01/Q02 source pair and Spout output 2.

SDK versions identify images, while GPU marker words independently check their contents.
This measures completed independent captures, never window presentation or GPU queue time.
"""

from collections import defaultdict
import json


def _uint(value, label, minimum=0, maximum=(1 << 64) - 1):
    if type(value) is not int or not minimum <= value <= maximum:
        raise ValueError(f"invalid {label}")
    return value


def _image(value):
    if value is None:
        return None
    return (_uint(value["shared_handle"], "shared-image handle", 1, (1 << 32) - 1),
            _uint(value["frame"], "shared-image counter", 1, (1 << 31) - 1))


def _token(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"))


def _inside(stamp, window):
    return type(stamp) is int and window["begin_qpc"] <= stamp < window["end_qpc"]


def _api(row):
    interval = row.get("consumer_api")
    if interval is None:
        return None
    begin = _uint(interval["begin"], "output API begin", 1)
    return begin, _uint(interval["end"], "output API end", begin)


def _producers(streams):
    images = [defaultdict(list), defaultdict(list)]
    for index, rows in enumerate(streams):
        for row in rows:
            if row["kind"] != "source_frame":
                continue
            submission = row["submission"]
            image = _image(submission.get("spout_image"))
            if image is not None:
                if submission["outcome"] != "Accepted":
                    raise ValueError("rejected producer claims a copied image")
                images[index][image].append(row)
    return images


def _expected_sources(output, uses, producers):
    result = []
    for index in range(2):
        use = uses.get((_token(output["token"]), index + 1))
        publication = use.get("publication") if use else None
        image = _image(publication.get("spout_image")) if publication else None
        rows = producers[index].get(image, [])
        if not use or use.get("sequence", 0) <= 0 or len(rows) != 1:
            return None
        row = rows[0]
        stamp = row["submission"]["stamp"]
        if stamp["source"] != index + 1:
            raise ValueError("shared-image producer belongs to another card source")
        result.append(row)
    return result


def _capture(frame, output, expected, receive):
    api = _api(output)
    completed = frame.get("completion_observed_qpc")
    if api is None or type(completed) is not int or completed < api[0]:
        return "missing or future output/capture timing"
    if receive is None or receive.get("receive_api") is None:
        return "missing exact consumer receive interval"
    interval = receive["receive_api"]
    begin = _uint(interval["begin"], "consumer receive begin", 1)
    end = _uint(interval["end"], "consumer receive end", begin)
    if begin != frame.get("receive_begin_qpc") or end < api[0] or completed < end:
        return "consumer receive interval does not overlap/follow the copied output"
    if expected is None:
        return "missing or ambiguous exact producer/source-use image"
    raw = frame.get("raw_words")
    if not isinstance(raw, list) or len(raw) != 12:
        return "invalid raw GPU marker width"
    for word in raw:
        _uint(word, "raw GPU marker word", maximum=(1 << 32) - 1)
    for index, name in enumerate(("left", "right")):
        producer = expected[index]
        stamp = producer["submission"]["stamp"]
        _uint(stamp["qpc"], "producer QPC", 1)
        if frame.get(name) != {"Ok": stamp} or raw[index * 6:(index + 1) * 6] != producer.get("marker_words"):
            return "GPU marker differs from the exact renderer source image"
        if type(stamp["qpc"]) is not int or completed < stamp["qpc"]:
            return "invalid producer-to-capture clock order"
    return None


def publication_age(outputs, uses, window, frequency, distribution):
    """CPU publication/receive to output API boundaries, excluding GPU/display queue claims."""
    reports = []
    for output_id in (1, 2, 3):
        selected = [row for row in outputs if row["output"] == output_id and row["outcome"] == "Accepted"
                    and _inside((row.get("delivery") or {}).get("end"), window)]
        sources = []
        for texture in (1, 2):
            retained, received, missing = [], [], 0
            for output in selected:
                use = uses.get((_token(output["token"]), texture))
                publication = use.get("publication") if use else None
                api = _api(output)
                if not publication or use.get("sequence", 0) <= 0 or api is None \
                        or any(publication.get(name) is None for name in ("publication_qpc", "receive_begin_qpc")):
                    missing += 1
                    continue
                begin = _uint(publication["receive_begin_qpc"], "source receive QPC", 1)
                published = _uint(publication["publication_qpc"], "source publication QPC", begin)
                if api[0] < published:
                    raise ValueError("output API precedes its retained source publication")
                retained.append((api[1] - published) * 1000 / frequency)
                received.append((api[1] - begin) * 1000 / frequency)
            sources.append({"texture": texture, "expected_output_calls": len(selected), "missing": missing,
                            "publication_to_api_end": distribution(retained),
                            "source_receive_begin_to_api_end": distribution(received)})
        reports.append({"output": output_id, "sources": sources,
                        "coverage_complete": bool(selected) and all(source["missing"] == 0 for source in sources)})
    return {"outputs": reports, "coverage_complete": all(row["coverage_complete"] for row in reports),
            "scope": "retained Q01/Q02 frame sources to accepted output CPU API end; not GPU/presentation queue age"}


def analyze_cohort(native, streams, consumer, consumer_events, window, frequency, distribution):
    _uint(frequency, "QPC frequency", 1)
    uses = {(_token(row["token"]), row["texture"]): row for row in native["SourceUse"]}
    producers = _producers(streams)
    publications = defaultdict(list)
    selected = []
    for output in native["Output"]:
        image = _image(output.get("spout_image"))
        if image is not None and (output["outcome"] != "Accepted" or output["output"] != 2):
            raise ValueError("unexpected renderer shared-image publication")
        if output["output"] == 2 and output["outcome"] == "Accepted":
            if image is not None:
                publications[image].append(output)
            if _inside((output.get("delivery") or {}).get("end"), window):
                selected.append(output)
    captures = defaultdict(list)
    for row in consumer:
        if row["kind"] == "consumer_frame":
            image = _image(row["frame"].get("spout_image"))
            if image is not None:
                captures[image].append(row["frame"])
    receive = {_token(row["token"]): row for row in consumer_events["SourceCpu"]}
    records, invalid, ages, capture_delay = [], [], [[], []], [[], []]
    repeated = 0
    for output in selected:
        image = _image(output.get("spout_image"))
        record = {"token": output["token"], "spout_image": output.get("spout_image"),
                  "capture_row": None, "source_frames": None, "source_ages_ms": None,
                  "sender_api_capture_delay_ms": None, "missing": None}
        records.append(record)
        if image is None or len(publications[image]) != 1:
            record["missing"] = "missing or ambiguous output image version"
            continue
        expected = _expected_sources(output, uses, producers)
        candidates = []
        for frame in captures[image]:
            reason = _capture(frame, output, expected, receive.get(_token(frame["token"])))
            if reason:
                invalid.append({"output_token": output["token"], "capture_row": frame["row"], "reason": reason})
            else:
                candidates.append(frame)
        if not candidates:
            record["missing"] = "no valid exact independent capture"
            continue
        repeated += len(candidates) - 1
        first = min(candidates, key=lambda row: row["completion_observed_qpc"])
        completed = first["completion_observed_qpc"]
        api = _api(output)
        record["capture_row"] = first["row"]
        record["source_frames"] = [row["submission"]["stamp"]["frame"] for row in expected]
        record["source_ages_ms"] = [(completed - row["submission"]["stamp"]["qpc"]) * 1000 / frequency
                                    for row in expected]
        for index, age in enumerate(record["source_ages_ms"]):
            ages[index].append(age)
        # Image copy/version reservation lies somewhere within the sender API interval.
        lower = max(0, completed - api[1]) * 1000 / frequency
        upper = (completed - api[0]) * 1000 / frequency
        record["sender_api_capture_delay_ms"] = {"lower_bound": lower, "upper_bound": upper}
        capture_delay[0].append(lower)
        capture_delay[1].append(upper)
    missing = sum(record["missing"] is not None for record in records)
    complete = bool(records) and missing == 0 and not invalid
    age = [distribution(values) for values in ages]
    return {
        "output": 2, "expected_accepted_images": len(records), "captured_images": len(records) - missing,
        "missing_images": missing, "repeated_captures": repeated, "invalid_captures": invalid,
        "coverage_complete": complete, "source_age": age,
        "observed_age_p95_within_budget": all(values is not None and values["p95_ms"] <= 50 for values in age),
        "sender_api_to_capture_completion": {"lower_bound": distribution(capture_delay[0]),
                                              "upper_bound": distribution(capture_delay[1])},
        "records": records,
        "source_publication_age": publication_age(native["Output"], uses, window, frequency, distribution),
        "scope": "every sample-accepted Spout image to its first valid completed capture, including later captures; missing images stay missing",
        "limitations": ["legacy handle/counter reuse is ambiguous and cannot be guessed",
                        "CPU API brackets bound copy admission; they are not calibrated GPU queue times",
                        "native windows, physical presentation and complete GPU critical path remain unobserved"],
    }

"""CPU plan handoff/install/first-use observations; no GPU or presentation queue claim."""

import json


def _key(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"))


def _uint(value, label, minimum=0):
    if type(value) is not int or not minimum <= value < (1 << 64):
        raise ValueError(f"invalid {label}")
    return value


def _identity(identity):
    project = identity["project"]
    if not isinstance(project["runtime"], str) or not project["runtime"]:
        raise ValueError("missing plan runtime authority")
    _uint(project["generation"], "plan project generation")
    _uint(identity["revision"], "plan revision", 1)
    return _key(identity)


def _generation(info):
    if info["device"]["runtime"] != info["identity"]["project"]["runtime"]:
        raise ValueError("foreign installed plan/device authority")
    _identity(info["identity"])
    _uint(info["device"]["generation"], "installed device generation")
    _uint(info["generation"], "installed resource generation", 1)
    return _key(info["device"]), info["generation"]


def _stamp(value):
    return None if value is None else _uint(value, "plan QPC", 1)


def _delay(begin, end, frequency):
    if begin is None or end is None:
        return None
    if end < begin:
        raise ValueError("plan lifecycle clock order is reversed")
    return (end - begin) * 1000 / frequency


def analyze_plan_age(transitions, preparations, frames, outputs, window, frequency, distribution):
    _uint(frequency, "QPC frequency", 1)
    plans, installed = {}, {}

    def plan_for(identity):
        return plans.setdefault(_identity(identity), {"identity": identity, "events": {}, "installations": []})

    for row in transitions:
        plan = plan_for(row["identity"])
        stage, stamp = row["stage"], _stamp(row["observed_qpc"])
        if isinstance(stage, dict) and set(stage) == {"Installed"}:
            generation = stage["Installed"]["generation"]
            if generation["identity"] != row["identity"]:
                raise ValueError("installation differs from authored plan identity")
            key = _generation(generation)
            if key in installed:
                raise ValueError("duplicate installed resource generation")
            record = {"generation": generation, "installed_qpc": stamp, "first_submit_qpc": None,
                      "submitted_frames": 0, "publication_to_first_submit_ms": None,
                      "install_to_first_submit_ms": None, "receipt_to_native_preparation_ms": None}
            installed[key] = record
            plan["installations"].append(record)
            continue
        if isinstance(stage, dict) and set(stage) == {"Rejected"}:
            if not isinstance(stage["Rejected"].get("code"), str):
                raise ValueError("missing typed plan rejection")
            stage = "Rejected"
        if not isinstance(stage, str) or stage not in ("Published", "Received", "Superseded", "Rejected", "Cancelled"):
            raise ValueError("unknown plan lifecycle stage")
        if stage in plan["events"]:
            raise ValueError("duplicate plan lifecycle observation")
        plan["events"][stage] = stamp

    prepared = {_generation(row["generation"]): row for row in preparations if row["succeeded"]}
    frame_by_token, unjoined_frames = {}, 0
    for frame in frames:
        key = _generation(frame["generation"])
        if frame["token"]["device"] != frame["generation"]["device"]:
            raise ValueError("frame uses foreign installed device")
        record = installed.get(key)
        if record is None:
            unjoined_frames += 1
            continue
        if record["generation"] != frame["generation"]:
            raise ValueError("frame differs from exact installed resource identity")
        wall = frame.get("wall")
        begin = _stamp(wall["begin"]) if wall else None
        _delay(record["installed_qpc"], begin, frequency)
        record["submitted_frames"] += 1
        if begin is not None:
            record["first_submit_qpc"] = min(record["first_submit_qpc"] or begin, begin)
        frame_by_token[_key(frame["token"])] = frame

    missing = unjoined_frames
    records = []
    for plan in plans.values():
        events = plan["events"]
        published, received = events.get("Published"), events.get("Received")
        terminals = [stage for stage in ("Superseded", "Rejected", "Cancelled") if stage in events]
        if len(terminals) > 1 or (terminals and plan["installations"]):
            raise ValueError("conflicting terminal plan observations")
        missing += int("Published" not in events or published is None)
        missing += int(not terminals and not plan["installations"])
        missing += int(any(stamp is None for stamp in events.values()))
        _delay(published, received, frequency)
        for terminal in terminals:
            _delay(received if "Received" in events else published, events[terminal], frequency)
        for record in plan["installations"]:
            missing += int("Received" not in events or received is None or record["installed_qpc"] is None)
            _delay(received, record["installed_qpc"], frequency)
            preparation = prepared.get(_generation(record["generation"]))
            if preparation is None or preparation.get("wall") is None:
                missing += 1
            else:
                if preparation["generation"] != record["generation"]:
                    raise ValueError("installation differs from resource preparation")
                _delay(preparation["wall"]["end"], record["installed_qpc"], frequency)
                record["receipt_to_native_preparation_ms"] = _delay(received, preparation["wall"]["begin"], frequency)
            record["publication_to_first_submit_ms"] = _delay(published, record["first_submit_qpc"], frequency)
            record["install_to_first_submit_ms"] = _delay(record["installed_qpc"], record["first_submit_qpc"], frequency)
            missing += int(record["submitted_frames"] > 0 and record["first_submit_qpc"] is None)
        records.append({"identity": plan["identity"], "events": events,
                        "terminal": terminals[0] if terminals else "Installed" if plan["installations"] else None,
                        "publication_to_receipt_ms": _delay(published, received, frequency),
                        "installations": plan["installations"]})

    output_reports, missing_outputs = [], 0
    for output_id in (1, 2, 3):
        selected = [o for o in outputs if o["output"] == output_id and o["outcome"] == "Accepted"
                    and window["begin_qpc"] <= (o.get("delivery") or {}).get("end", -1) < window["end_qpc"]]
        ages, absent = [], 0
        for output in selected:
            frame = frame_by_token.get(_key(output["token"]))
            identity = frame["generation"]["identity"] if frame else None
            plan = plans.get(_identity(identity)) if identity else None
            published = plan["events"].get("Published") if plan else None
            api = output.get("consumer_api")
            wall = frame.get("wall") if frame else None
            if published is None or api is None or wall is None:
                absent += 1
                continue
            begin, end = _stamp(api["begin"]), _stamp(api["end"])
            _delay(_stamp(wall["begin"]), begin, frequency)
            _delay(begin, end, frequency)
            _delay(end, _stamp(wall["end"]), frequency)
            ages.append(_delay(published, end, frequency))
        missing_outputs += absent
        output_reports.append({"output": output_id, "expected_accepted_calls": len(selected),
                               "missing": absent, "publication_to_api_end": distribution(ages)})
    first_uses = [i["publication_to_first_submit_ms"] for p in records for i in p["installations"]
                  if i["publication_to_first_submit_ms"] is not None]
    return {"coverage_complete": bool(transitions) and missing == 0 and missing_outputs == 0,
            "accepted_publications": sum("Published" in p["events"] for p in records),
            "installed_generations": len(installed), "missing_lifecycle_observations": missing,
            "unjoined_frame_rows": unjoined_frames, "unjoined_output_calls": missing_outputs,
            "publication_to_receipt": distribution([p["publication_to_receipt_ms"] for p in records
                                                     if p["publication_to_receipt_ms"] is not None]),
            "publication_to_first_submit": distribution(first_uses),
            "outputs": output_reports, "records": records,
            "scope": "CPU admitted publication to first worker receipt, atomic install, first submit and accepted output API; retained plan age is not edit latency",
            "limitations": ["superseded/unread plans have no invented receipt or first use",
                            "resource recreation retains the original publication identity/age",
                            "GPU queue, physical presentation and complete critical-path time remain unmeasured"]}

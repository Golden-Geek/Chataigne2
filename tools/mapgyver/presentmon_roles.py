"""Match software PresentMon rows to owned diagnostic swapchains by address and QPC lifetime."""

from bisect import bisect_right
import csv
import hashlib
import io
from pathlib import Path
import runpy

DATA = runpy.run_path(str(Path(__file__).with_name("presentmon_data.py")))
_INTERFACES = ("idxgi_swap_chain", "idxgi_swap_chain1", "idxgi_swap_chain2")
_ROLES = {"editor", "native_output"}
_MAX_U64 = (1 << 64) - 1


def _require(condition, message):
    if not condition:
        raise ValueError(message)


def _uint(value, label, maximum=_MAX_U64, zero=False):
    _require(type(value) is int and (0 if zero else 1) <= value <= maximum,
             f"{label} must be an in-range integer")
    return value


def _address(value):
    _require(isinstance(value, str), "native swapchain address must be a string")
    match = DATA["_ADDRESS"].fullmatch(value)
    _require(match is not None and len(match[1]) <= 16 and 0 < int(match[1], 16) <= _MAX_U64,
             "invalid native swapchain address")
    return hex(int(match[1], 16))


def _records(native_report, expected_pid):
    _require(isinstance(native_report, dict), "native report is missing")
    _require(_uint(native_report.get("render_process_id"), "native PID", 0xffffffff) == expected_pid,
             "native report PID mismatch")
    generations = native_report.get("generations")
    _require(isinstance(generations, list) and generations, "native generations are missing")
    seen = set()
    records = []
    for generation in generations:
        _require(isinstance(generation, dict), "malformed native generation")
        identity = _uint(generation.get("generation"), "generation", 0xffffffff, zero=True)
        _require(identity not in seen, "duplicate native generation")
        seen.add(identity)
        chains = generation.get("swapchains")
        _require(isinstance(chains, list) and len(chains) == 2,
                 "each native generation requires exactly two swapchains")
        roles = set()
        for chain in chains:
            _require(isinstance(chain, dict), "malformed native swapchain")
            role = chain.get("role")
            _require(isinstance(role, str) and role in _ROLES and role not in roles,
                     "each native generation requires unique editor and native_output roles")
            roles.add(role)
            begin = _uint(chain.get("begin_qpc"), "swapchain begin QPC")
            end = _uint(chain.get("end_qpc"), "swapchain end QPC")
            _require(begin < end, "native swapchain lifetime must be nonempty")
            addresses = chain.get("addresses")
            _require(isinstance(addresses, dict) and set(addresses) == set(_INTERFACES),
                     "native swapchain requires all three DXGI interface addresses")
            records.append(dict(generation=identity, role=role, begin_qpc=begin, end_qpc=end,
                                addresses={key: _address(addresses[key]) for key in _INTERFACES}))
    return records


def _index(records):
    by_address = {}
    for index, record in enumerate(records):
        # QueryInterface aliases may legitimately share the same raw pointer in one object.
        for address in set(record["addresses"].values()):
            by_address.setdefault(address, []).append(index)
    result = {}
    for address, indices in by_address.items():
        indices.sort(key=lambda index: records[index]["begin_qpc"])
        for previous, current in zip(indices, indices[1:]):
            _require(records[previous]["end_qpc"] <= records[current]["begin_qpc"],
                     f"overlapping native swapchain aliases at {address}")
        result[address] = ([records[index]["begin_qpc"] for index in indices], indices)
    return result


def _csv(headers, rows):
    output = io.StringIO(newline="")
    writer = csv.writer(output)
    writer.writerow(headers)
    writer.writerows(row.raw for row in rows)
    return output.getvalue()


def correlate(csv_text, expected_pid, native_report, config):
    """Require every row and declared presenter to have an exact diagnostic identity match.

    Callers retain the unchanged CSV and native report. COM/ETW pointer equality is tested
    against those rows, never assumed from interface version, address order or row counts.
    """
    _uint(expected_pid, "expected PID", 0xffffffff)
    begin, end, frequency = DATA["_window"](config)
    headers, groups, count = DATA["_read"](csv_text, expected_pid, frequency)
    records = _records(native_report, expected_pid)
    by_address = _index(records)
    matched = [{} for _ in records]
    for address, rows in groups.items():
        _require(address in by_address, f"CSV swapchain {address} has no native identity")
        starts, indices = by_address[address]
        for row in rows:
            position = bisect_right(starts, row.qpc) - 1
            _require(position >= 0, f"CSV swapchain {address} is outside its native lifetime")
            index = indices[position]
            _require(row.qpc < records[index]["end_qpc"],
                     f"CSV swapchain {address} is outside its native lifetime")
            matched[index].setdefault(address, []).append(row)
    roles = []
    for record, observed in zip(records, matched):
        _require(observed, f"native {record['role']} generation {record['generation']} has no captured rows")
        chains = []
        for address, rows in observed.items():
            # Preserve predecessor/window semantics and keep distinct CSV addresses separate.
            summary = DATA["analyze"](_csv(headers, rows), expected_pid, config)["chains"][0]
            summary.update(label=record["role"], generation=record["generation"],
                           matched_interface_keys=[key for key in _INTERFACES
                                                   if record["addresses"][key] == address])
            chains.append(summary)
        roles.append(dict(**record, raw_rows=sum(len(rows) for rows in observed.values()), chains=chains))
    return dict(schema_version=1,
                evidence_kind="presentmon_v1_exact_native_role_correlation_not_qualification",
                scope="Owned PID/address/QPC-lifetime correlation only; no independent physical cycles, scanout, optical latency or full M01 claim",
                expected_pid=expected_pid, raw_rows=count, native_records=len(records),
                csv_text_utf8_sha256=hashlib.sha256(csv_text.encode("utf-8")).hexdigest(),
                sample_present_window_qpc=dict(begin=begin, end=end, frequency=frequency),
                all_rows_correlated=True, all_native_records_observed=True, roles=roles)

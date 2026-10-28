"""Validate the run-owned clock and the consumer's pre-sample observation fence."""

OPERATIONS = (
    "clears", "shader_draws", "uniform_upload_bytes", "diagnostic_readback_pixels",
    "sdk_send_calls", "sdk_receive_calls", "sdk_texture_queries",
    "sdk_texture_observations", "explicit_gpu_copy_calls", "explicit_gpu_copy_bytes",
)
_WINDOW_FIELDS = {"start_qpc", "frequency", "warmup_seconds", "maximum_seconds"}
_PHASES = ("initial", "resized", "source_absent", "recovered")
_MARKERS = ("completion_markers", "skipped_completion_markers")
_U64_MAX = (1 << 64) - 1
_U32_MAX = (1 << 32) - 1


def _require(condition, message):
    if not condition:
        raise ValueError(message)


def _uint(value, label, maximum=_U64_MAX):
    _require(type(value) is int and 0 <= value <= maximum,
             f"{label} must be an exact unsigned integer within its native range")
    return value


def _window(value, label):
    _require(isinstance(value, dict) and set(value) == _WINDOW_FIELDS,
             f"{label} fields differ from the native sample-window schema")
    start = _uint(value["start_qpc"], f"{label}.start_qpc")
    frequency = _uint(value["frequency"], f"{label}.frequency")
    warmup = _uint(value["warmup_seconds"], f"{label}.warmup_seconds", _U32_MAX)
    maximum = _uint(value["maximum_seconds"], f"{label}.maximum_seconds", _U32_MAX)
    _require(start > 0 and frequency > 0 and 0 <= warmup < maximum <= 3600,
             f"{label} clock or duration is invalid")
    _require(start + maximum * frequency <= _U64_MAX,
             f"{label} QPC end overflows u64")
    return dict(value)


def _operations(value, label):
    _require(isinstance(value, dict) and set(value) == set(OPERATIONS),
             f"{label} fields differ from the native operations schema")
    for name in OPERATIONS:
        _uint(value[name], f"{label}.{name}")
    return dict(value)


def _markers(value, label):
    return {name: _uint(value.get(name), f"{label}.{name}", _U32_MAX) for name in _MARKERS}


def validate(native_report, requested_mode: str) -> dict:
    """Require a fenced observer before the controlled 30-second warmup ends.

    Active marker contents and distributions are verified by the frame-age analyzer.
    Paused observation adds no counted work or resource markers after its fence.
    Both modes retain observation ownership until the complete sample window has elapsed.
    """
    _require(type(requested_mode) is str and requested_mode in ("active", "paused"),
             "requested consumer mode must be active or paused")
    _require(isinstance(native_report, dict), "native report is malformed")
    window = _window(native_report.get("sample_window"), "sample_window")
    workload = native_report.get("workload")
    _require(isinstance(workload, dict), "native workload is missing")
    _require(workload.get("consumer_mode") == requested_mode,
             "native workload consumer mode differs from the request")
    _require(workload.get("scenario") in ("q01", "q02"),
             "consumer observation requires q01 or q02")
    _require(type(workload.get("gpu_stages")) is bool,
             "native GPU stage selection must be a boolean")
    warmup = _uint(workload.get("warmup_seconds"), "workload.warmup_seconds", _U32_MAX)
    sample = _uint(workload.get("sample_seconds"), "workload.sample_seconds", _U32_MAX)
    _require(warmup == window["warmup_seconds"] == 30 and sample >= 8
             and window["maximum_seconds"] == warmup + sample,
             "workload and sample window must share the controlled 30-second warmup and duration")
    if requested_mode == "paused":
        _require(workload["gpu_stages"] is True and workload.get("presentation_policy") == "latest",
                 "paused observation requires GPU stages and latest presentation")

    observer = native_report.get("observer")
    _require(isinstance(observer, dict), "observer report is missing")
    _require(observer.get("consumer_mode") == requested_mode,
             "observer consumer mode differs from the request")
    ready = observer.get("observation_ready")
    _require(isinstance(ready, dict) and set(ready) == {
        "window", "consumer_mode", "completed_qpc", "operations_at_fence",
        "completion_markers", "skipped_completion_markers",
    }, "observer readiness acknowledgement is missing or malformed")
    _require(_window(ready["window"], "observation_ready.window") == window
             and ready["consumer_mode"] == requested_mode,
             "observer readiness mode or clock differs from the run")
    completed = _uint(ready["completed_qpc"], "observation_ready.completed_qpc")
    received = _uint(observer.get("observation_ready_received_qpc"),
                     "observer.observation_ready_received_qpc")
    sample_begin = window["start_qpc"] + 30 * window["frequency"]
    _require(window["start_qpc"] <= completed <= received < sample_begin,
             "observer readiness must complete and reach the parent before sampling")
    finished = _uint(observer.get("observation_finish_qpc"), "observer.observation_finish_qpc")
    sample_end = window["start_qpc"] + window["maximum_seconds"] * window["frequency"]
    _require(finished >= sample_end and finished >= received,
             "observer observation must finish after the full sample window and readiness receipt")
    checkpoints = observer.get("checkpoints")
    _require(isinstance(checkpoints, list) and len(checkpoints) == len(_PHASES)
             and all(isinstance(checkpoint, dict) and checkpoint.get("phase") == phase
                     for checkpoint, phase in zip(checkpoints, _PHASES, strict=True)),
             "observer requires all four ordered content checkpoints")
    at_fence = _operations(ready["operations_at_fence"], "observation_ready.operations_at_fence")
    resources = observer.get("resources")
    _require(isinstance(resources, dict), "observer resources are missing")
    final = _operations(resources.get("operations"), "observer.resources.operations")
    markers_at_fence = _markers(ready, "observation_ready")
    final_markers = _markers(resources, "observer.resources")
    _require(all(final_markers[name] >= markers_at_fence[name] for name in _MARKERS),
             "observer resource-marker counters regressed after the fence")
    _require("frame_age" in observer, "observer frame-age mode observation is missing")
    age = observer["frame_age"]
    paused_proof = None
    if requested_mode == "paused":
        _require(age is None, "paused observation must have no frame-age report")
        _require(final == at_fence, "paused observer added counted work after its fence")
        _require(final_markers == markers_at_fence,
                 "paused observer added resource markers after its fence")
        paused_proof = dict(operations_at_fence=at_fence, final_operations=final,
                            markers_at_fence=markers_at_fence, final_markers=final_markers,
                            observation_finish_qpc=finished, sample_window_end_qpc=sample_end,
                            unchanged=True)
    else:
        _require(isinstance(age, dict), "active observation requires a frame-age report")
        _require(_window(age.get("config"), "observer.frame_age.config") == window,
                 "frame-age clock differs from the run-owned sample window")
        _require(_uint(age.get("captured_samples"), "observer.frame_age.captured_samples") > 0,
                 "active observation requires captured frame-age samples")
    return dict(clock=window, consumer_mode=requested_mode, quiescence_verified=True,
                frame_age_available=age is not None, paused_work_proof=paused_proof)

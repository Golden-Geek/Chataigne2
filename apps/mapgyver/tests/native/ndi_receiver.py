"""Receive and verify an exact owned Screen sender through the public NDI SDK."""
import argparse
import ctypes
import json
import math
from pathlib import Path
import time

from ndi_sender import AudioFrame, Source, VideoFrame, fourcc, runtime


class FindSettings(ctypes.Structure):
    _fields_ = [("show_local", ctypes.c_bool), ("groups", ctypes.c_char_p), ("extra_ips", ctypes.c_char_p)]


class ReceiveSettings(ctypes.Structure):
    _fields_ = [
        ("source", Source),
        ("color_format", ctypes.c_int32),
        ("bandwidth", ctypes.c_int32),
        ("allow_fields", ctypes.c_bool),
        ("name", ctypes.c_char_p),
    ]


def receiver_api(dll):
    for function, result, arguments in [
        ("find_create_v2", ctypes.c_void_p, [ctypes.POINTER(FindSettings)]),
        ("find_destroy", None, [ctypes.c_void_p]),
        ("find_wait_for_sources", ctypes.c_bool, [ctypes.c_void_p, ctypes.c_uint32]),
        ("find_get_current_sources", ctypes.POINTER(Source), [ctypes.c_void_p, ctypes.POINTER(ctypes.c_uint32)]),
        ("recv_create_v3", ctypes.c_void_p, [ctypes.POINTER(ReceiveSettings)]),
        ("recv_destroy", None, [ctypes.c_void_p]),
        ("recv_get_no_connections", ctypes.c_int32, [ctypes.c_void_p]),
        ("recv_capture_v3", ctypes.c_int32, [
            ctypes.c_void_p, ctypes.POINTER(VideoFrame), ctypes.POINTER(AudioFrame), ctypes.c_void_p, ctypes.c_uint32,
        ]),
        ("recv_free_video_v2", None, [ctypes.c_void_p, ctypes.POINTER(VideoFrame)]),
        ("recv_free_audio_v3", None, [ctypes.c_void_p, ctypes.POINTER(AudioFrame)]),
    ]:
        method = getattr(dll, f"NDIlib_{function}")
        method.argtypes = arguments
        method.restype = result


def match_source(dll, finder, name):
    count = ctypes.c_uint32()
    sources = dll.NDIlib_find_get_current_sources(finder, ctypes.byref(count))
    matches = []
    for index in range(count.value):
        source = sources[index]
        if not source.name:
            continue
        exact = source.name.decode("utf-8")
        if exact == name or exact.endswith(f" ({name})"):
            matches.append(Source(source.name, source.url))
    if len(matches) > 1:
        raise RuntimeError(f"ambiguous fixture sender {name}")
    return matches[0] if matches else None


def video_sample(dll, receiver, frame):
    try:
        if (
            frame.fourcc not in (fourcc(b"BGRA"), fourcc(b"BGRX"))
            or frame.format != 1 or not frame.data
            or not (1 <= frame.width <= 8192 and 1 <= frame.height <= 8192)
            or frame.line_stride < frame.width * 4
        ):
            raise RuntimeError("unsupported received video layout")
        pixels = []
        for y in (frame.height // 3, frame.height // 2, frame.height * 2 // 3):
            for x in (frame.width // 3, frame.width // 2, frame.width * 2 // 3):
                address = frame.data + y * frame.line_stride + x * 4
                blue, green, red, _ = ctypes.string_at(address, 4)
                pixels.append((red, green, blue))
        return dict(
            width=frame.width, height=frame.height,
            rate=[frame.rate_n, frame.rate_d], timecode=str(frame.timecode), pixels=pixels,
        )
    finally:
        dll.NDIlib_recv_free_video_v2(receiver, ctypes.byref(frame))


def capture(dll, receiver):
    frame = VideoFrame()
    if dll.NDIlib_recv_capture_v3(receiver, ctypes.byref(frame), None, None, 125) == 1:
        return video_sample(dll, receiver, frame)
    return None


def tone_power(samples, frequency, rate):
    radians = math.tau * frequency / rate
    sine = sum(value * math.sin(index * radians) for index, value in enumerate(samples))
    cosine = sum(value * math.cos(index * radians) for index, value in enumerate(samples))
    return sine * sine + cosine * cosine


def observe_audio(dll, receiver, expected, name, write):
    video_ticks = []
    tones = [None] * len(expected)
    silent_peak = 0.0
    blocks = 0
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        picture, sound = VideoFrame(), AudioFrame()
        kind = dll.NDIlib_recv_capture_v3(receiver, ctypes.byref(picture), ctypes.byref(sound), None, 125)
        if kind == 1:
            video_ticks.append(int(video_sample(dll, receiver, picture)["timecode"]))
        if kind == 2:
            try:
                if sound.channels != len(expected):
                    continue
                if (
                    sound.sample_rate != 48000 or sound.fourcc != fourcc(b"FLTp") or not sound.data
                    or not 1 <= sound.samples <= 4096
                    or sound.channel_stride < sound.samples * ctypes.sizeof(ctypes.c_float)
                ):
                    raise RuntimeError("unexpected incoming NDI audio contract")
                blocks += 1
                for channel, frequency in enumerate(expected):
                    data = (ctypes.c_float * sound.samples).from_address(sound.data + channel * sound.channel_stride)
                    rms = math.sqrt(sum(value * value for value in data) / sound.samples)
                    if frequency == 0:
                        silent_peak = max(silent_peak, rms)
                        if rms > 0.00001:
                            raise RuntimeError(f"unassigned stream channel {channel + 1} is audible: {rms}")
                    else:
                        expected_power = tone_power(data, frequency, sound.sample_rate)
                        other = max((tone_power(data, hz, sound.sample_rate) for hz in expected if hz not in (0, frequency)), default=0)
                        if rms > 0.035 and expected_power > other * 1.7:
                            tones[channel] = dict(channel=channel + 1, hz=frequency, rms=rms, timecode=str(sound.timecode))
            finally:
                dll.NDIlib_recv_free_audio_v3(receiver, ctypes.byref(sound))
        if any(hz and tone is None for hz, tone in zip(expected, tones, strict=True)) or not video_ticks:
            continue
        measured = [tone for tone in tones if tone is not None]
        gaps = [min(abs(int(tone["timecode"]) - tick) for tick in video_ticks) / 10000 for tone in measured]
        if max(gaps, default=0) <= 80:
            write(dict(kind="audio", name=name, channels=len(expected), tones=measured,
                       silent_peak=silent_peak, received_blocks=blocks, nearest_picture_ms=gaps))
            return
    raise RuntimeError(f"NDI audio did not match {expected}: tones={tones}, blocks={blocks}, picture={video_ticks[-3:]}")


def observe(dll, finder, args, write):
    find_until = time.monotonic() + 15
    source = None
    while source is None and time.monotonic() < find_until:
        source = match_source(dll, finder, args.name)
        if source is None:
            dll.NDIlib_find_wait_for_sources(finder, 125)
    if source is None:
        raise RuntimeError(f"fixture sender was never discovered: {args.name}")
    name = source.name.decode("utf-8")
    receiver = dll.NDIlib_recv_create_v3(ctypes.byref(ReceiveSettings(source, 0, 100, False, None)))
    if not receiver:
        raise RuntimeError("the SDK did not open the receiver")
    try:
        last = None
        for index, expected in enumerate(args.colors):
            deadline = time.monotonic() + 18
            while time.monotonic() < deadline:
                sample = capture(dll, receiver)
                if sample is None:
                    continue
                last = sample
                if all(abs(actual - wanted) <= 6 for pixel in sample["pixels"]
                       for actual, wanted in zip(pixel, expected, strict=True)):
                    write(dict(kind="picture", name=name, stage=index, **sample))
                    break
            else:
                raise RuntimeError(f"expected picture {expected} from {name}; last sample: {last}")
        if args.tones is not None:
            observe_audio(dll, receiver, args.tones, name, write)
        deadline = time.monotonic() + 20
        while time.monotonic() < deadline:
            capture(dll, receiver)
            if dll.NDIlib_recv_get_no_connections(receiver) == 0 and match_source(dll, finder, args.name) is None:
                write(dict(kind="withdrawn", name=name))
                return
        raise RuntimeError(f"sender did not withdraw: {name}")
    finally:
        dll.NDIlib_recv_destroy(receiver)


def color(value):
    try:
        rgb = tuple(map(int, value.split(",")))
    except ValueError as error:
        raise argparse.ArgumentTypeError("expected three comma-separated RGB bytes") from error
    if len(rgb) != 3 or not all(0 <= byte <= 255 for byte in rgb):
        raise argparse.ArgumentTypeError("expected three comma-separated RGB bytes")
    return rgb


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--name", required=True)
    parser.add_argument("--receipt", required=True, type=Path)
    parser.add_argument("--runtime-dir", type=Path)
    parser.add_argument("--tones", type=lambda value: [int(hz) for hz in value.split(",")],
                        help="expected frequencies by stream channel, with zero for silence")
    parser.add_argument("colors", nargs="+", type=color)
    args = parser.parse_args()
    dll, _ = runtime(args.runtime_dir)
    receiver_api(dll)
    if not dll.NDIlib_initialize():
        raise RuntimeError("NDI failed to initialize")
    try:
        finder = dll.NDIlib_find_create_v2(ctypes.byref(FindSettings(True, None, None)))
        if not finder:
            raise RuntimeError("NDI failed to create a finder")
        try:
            with args.receipt.open("x", encoding="utf-8", buffering=1) as receipt:
                observe(dll, finder, args, lambda record: receipt.write(json.dumps(record) + "\n"))
        finally:
            dll.NDIlib_find_destroy(finder)
    finally:
        dll.NDIlib_destroy()


if __name__ == "__main__":
    main()

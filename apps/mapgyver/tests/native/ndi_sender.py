"""Publish only owned synthetic A/V via the public Windows x64 NDI runtime.

The controller supplies a fresh ready path, stop path, and unique sender name.
Ready JSON records the actual SDK sender name and the independent signal oracle.
"""

import argparse
from array import array
import ctypes
import json
import math
import os
from pathlib import Path
import time


WIDTH = 320
HEIGHT = 240
FPS = 30
SAMPLE_RATE = 48000
SAMPLES_PER_FRAME = SAMPLE_RATE // FPS
CHANNELS = (("left", 440, 4, (80, 135)), ("right", 880, 19, (240, 135)))
PULSE_FRAMES = 3
PULSE_AMPLITUDE = 0.25
CODE_BITS = 13
CODE_ORIGIN = (32, 18)
CODE_STEP = 20
CODE_SENTINEL = (12, 18)
MOTION_SAMPLE_Y = 205


class SendCreate(ctypes.Structure):
    _fields_ = [
        ("name", ctypes.c_char_p),
        ("groups", ctypes.c_char_p),
        ("clock_video", ctypes.c_bool),
        ("clock_audio", ctypes.c_bool),
    ]


class Source(ctypes.Structure):
    _fields_ = [("name", ctypes.c_char_p), ("url", ctypes.c_char_p)]


class VideoFrame(ctypes.Structure):
    _fields_ = [
        ("width", ctypes.c_int32),
        ("height", ctypes.c_int32),
        ("fourcc", ctypes.c_int32),
        ("rate_n", ctypes.c_int32),
        ("rate_d", ctypes.c_int32),
        ("aspect", ctypes.c_float),
        ("format", ctypes.c_int32),
        ("timecode", ctypes.c_int64),
        ("data", ctypes.c_void_p),
        ("line_stride", ctypes.c_int32),
        ("metadata", ctypes.c_char_p),
        ("timestamp", ctypes.c_int64),
    ]


class AudioFrame(ctypes.Structure):
    _fields_ = [
        ("sample_rate", ctypes.c_int32),
        ("channels", ctypes.c_int32),
        ("samples", ctypes.c_int32),
        ("timecode", ctypes.c_int64),
        ("fourcc", ctypes.c_int32),
        ("data", ctypes.c_void_p),
        ("channel_stride", ctypes.c_int32),
        ("metadata", ctypes.c_char_p),
        ("timestamp", ctypes.c_int64),
    ]


def fourcc(value):
    return int.from_bytes(value, "little")


def runtime(runtime_dir):
    if os.name != "nt" or ctypes.sizeof(ctypes.c_void_p) != 8:
        raise RuntimeError("the NDI sender fixture requires Windows x64 Python")
    directory = runtime_dir or os.environ.get("NDI_RUNTIME_DIR_V6")
    if not directory:
        raise RuntimeError("pass --runtime-dir or set NDI_RUNTIME_DIR_V6 to the installed runtime directory")
    library = (Path(directory) / "Processing.NDI.Lib.x64.dll").resolve(strict=True)
    # Synchronous public SDK entry points. Pointers borrow the submitted frame for each call.
    dll = ctypes.CDLL(str(library), winmode=0x1100)
    for function, result, arguments in [
        ("initialize", ctypes.c_bool, []),
        ("destroy", None, []),
        ("version", ctypes.c_char_p, []),
        ("send_create", ctypes.c_void_p, [ctypes.POINTER(SendCreate)]),
        ("send_destroy", None, [ctypes.c_void_p]),
        ("send_get_source_name", ctypes.POINTER(Source), [ctypes.c_void_p]),
        ("send_send_video_v2", None, [ctypes.c_void_p, ctypes.POINTER(VideoFrame)]),
        ("send_send_audio_v3", None, [ctypes.c_void_p, ctypes.POINTER(AudioFrame)]),
    ]:
        method = getattr(dll, f"NDIlib_{function}")
        method.argtypes = arguments
        method.restype = result
    return dll, library


def paint(buffer, left, top, width, height, pixel):
    row = bytes(pixel) * width
    for y in range(top, top + height):
        start = (y * WIDTH + left) * 4
        buffer[start:start + len(row)] = row


def video_payload(index, channels, pulse_frames):
    pixels = bytearray((22, 22, 22, 255)) * (WIDTH * HEIGHT)
    paint(pixels, 0, 0, WIDTH, 36, (0, 0, 0, 255))
    paint(pixels, 4, 8, 16, 20, (255, 255, 255, 255))
    for bit in range(CODE_BITS):
        if index & (1 << bit):
            paint(pixels, CODE_ORIGIN[0] + bit * CODE_STEP - 6, 8, 12, 20, (255, 255, 255, 255))
    for _, _, start, (center_x, center_y) in CHANNELS[:channels]:
        pulse = start <= (index % FPS) < start + pulse_frames
        fill = 245 if pulse else 52
        paint(pixels, center_x - 40, center_y - 28, 80, 56, (fill, fill, fill, 255))
    moving_x = 12 + (index * 7) % (WIDTH - 56)
    paint(pixels, moving_x, MOTION_SAMPLE_Y - 13, 32, 26, (35, 210, 120, 255))
    return pixels


def make_audio_blocks(channels, pulse_frames):
    silence = bytes(SAMPLES_PER_FRAME * 4)
    blocks = []
    for _, frequency, _, _ in CHANNELS[:channels]:
        count = pulse_frames * SAMPLES_PER_FRAME
        # A short ramp avoids a broadband click while preserving exact frame/pulse alignment.
        pulse = array("f", (
            PULSE_AMPLITUDE
            * min(1, index / 64, (count - 1 - index) / 64)
            * math.sin(math.tau * frequency * index / SAMPLE_RATE)
            for index in range(count)
        )).tobytes()
        blocks.append(tuple(
            pulse[index * len(silence):(index + 1) * len(silence)]
            for index in range(pulse_frames)
        ))
    return silence, blocks


def audio_payload(index, silence, blocks):
    planes = []
    for (_, _, start, _), pulse in zip(CHANNELS, blocks):
        offset = index % FPS - start
        planes.append(pulse[offset] if 0 <= offset < len(pulse) else silence)
    return b"".join(planes)


def ready_record(dll, sender, library, args, epoch):
    source = dll.NDIlib_send_get_source_name(sender)
    if not source or not source.contents.name:
        raise RuntimeError("NDI did not report the fixture's actual sender name")
    return {
        "kind": "ready",
        "pid": os.getpid(),
        "name": source.contents.name.decode("utf-8"),
        "provided_name": args.name,
        "runtime": str(library),
        "runtime_version": dll.NDIlib_version().decode("utf-8"),
        "width": WIDTH,
        "height": HEIGHT,
        "fps": FPS,
        "timecode_epoch_100ns": str(epoch),
        "frame_code": {
            "bits": CODE_BITS,
            "origin": CODE_ORIGIN,
            "step": CODE_STEP,
            "sentinel": CODE_SENTINEL,
            "threshold": 128,
        },
        "motion_sample_y": MOTION_SAMPLE_Y,
        "audio": {
            "sample_rate": SAMPLE_RATE,
            "samples_per_frame": SAMPLES_PER_FRAME,
            "pulse_frames": args.pulse_frames,
            "pulse_amplitude": PULSE_AMPLITUDE,
            "lanes": [
                dict(channel=lane, frequency_hz=frequency, first_frame=start, marker=marker)
                for lane, frequency, start, marker in CHANNELS[:args.channels]
            ],
        },
    }


def publish(dll, sender, library, args):
    silence, audio_blocks = make_audio_blocks(args.channels, args.pulse_frames)
    video = VideoFrame(WIDTH, HEIGHT, fourcc(b"BGRX"), FPS, 1, WIDTH / HEIGHT, 1,
                       0, None, WIDTH * 4, None, 0)
    audio = AudioFrame(SAMPLE_RATE, args.channels, SAMPLES_PER_FRAME, 0, fourcc(b"FLTp"),
                       None, SAMPLES_PER_FRAME * 4, None, 0)
    epoch = time.time_ns() // 100
    started = time.monotonic_ns()
    deadline = started + args.max_seconds * 1_000_000_000
    sent = 0
    index = 0
    skipped = 0
    while not args.stop.is_file():
        now = time.monotonic_ns()
        if now >= deadline:
            return "timeout", sent, skipped
        due = started + index * 1_000_000_000 // FPS
        if now < due:
            time.sleep(min(due - now, 10_000_000) / 1_000_000_000)
            continue
        # Drop a stalled pair as a unit. Never replay a burst or skew audio from its marker.
        elapsed_index = (now - started) * FPS // 1_000_000_000
        if elapsed_index > index:
            skipped += elapsed_index - index
            index = elapsed_index
        pixels = video_payload(index, args.channels, args.pulse_frames)
        image = (ctypes.c_ubyte * len(pixels)).from_buffer(pixels)
        video.data = ctypes.addressof(image)
        video.timecode = epoch + index * 10_000_000 // FPS
        dll.NDIlib_send_send_video_v2(sender, ctypes.byref(video))
        if audio_blocks:
            signal = audio_payload(index, silence, audio_blocks)
            samples = ctypes.c_char_p(signal)
            audio.data = ctypes.cast(samples, ctypes.c_void_p)
            audio.timecode = video.timecode
            dll.NDIlib_send_send_audio_v3(sender, ctypes.byref(audio))
        if sent == 0:
            ready = json.dumps(ready_record(dll, sender, library, args, epoch))
            with args.ready.open("x", encoding="utf-8") as file:
                file.write(ready + "\n")
            print(ready, flush=True)
        sent += 1
        index += 1
    return "marker", sent, skipped


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--name", required=True, help="short unique synthetic sender name")
    parser.add_argument("--ready", required=True, type=Path, help="new caller-owned JSON readiness path")
    parser.add_argument("--stop", required=True, type=Path, help="caller-owned marker to stop publishing")
    parser.add_argument("--channels", type=int, choices=(0, 1, 2), default=2)
    parser.add_argument("--pulse-frames", type=int, choices=range(1, 12), default=PULSE_FRAMES)
    parser.add_argument("--max-seconds", type=int, default=60)
    parser.add_argument("--runtime-dir", type=Path)
    args = parser.parse_args()
    if not 1 <= args.max_seconds <= 180:
        parser.error("--max-seconds must be between 1 and 180")
    encoded = args.name.encode("utf-8")
    if not args.name.strip() or len(encoded) > 180 or not args.name.isprintable():
        parser.error("--name must be 1 to 180 UTF-8 bytes without control characters")
    if args.ready.resolve() == args.stop.resolve() or args.ready.exists() or args.stop.exists():
        parser.error("--ready and --stop must be distinct, unused paths")
    try:
        dll, library = runtime(args.runtime_dir)
        if not dll.NDIlib_initialize():
            raise RuntimeError("NDI runtime could not initialize")
        try:
            setup = SendCreate(encoded, None, False, False)
            sender = dll.NDIlib_send_create(ctypes.byref(setup))
            if not sender:
                raise RuntimeError("NDI could not create the fixture sender")
            try:
                reason, sent, skipped = publish(dll, sender, library, args)
            finally:
                dll.NDIlib_send_destroy(sender)
        finally:
            dll.NDIlib_destroy()
    except (AttributeError, OSError, RuntimeError) as error:
        parser.exit(1, f"NDI sender fixture: {error}\n")
    print(json.dumps({"kind": "stopped", "reason": reason, "frames": sent, "skipped": skipped}), flush=True)


if __name__ == "__main__":
    main()

"""Bounded foreground-only frame sequence for the owned M12 panel reproduction.

Uses the existing exact PID/HWND capture guard. No input injection or unrelated desktop capture.
FFmpeg stores lossless RGB frames; the timestamps do not qualify input latency.
"""
import argparse
import json
from pathlib import Path
import subprocess
import time

from window_capture import client_geometry, visible_owned_window


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("directory", type=Path)
    parser.add_argument("--hwnd", type=int, required=True)
    parser.add_argument("--seconds", type=int, default=60)
    args = parser.parse_args()
    assert 1 <= args.seconds <= 90
    ready = json.loads((args.directory / "ready.json").read_text())
    image, bounds = visible_owned_window(args.hwnd, ready["pid"])
    geometry = client_geometry(args.hwnd)
    image = image.convert("RGB")
    image.save(args.directory / "capture-first.png")
    command = ["ffmpeg", "-v", "error", "-y", "-f", "rawvideo", "-pixel_format", "rgb24",
               "-video_size", f"{image.width}x{image.height}", "-framerate", "30", "-i", "pipe:0",
               "-c:v", "libx264rgb", "-preset", "ultrafast", "-crf", "0",
               str(args.directory / "native-frames.mkv")]
    with (args.directory / "capture-stderr.log").open("wb") as errors:
        encoder = subprocess.Popen(command, stdin=subprocess.PIPE, stderr=errors,
                                   creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
        samples = []
        try:
            (args.directory / "capture-ready").write_text("ready")
            deadline = time.monotonic() + 120
            while not (args.directory / "capture-go").exists():
                assert time.monotonic() < deadline, "capture start timeout"
                time.sleep(0.05)
            start = time.perf_counter()
            while time.perf_counter() - start < args.seconds and not (args.directory / "capture-finish").exists():
                begin = time.time_ns()
                image, current_bounds = visible_owned_window(args.hwnd, ready["pid"])
                assert bounds == current_bounds, "owned outer window moved/resized during the dock test"
                end = time.time_ns()
                encoder.stdin.write(image.convert("RGB").tobytes())
                samples.append({"frame": len(samples), "begin_unix_ns": begin, "end_unix_ns": end})
                remaining = 1 / 30 - (time.perf_counter() - start - (len(samples) - 1) / 30)
                if remaining > 0:
                    time.sleep(min(remaining, 1 / 30))
            encoder.stdin.close()
            assert encoder.wait(timeout=15) == 0, "lossless capture encoder failed"
            assert samples
            image.save(args.directory / "capture-last.png")
            result = {"frames": samples, "pid": ready["pid"], "hwnd": args.hwnd,
                      "desktop_bounds": bounds, **geometry,
                      "size": list(image.size), "capture": "visible owned foreground window",
                      "input_latency_verified": False}
            (args.directory / "capture.json").write_text(json.dumps(result, indent=2))
            print(json.dumps({"frames": len(samples), "elapsed_seconds": time.perf_counter() - start}))
        finally:
            if encoder.poll() is None:
                encoder.kill()
                encoder.wait()


if __name__ == "__main__":
    main()

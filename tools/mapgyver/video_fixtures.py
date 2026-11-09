"""Generate finite, independently checked Q06 files; never used by product playback."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess


def digest(path):
    with open(path, "rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def frame_code(rgb, width=320):
    def pixel(x):
        offset = (24 * width + x) * 3
        return sum(rgb[offset:offset + 3]) / 3
    if pixel(4) < 200:
        raise ValueError("missing white frame-code sentinel")
    return sum((pixel(24 + bit * 24) > 128) << bit for bit in range(12))


def generate(encoder, directory):
    directory.mkdir(parents=True, exist_ok=False)
    cases = []
    for width, height, rate in [(1920, 1080, 60), (3840, 2160, 30)]:
        frames = rate * 4
        output = directory / f"prores-hq-{width}x{height}-{rate}.mov"
        filters = ["drawbox=x=0:y=0:w=320:h=48:color=black:t=fill",
                   "drawbox=x=0:y=16:w=8:h=16:color=white:t=fill"]
        filters += [f"drawbox=x={16 + bit * 24}:y=16:w=16:h=16:color=white:t=fill:enable='bitand(n,{1 << bit})'"
                    for bit in range(12)]
        command = [str(encoder), "-hide_banner", "-loglevel", "error", "-n", "-f", "lavfi", "-i",
                   f"testsrc2=size={width}x{height}:rate={rate}", "-vf", ",".join(filters),
                   "-frames:v", str(frames), "-an", "-c:v", "prores_ks", "-profile:v", "3",
                   "-threads", "4", "-pix_fmt", "yuv422p10le", "-color_range", "tv",
                   "-colorspace", "bt709", "-color_primaries", "bt709", "-color_trc", "bt709", str(output)]
        subprocess.run(command, check=True, timeout=180)
        info = json.loads(subprocess.check_output([
            str(encoder.with_name("ffprobe.exe")), "-v", "error", "-select_streams", "v:0",
            "-show_streams", "-of", "json", str(output)]))["streams"][0]
        assert (info["codec_name"], info["profile"], info["pix_fmt"]) == ("prores", "HQ", "yuv422p10le")
        assert (info["width"], info["height"], int(info["nb_frames"])) == (width, height, frames)
        oracle = []
        for index in [0, 1, 31, frames - 1]:
            pixels = subprocess.check_output([
                str(encoder), "-hide_banner", "-loglevel", "error", "-threads", "2", "-ss", str(index / rate),
                "-i", str(output), "-vf", "crop=320:32:0:0", "-frames:v", "1", "-f", "rawvideo",
                "-pix_fmt", "rgb24", "pipe:1"], timeout=30)
            assert len(pixels) == 320 * 32 * 3
            actual = frame_code(pixels)
            assert actual == index, (output.name, index, actual)
            oracle.append(actual)
        cases.append({"file": str(output.resolve()), "width": width, "height": height, "rate": rate,
                      "frames": frames, "duration_seconds": 4, "bytes": output.stat().st_size,
                      "sha256": digest(output), "stream": info, "verified_frame_codes": oracle})
    manifest = {"schema": 1, "codec": "ProRes HQ", "encoder": str(encoder), "encoder_sha256": digest(encoder),
                "encoder_version": subprocess.check_output([str(encoder), "-version"], text=True).splitlines()[0],
                "generator_sha256": digest(__file__), "frame_code": {"bits": 12, "x": 24, "y": 24, "step": 24,
                "sentinel_x": 4, "threshold": 128}, "cases": cases}
    (directory / "fixtures.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--encoder", type=Path, required=True)
    parser.add_argument("--directory", type=Path, required=True)
    args = parser.parse_args()
    manifest = generate(args.encoder.resolve(strict=True), args.directory.resolve())
    print(json.dumps({"result": "pass", "cases": len(manifest["cases"]), "directory": str(args.directory)}))


if __name__ == "__main__":
    main()

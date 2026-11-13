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


PROFILES = {
    "h264": ("H.264 High", "h264-high", ["-c:v", "libx264", "-preset", "veryfast",
             "-crf", "18", "-profile:v", "high", "-pix_fmt", "yuv420p", "-g", "60",
             "-bf", "2"], {"codec_name": "h264", "profile": "High", "pix_fmt": "yuv420p",
             "codec_tag_string": "avc1", "has_b_frames": 2}),
    "hevc": ("HEVC Main", "hevc-main", ["-c:v", "libx265", "-preset", "veryfast",
             "-crf", "20", "-pix_fmt", "yuv420p", "-g", "60", "-bf", "2", "-tag:v", "hvc1",
             "-x265-params", "pools=4:frame-threads=2"], {"codec_name": "hevc",
             "profile": "Main", "pix_fmt": "yuv420p", "codec_tag_string": "hvc1",
             "has_b_frames": 2}),
    "prores": ("ProRes HQ", "prores-hq", ["-c:v", "prores_ks", "-profile:v", "3",
               "-pix_fmt", "yuv422p10le"], {"codec_name": "prores", "profile": "HQ",
               "pix_fmt": "yuv422p10le"}),
    "prores_alpha": ("ProRes 4444", "prores-4444-alpha", ["-c:v", "prores_ks",
                     "-profile:v", "4", "-alpha_bits", "16", "-pix_fmt", "yuva444p10le"],
                     {"codec_name": "prores", "profile": "4444", "pix_fmt": "yuva444p12le",
                     "codec_tag_string": "ap4h"}),
    "hap": ("HAP", "hap", ["-c:v", "hap", "-format", "hap", "-chunks", "4",
            "-compressor", "snappy", "-pix_fmt", "rgba"], {"codec_name": "hap",
            "codec_tag_string": "Hap1", "pix_fmt": "rgb0"}),
    "hap_alpha": ("HAP Alpha", "hap-alpha", ["-c:v", "hap", "-format", "hap_alpha",
                  "-chunks", "4", "-compressor", "snappy", "-pix_fmt", "rgba"],
                  {"codec_name": "hap", "codec_tag_string": "Hap5", "pix_fmt": "rgba"}),
    "hap_q": ("HAP Q", "hap-q", ["-c:v", "hap", "-format", "hap_q", "-chunks", "4",
              "-compressor", "snappy", "-pix_fmt", "rgba"], {"codec_name": "hap",
              "codec_tag_string": "HapY", "pix_fmt": "rgb0"}),
}


def validate_stream(info, expected, width, height, frames):
    for key, value in expected.items():
        if info.get(key) != value:
            raise ValueError(f"unexpected {key}: {info.get(key)!r}, expected {value!r}")
    if (info["width"], info["height"], int(info["nb_frames"])) != (width, height, frames):
        raise ValueError("unexpected fixture dimensions or frame count")


def verify_alpha(rgba, width):
    if len(rgba) != width * 64 * 4:
        raise ValueError("unexpected alpha oracle buffer length")
    samples = []
    for x, y, expected in [(4, 24, 255), (8, 56, 64), (width - 8, 56, 192)]:
        actual = rgba[(y * width + x) * 4 + 3]
        if abs(actual - expected) > 3:
            raise ValueError(f"alpha at {(x, y)} is {actual}, expected {expected}")
        samples.append({"x": x, "y": y, "expected": expected, "actual": actual})
    return samples


WORKLOADS = {
    "standard": [("hd", 1920, 1080, 60), ("uhd", 3840, 2160, 30)],
    "performance": [("uhd-a", 3840, 2160, 60), ("uhd-b", 3840, 2160, 60)],
}


def marker_filters(offset):
    # Disjoint halves of the 12-bit code distinguish files even at identical PTS/extents.
    return ["drawbox=x=0:y=0:w=320:h=48:color=black:t=fill",
            "drawbox=x=0:y=16:w=8:h=16:color=white:t=fill"] + [
        f"drawbox=x={16 + bit * 24}:y=16:w=16:h=16:color=white:t=fill:enable='bitand(n+{offset},{1 << bit})'"
        for bit in range(12)]


def generate(encoder, directory, codec="prores", workload="standard"):
    label, prefix, encoding, expected = PROFILES[codec]
    has_alpha = codec in {"hap_alpha", "prores_alpha"}
    directory.mkdir(parents=True, exist_ok=False)
    cases = []
    for index, (case_id, width, height, rate) in enumerate(WORKLOADS[workload]):
        frames = rate * 4
        offset = index * 2048
        output = directory / f"{prefix}-{case_id}-{width}x{height}-{rate}.mov"
        filters = marker_filters(offset)
        if has_alpha:
            # Keep numbered witnesses opaque; the moving picture below has two alpha levels.
            filters += ["format=rgba", "geq=r='r(X,Y)':g='g(X,Y)':b='b(X,Y)':"
                        "a='if(lt(Y,48),255,if(lt(X,W/2),64,192))'"]
        command = [str(encoder), "-hide_banner", "-loglevel", "error", "-n", "-f", "lavfi", "-i",
                   f"testsrc2=size={width}x{height}:rate={rate}", "-vf", ",".join(filters),
                   "-frames:v", str(frames), "-an", *encoding,
                   "-threads", "4", "-color_range", "tv",
                   "-colorspace", "bt709", "-color_primaries", "bt709", "-color_trc", "bt709", str(output)]
        subprocess.run(command, check=True, timeout=180)
        info = json.loads(subprocess.check_output([
            str(encoder.with_name("ffprobe.exe")), "-v", "error", "-select_streams", "v:0",
            "-show_streams", "-of", "json", str(output)]))["streams"][0]
        validate_stream(info, expected, width, height, frames)
        oracle = []
        for index in [0, 1, 31, frames - 1]:
            pixels = subprocess.check_output([
                str(encoder), "-hide_banner", "-loglevel", "error", "-threads", "2", "-ss", str(index / rate),
                "-i", str(output), "-vf", "crop=320:32:0:0", "-frames:v", "1", "-f", "rawvideo",
                "-pix_fmt", "rgb24", "pipe:1"], timeout=30)
            assert len(pixels) == 320 * 32 * 3
            actual = frame_code(pixels)
            assert actual == index + offset, (output.name, index + offset, actual)
            oracle.append(actual)
        alpha = []
        if has_alpha:
            pixels = subprocess.check_output([
                str(encoder), "-hide_banner", "-loglevel", "error", "-threads", "2", "-i", str(output),
                "-vf", f"crop={width}:64:0:0", "-frames:v", "1", "-f", "rawvideo",
                "-pix_fmt", "rgba", "pipe:1"], timeout=30)
            alpha = verify_alpha(pixels, width)
        cases.append({"id": case_id, "frame_code_offset": offset,
                      "file": str(output.resolve()), "width": width, "height": height, "rate": rate,
                      "frames": frames, "duration_seconds": 4, "bytes": output.stat().st_size,
                      "sha256": digest(output), "stream": info, "verified_frame_codes": oracle,
                      "verified_alpha_samples": alpha})
    manifest = {"schema": 2, "workload": workload, "codec": label,
                "encoder": str(encoder), "encoder_sha256": digest(encoder),
                "encoder_version": subprocess.check_output([str(encoder), "-version"], text=True).splitlines()[0],
                "generator_sha256": digest(__file__), "frame_code": {"bits": 12, "x": 24, "y": 24, "step": 24,
                "sentinel_x": 4, "threshold": 128}, "cases": cases}
    (directory / "fixtures.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--encoder", type=Path, required=True)
    parser.add_argument("--directory", type=Path, required=True)
    parser.add_argument("--codec", choices=PROFILES, default="prores")
    parser.add_argument("--workload", choices=WORKLOADS, default="standard")
    args = parser.parse_args()
    manifest = generate(args.encoder.resolve(strict=True), args.directory.resolve(), args.codec, args.workload)
    print(json.dumps({"result": "pass", "cases": len(manifest["cases"]), "directory": str(args.directory)}))


if __name__ == "__main__":
    main()

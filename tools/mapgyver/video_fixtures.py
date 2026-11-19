"""Generate finite, independently checked Q06 files; never used by product playback."""
import argparse
from fractions import Fraction
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
    "webp_alpha": ("WebP VP8 Alpha", "webp-vp8-alpha",
                   ["-c:v", "libwebp_anim", "-lossless", "0", "-quality", "85",
                    "-compression_level", "2", "-pix_fmt", "yuva420p", "-loop", "0"],
                   {"codec_name": "webp_anim", "pix_fmt": "argb"}),
}


def validate_stream(info, expected, width, height, frames, count_field="nb_frames"):
    for key, value in expected.items():
        if info.get(key) != value:
            raise ValueError(f"unexpected {key}: {info.get(key)!r}, expected {value!r}")
    if (info["width"], info["height"], int(info[count_field])) != (width, height, frames):
        raise ValueError("unexpected fixture dimensions or frame count")


def matroska_timestamps(decoded, rate, frames):
    info = decoded["streams"][0]
    if ("matroska" not in decoded["format"]["format_name"].split(",")
            or Fraction(decoded["format"]["duration"]) != 4
            or Fraction(info["time_base"]) != Fraction(1, 1000)):
        raise ValueError("expected the four-second Matroska fixture with millisecond timestamps")
    return validate_numbered_timestamps(
        [int(row["best_effort_timestamp"]) * 1_000_000 for row in decoded["frames"]], rate, frames)


def validate_numbered_timestamps(points, rate, frames):
    if (len(points) != frames or points[0] != 0
            or any(previous >= current for previous, current in zip(points, points[1:]))
            or any(abs(pts * rate - index * 1_000_000_000) > rate * 1_000_000
                   for index, pts in enumerate(points))):
        raise ValueError("timestamps do not represent the expected numbered frame sequence")
    return points


def webp_oracle(file, width, height, rate, frames, indexes, offset):
    # libwebp reads the encoded animation independently of the app's FFmpeg path.
    from PIL import Image

    timestamps, codes, alpha = [], [], []
    end_ms = 0
    with Image.open(file) as image:
        if (image.format, image.size, image.n_frames, image.mode) != ("WEBP", (width, height), frames, "RGBA"):
            raise ValueError("wrong encoded WebP canvas, frame count or alpha format")
        for index in range(frames):
            image.seek(index)
            image.load()
            timestamp_ms = image.info["timestamp"]
            duration_ms = image.info["duration"]
            if timestamp_ms != end_ms or duration_ms <= 10:
                raise ValueError("noncontiguous WebP timestamps or a normalized short delay")
            timestamps.append(timestamp_ms * 1_000_000)
            end_ms += duration_ms
            if index in indexes:
                code = frame_code(image.crop((0, 0, 320, 32)).convert("RGB").tobytes())
                if code != index + offset:
                    raise ValueError(f"unexpected WebP frame code {code} at frame {index}")
                codes.append(code)
            if index == 0:
                for x, y, expected in ((4, 24, 255), (8, 56, 0), (width - 8, 56, 128)):
                    actual = image.getpixel((x, y))[3]
                    if abs(actual - expected) > 1:
                        raise ValueError(f"WebP alpha at {(x, y)} is {actual}, expected {expected}")
                    alpha.append({"x": x, "y": y, "expected": expected, "actual": actual})
    if end_ms not in {3999, 4000}:
        raise ValueError(f"unexpected encoded WebP duration {end_ms} ms")
    return codes, alpha, {
        "frame_pts_nanos": validate_numbered_timestamps(timestamps, rate, frames),
        "duration_ms": end_ms,
    }


def matroska_marker_oracle(encoder, file, indexes, offset):
    expression = "+".join(f"eq(n\\,{index})" for index in indexes)
    pixels = subprocess.check_output([
        str(encoder), "-hide_banner", "-loglevel", "error", "-threads", "2", "-i", str(file),
        "-vf", f"crop=320:32:0:0,select={expression}", "-fps_mode", "passthrough",
        "-frames:v", str(len(indexes)), "-f", "rawvideo", "-pix_fmt", "rgb24", "pipe:1"], timeout=60)
    frame_size = 320 * 32 * 3
    if len(pixels) != len(indexes) * frame_size:
        raise ValueError("Matroska marker oracle returned the wrong number of frames")
    actual = [frame_code(pixels[start:start + frame_size]) for start in range(0, len(pixels), frame_size)]
    if actual != [index + offset for index in indexes]:
        raise ValueError(f"unexpected Matroska frame codes: {actual}")
    return actual


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


def generate(encoder, directory, codec="prores", workload="standard", container="mov"):
    label, prefix, encoding, expected = PROFILES[codec]
    if container not in {"mov", "matroska", "webp"}:
        raise ValueError(f"unsupported fixture container {container}")
    matroska = container == "matroska"
    webp = container == "webp"
    if webp != (codec == "webp_alpha"):
        raise ValueError("numbered WebP fixtures require the WebP alpha profile and container")
    if matroska:
        if codec not in {"h264", "hevc"}:
            raise ValueError("numbered Matroska fixtures require H.264 or HEVC")
        encoding = encoding.copy()
        if "-tag:v" in encoding:
            tag = encoding.index("-tag:v")
            del encoding[tag:tag + 2]
        expected = {**expected, "codec_tag_string": "[0][0][0][0]"}
    has_alpha = codec in {"hap_alpha", "prores_alpha"}
    directory.mkdir(parents=True, exist_ok=False)
    cases = []
    for index, (case_id, width, height, rate) in enumerate(WORKLOADS[workload]):
        frames = rate * 4
        offset = index * 2048
        suffix = "mkv" if matroska else container
        output = directory / f"{prefix}-{case_id}-{width}x{height}-{rate}.{suffix}"
        filters = marker_filters(offset)
        source = f"testsrc2=size={width}x{height}:rate={rate}"
        if webp:
            source += ":alpha=128,format=yuva420p"
            filters = [name.replace(":t=fill", ":t=fill:replace=1") for name in filters]
            filters.append("drawbox=x=0:y=48:w=96:h=16:color=red@0.0:t=fill:replace=1")
        if has_alpha:
            # Keep numbered witnesses opaque; the moving picture below has two alpha levels.
            filters += ["format=rgba", "geq=r='r(X,Y)':g='g(X,Y)':b='b(X,Y)':"
                        "a='if(lt(Y,48),255,if(lt(X,W/2),64,192))'"]
        command = [str(encoder), "-hide_banner", "-loglevel", "error", "-n", "-f", "lavfi", "-i",
                   source, "-vf", ",".join(filters),
                   "-frames:v", str(frames), "-an", *encoding,
                   "-threads", "4", "-color_range", "tv",
                   "-colorspace", "bt709", "-color_primaries", "bt709", "-color_trc", "bt709", str(output)]
        subprocess.run(command, check=True, timeout=180)
        probe = [str(encoder.with_name("ffprobe.exe")), "-v", "error", "-select_streams", "v:0",
                 "-show_streams", "-of", "json"]
        if matroska:
            probe += ["-count_frames", "-show_frames", "-show_format",
                      "-show_entries", "stream:format=format_name,duration:frame=best_effort_timestamp"]
        decoded = json.loads(subprocess.check_output([*probe, str(output)], timeout=120))
        info = decoded["streams"][0]
        indexes = [0, 1, 31, frames - 1]
        timestamps = {}
        alpha = []
        if webp:
            oracle, alpha, timestamps = webp_oracle(output, width, height, rate, frames, indexes, offset)
            # WebP reports no stream frame count; the independent animation reader proved it.
            validate_stream({**info, "nb_frames": frames}, expected, width, height, frames)
        elif matroska:
            validate_stream(info, expected, width, height, frames, "nb_read_frames")
            timestamps["frame_pts_nanos"] = matroska_timestamps(decoded, rate, frames)
            oracle = matroska_marker_oracle(encoder, output, indexes, offset)
        else:
            validate_stream(info, expected, width, height, frames)
            oracle = []
            for index in indexes:
                pixels = subprocess.check_output([
                    str(encoder), "-hide_banner", "-loglevel", "error", "-threads", "2", "-ss", str(index / rate),
                    "-i", str(output), "-vf", "crop=320:32:0:0", "-frames:v", "1", "-f", "rawvideo",
                    "-pix_fmt", "rgb24", "pipe:1"], timeout=30)
                assert len(pixels) == 320 * 32 * 3
                actual = frame_code(pixels)
                assert actual == index + offset, (output.name, index + offset, actual)
                oracle.append(actual)
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
                      "verified_alpha_samples": alpha, **timestamps})
    manifest = {"schema": 2, "workload": workload, "codec": label, "container": container,
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
    parser.add_argument("--container", choices=("mov", "matroska", "webp"), default="mov")
    args = parser.parse_args()
    manifest = generate(args.encoder.resolve(strict=True), args.directory.resolve(),
                        args.codec, args.workload, args.container)
    print(json.dumps({"result": "pass", "cases": len(manifest["cases"]), "directory": str(args.directory)}))


if __name__ == "__main__":
    main()

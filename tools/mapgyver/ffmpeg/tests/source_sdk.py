"""Verify the isolated source SDK through its public headers and live C exports."""
import argparse
import ctypes
from ctypes import wintypes
import hashlib
import json
from pathlib import Path
import os
import re
import shlex


def inspect(sdk, pin):
    components = ["avcodec", "avformat", "avutil", "swresample", "swscale"]
    loaded = []
    module_path = ctypes.windll.kernel32.GetModuleFileNameW
    module_path.argtypes = [wintypes.HMODULE, wintypes.LPWSTR, wintypes.DWORD]
    module_path.restype = wintypes.DWORD
    with os.add_dll_directory(str(sdk / "bin")):
        for name in components:
            include = sdk / "include" / ("lib" + name)
            header = include / "version_major.h"
            if not header.exists():
                header = include / "version.h"
            major = int(re.search(r"#define\s+LIB" + name.upper() + r"_VERSION_MAJOR\s+(\d+)",
                                  header.read_text()).group(1))
            path = sdk / "bin" / f"{name}-{major}.dll"
            library = ctypes.WinDLL(str(path), winmode=0x1100)
            actual = ctypes.create_unicode_buffer(32768)
            assert module_path(library._handle, actual, len(actual))
            assert Path(actual.value).resolve() == path.resolve(), actual.value
            version = getattr(library, name + "_version")
            version.restype = ctypes.c_uint
            assert version() >> 16 == major
            config = getattr(library, name + "_configuration")
            config.restype = ctypes.c_char_p
            assert shlex.split(config().decode()) == pin["configure"]
            license_text = getattr(library, name + "_license")
            license_text.restype = ctypes.c_char_p
            assert license_text() == b"LGPL version 3 or later"
            if name == "avutil":
                identity = library.av_version_info
                identity.restype = ctypes.c_char_p
                commit = pin["ffmpeg"]["commit"]
                expected = pin["ffmpeg"]["version"] + "-" + commit[:12] + "-mapgyver-" + commit[:10]
                assert identity().decode() == expected, identity()
            if name == "avcodec":
                decoder = library.avcodec_find_decoder_by_name
                decoder.argtypes = [ctypes.c_char_p]
                decoder.restype = ctypes.c_void_p
                for codec in [b"h264", b"hevc", b"prores", b"hap", b"dxv", b"webp"]:
                    assert decoder(codec), codec
            loaded.append({"name": path.name, "bytes": path.stat().st_size,
                           "sha256": hashlib.sha256(path.read_bytes()).hexdigest()})
    assert sorted(p.name for p in (sdk / "bin").glob("*.dll")) == sorted(p["name"] for p in loaded)
    return {"revision": pin["revision"], "ffmpeg_commit": pin["ffmpeg"]["commit"],
            "configure": pin["configure"], "runtime": loaded,
            "license_sha256": hashlib.sha256((sdk / "LICENSE.txt").read_bytes()).hexdigest()}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("sdk", type=Path)
    parser.add_argument("--pin", type=Path, default=Path(__file__).parent.parent / "source-build.json")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    result = inspect(args.sdk.resolve(), json.loads(args.pin.read_text(encoding="utf-8-sig")))
    if args.output:
        args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"result": "pass", "libraries": len(result["runtime"]),
                      "bytes": sum(item["bytes"] for item in result["runtime"]),
                      "ffmpeg_commit": result["ffmpeg_commit"]}))

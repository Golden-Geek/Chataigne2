"""Run qualification, then emit paced lossless chunks below the terminal's output ceiling."""

import hashlib
import json
from pathlib import Path
import subprocess
import sys
import time

PART_CHARACTERS = 65_536


def records(payload, exit_code):
    # The runner's gzip-base64 envelope is ASCII, so character and byte offsets agree.
    text = payload.decode("ascii")
    count = (len(text) + PART_CHARACTERS - 1) // PART_CHARACTERS
    for index in range(count):
        yield dict(collector_wire_schema=1, kind="chunk", index=index,
                   text=text[index * PART_CHARACTERS:(index + 1) * PART_CHARACTERS])
    yield dict(collector_wire_schema=1, kind="end", parts=count, bytes=len(payload),
               sha256=hashlib.sha256(payload).hexdigest(), runner_exit_code=exit_code)


def reconstruct(items):
    if not items or items[-1].get("kind") != "end":
        raise ValueError("collector stream is incomplete")
    end = items[-1]
    if any(item.get("collector_wire_schema") != 1 for item in items):
        raise ValueError("unsupported collector stream")
    chunks = items[:-1]
    if end["parts"] != len(chunks) or any(item.get("kind") != "chunk" or item["index"] != index
                                          for index, item in enumerate(chunks)):
        raise ValueError("collector chunks missing or reordered")
    payload = "".join(item["text"] for item in chunks).encode("ascii")
    if len(payload) != end["bytes"] or hashlib.sha256(payload).hexdigest() != end["sha256"]:
        raise ValueError("collector length/hash mismatch")
    return payload, end["runner_exit_code"]


def main():
    runner = Path(__file__).with_name("run-qualification.py")
    args = sys.argv[1:]
    # Validate before launching any native process; collection changes only output transport.
    import runpy
    parsed = runpy.run_path(str(runner))["parse_args"](args)
    if parsed.encoding != "gzip-base64":
        raise SystemExit("collector requires --encoding gzip-base64")
    result = subprocess.run([sys.executable, str(runner), *args], stdout=subprocess.PIPE)
    # All owned native measurement windows have ended before any pacing or chunk encoding.
    for record in records(result.stdout, result.returncode):
        print(json.dumps(record, separators=(",", ":")), flush=True)
        if record["kind"] == "chunk":
            time.sleep(0.5)
    return result.returncode


if __name__ == "__main__":
    sys.exit(main())

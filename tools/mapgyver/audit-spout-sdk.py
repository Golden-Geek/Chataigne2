"""Compare the published SDK files with a pinned upstream Git tree, without writing files."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import urllib.request


UPSTREAM_REVISION = "f49e2f469f8cb25f559a6eaa61a3f5b8173fc100"


def git_blob(data: bytes) -> str:
    return hashlib.sha1(f"blob {len(data)}\0".encode() + data).hexdigest()


def audit(package_root: Path) -> dict:
    sdk_root = package_root / "vendor" / "Spout2"
    if not sdk_root.is_dir():
        raise ValueError(f"SDK source directory absent: {sdk_root}")
    url = f"https://api.github.com/repos/leadedge/Spout2/git/trees/{UPSTREAM_REVISION}?recursive=1"
    request = urllib.request.Request(url, headers={"User-Agent": "MapGyver-native-audit"})
    with urllib.request.urlopen(request, timeout=30) as response:
        tree = json.load(response)
    if tree.get("truncated"):
        raise ValueError("upstream Git tree is truncated")
    upstream = {entry["path"]: entry["sha"] for entry in tree["tree"] if entry["type"] == "blob"}
    records = []
    for file in sorted(sdk_root.rglob("*")):
        if not file.is_file():
            continue
        data = file.read_bytes()
        path = file.relative_to(sdk_root).as_posix()
        raw = git_blob(data)
        normalized = git_blob(data.replace(b"\r\n", b"\n"))
        expected = upstream.get(path)
        classification = "different"
        if expected is None:
            classification = "absent_upstream"
        elif raw == expected:
            classification = "identical"
        elif normalized == expected:
            classification = "crlf_only"
        records.append({
            "path": path,
            "classification": classification,
            "sha256": hashlib.sha256(data).hexdigest(),
            "git_blob_sha1": raw,
            "lf_git_blob_sha1": normalized,
            "upstream_git_blob_sha1": expected,
        })
    return {
        "schema_version": 1,
        "package": "spout2-sys 0.1.1",
        "upstream_repository": "https://github.com/leadedge/Spout2",
        "upstream_tag": "2.007.017",
        "compared_upstream_revision": UPSTREAM_REVISION,
        "packaged_file_count": len(records),
        "identical_count": sum(r["classification"] == "identical" for r in records),
        "crlf_only_count": sum(r["classification"] == "crlf_only" for r in records),
        "different_count": sum(r["classification"] in {"different", "absent_upstream"} for r in records),
        "records": records,
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package-root", type=Path)
    args = parser.parse_args()
    package_root = args.package_root
    if package_root is None:
        cargo_root = Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo"))
        candidates = list((cargo_root / "registry" / "src").glob("*/spout2-sys-0.1.1"))
        if len(candidates) != 1:
            parser.error("specify --package-root; expected exactly one downloaded spout2-sys 0.1.1")
        package_root = candidates[0]
    report = audit(package_root)
    print(json.dumps(report, indent=2))
    if not report["packaged_file_count"] or report["different_count"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()

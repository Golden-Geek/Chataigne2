#!/usr/bin/env python3
"""Run Clippy while allowing only an exact, shrinking warning baseline."""

from __future__ import annotations

import argparse
import collections
import json
import os
from pathlib import Path
import subprocess
import sys


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--manifest-path", type=Path)
    parser.add_argument("--emit-baseline", action="store_true")
    return parser.parse_args()


def normalized_file(file_name: str, workspace: Path) -> str:
    path = Path(file_name)
    try:
        path = path.resolve().relative_to(workspace.resolve())
    except ValueError:
        pass
    normalized = path.as_posix()
    if normalized.startswith("target/") and "/out/app_nodes.rs" in normalized:
        return "target/<generated>/app_nodes.rs"
    return normalized


def warning_key(message: dict, workspace: Path) -> str | None:
    diagnostic = message.get("message", {})
    if diagnostic.get("level") != "warning":
        return None
    code = (diagnostic.get("code") or {}).get("code") or "rustc::uncoded"
    spans = [span for span in diagnostic.get("spans", []) if span.get("is_primary")]
    file_name = spans[0].get("file_name", "<unknown>") if spans else "<unknown>"
    return f"{code}|{normalized_file(file_name, workspace)}"


def main() -> int:
    args = parse_args()
    workspace = Path.cwd()
    command = [
        "cargo",
        "clippy",
        "--workspace",
        "--all-targets",
        "--all-features",
        "--message-format=json",
    ]
    if args.manifest_path:
        command.extend(["--manifest-path", str(args.manifest_path)])
    command.extend(["--", "-D", "warnings", "--cap-lints", "warn"])

    environment = os.environ.copy()
    environment.setdefault("GC_SKIP_UI_BUILD", "1")
    result = subprocess.run(
        command,
        cwd=workspace,
        env=environment,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )

    warnings: collections.Counter[str] = collections.Counter()
    hard_errors: list[str] = []
    rendered_by_key: dict[str, str] = {}
    for line in result.stdout.splitlines():
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            continue
        if message.get("reason") != "compiler-message":
            continue
        diagnostic = message.get("message", {})
        if diagnostic.get("level") == "error":
            hard_errors.append(diagnostic.get("rendered") or diagnostic.get("message", "error"))
            continue
        key = warning_key(message, workspace)
        if key:
            warnings[key] += 1
            rendered_by_key.setdefault(key, diagnostic.get("rendered") or key)

    document = {"schema_version": 1, "warnings": dict(sorted(warnings.items()))}
    if args.emit_baseline:
        print(json.dumps(document, indent=2))
        return 0 if result.returncode == 0 and not hard_errors else 1

    if not args.baseline:
        print("--baseline is required unless --emit-baseline is used", file=sys.stderr)
        return 2
    baseline = json.loads(args.baseline.read_text(encoding="utf-8"))
    allowed = collections.Counter(baseline.get("warnings", {}))
    excess = warnings - allowed

    if hard_errors:
        print("\n".join(hard_errors), file=sys.stderr)
    if excess:
        print("Clippy produced warnings outside the committed baseline:", file=sys.stderr)
        for key, count in sorted(excess.items()):
            print(f"\n[{count} new] {rendered_by_key[key]}", file=sys.stderr)
    if result.returncode != 0 and not hard_errors:
        print(result.stderr, file=sys.stderr)
    if hard_errors or excess or result.returncode != 0:
        return 1

    remaining = sum(warnings.values())
    removed = sum((allowed - warnings).values())
    print(f"Clippy gate passed with {remaining} baselined warning(s); {removed} removed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

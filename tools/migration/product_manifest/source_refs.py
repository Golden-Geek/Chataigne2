"""Discover exact committed gitlink revisions, including nested submodules."""

from __future__ import annotations

import configparser
import subprocess
from collections.abc import Sequence
from pathlib import Path
from typing import Any


def _git_output(root: Path, arguments: Sequence[str]) -> bytes | None:
    try:
        result = subprocess.run(
            ["git", "-C", str(root), *arguments],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
        )
    except OSError:
        return None
    return result.stdout if result.returncode == 0 else None


def _submodule_metadata(repo_root: Path) -> dict[str, dict[str, str]]:
    path = repo_root / ".gitmodules"
    if not path.is_file():
        return {}
    parser = configparser.ConfigParser(interpolation=None)
    try:
        parser.read(path, encoding="utf-8")
    except (configparser.Error, OSError):
        return {}
    result: dict[str, dict[str, str]] = {}
    for section in parser.sections():
        if not section.startswith("submodule ") or not parser.has_option(section, "path"):
            continue
        module_path = parser.get(section, "path").replace("\\", "/")
        result[module_path] = {
            key: parser.get(section, key)
            for key in ("url", "branch")
            if parser.has_option(section, key)
        }
    return result


def scan_submodule_refs(root: Path) -> list[dict[str, Any]]:
    """Read gitlinks from each repository index, never working branch heads."""
    entries: list[dict[str, Any]] = []
    visited: set[Path] = set()

    def visit(repo_root: Path, prefix: str) -> None:
        resolved = repo_root.resolve()
        if resolved in visited:
            return
        visited.add(resolved)
        output = _git_output(repo_root, ("ls-files", "--stage", "-z"))
        if output is None:
            return
        metadata = _submodule_metadata(repo_root)
        for record in output.split(b"\0"):
            if not record:
                continue
            header, separator, raw_path = record.partition(b"\t")
            fields = header.decode("utf-8", errors="replace").split()
            if not separator or len(fields) < 3 or fields[0] != "160000":
                continue
            child_path = raw_path.decode("utf-8", errors="replace").replace("\\", "/")
            full_path = f"{prefix}/{child_path}".strip("/")
            meta = metadata.get(child_path, {})
            entries.append(
                {
                    "path": full_path,
                    "gitlink": fields[1],
                    "parent": prefix or ".",
                    "url": meta.get("url"),
                    "branch": meta.get("branch"),
                }
            )
            child_root = repo_root / child_path
            if child_root.is_dir():
                visit(child_root, full_path)

    visit(root, "")
    return sorted(entries, key=lambda entry: entry["path"])

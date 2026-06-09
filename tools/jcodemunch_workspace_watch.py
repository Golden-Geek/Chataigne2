from __future__ import annotations

import atexit
import json
import os
import subprocess
import sys
import time
from pathlib import Path

from jcodemunch_mcp import config as jcodemunch_config
from jcodemunch_mcp.parser import LANGUAGE_EXTENSIONS, get_language_for_path
from jcodemunch_mcp.storage import IndexStore
from jcodemunch_mcp.tools.index_file import index_file


WORKSPACE = Path(__file__).resolve().parent.parent
INDEX_ROOT = Path(os.environ.get("CODE_INDEX_PATH", Path.home() / ".code-index")).resolve()
LOCK_PATH = INDEX_ROOT / "chataigne2-workspace-watch.lock"
POLL_SECONDS = 1.0
GIT_POLL_SECONDS = 5.0

EXPECTED_SOURCE_ROOTS = (
    WORKSPACE,
    WORKSPACE / "src-ui" / "src" / "lib" / "golden_alchemist_ui",
    WORKSPACE / "src-ui" / "src" / "lib" / "golden_ui",
    WORKSPACE / "submodules" / "golden_alchemist_core",
    WORKSPACE / "submodules" / "golden_core",
)

SKIP_DIRECTORY_NAMES = {
    ".git",
    ".idea",
    ".kilo",
    ".netlify",
    ".output",
    ".pytest_cache",
    ".ruff_cache",
    ".svelte-kit",
    ".venv",
    ".vercel",
    ".wrangler",
    "__pycache__",
    "artifacts",
    "build",
    "node_modules",
    "target",
    "target-ci-validate",
}

SKIP_RELATIVE_PREFIXES = (
    Path("gen/schemas"),
    Path("icons"),
    Path("src-ui/static"),
    Path("src-ui/src/lib/golden_ui/generated"),
)


def process_is_alive(pid: int) -> bool:
    try:
        os.kill(pid, 0)
    except OSError:
        return False
    return True


def acquire_lock() -> None:
    INDEX_ROOT.mkdir(parents=True, exist_ok=True)
    announced_wait = False
    while True:
        try:
            descriptor = os.open(LOCK_PATH, os.O_CREAT | os.O_EXCL | os.O_WRONLY)
        except FileExistsError:
            try:
                holder = json.loads(LOCK_PATH.read_text(encoding="utf-8"))
                holder_pid = int(holder.get("pid", 0))
            except (OSError, ValueError, TypeError, json.JSONDecodeError):
                holder_pid = 0
            if holder_pid and process_is_alive(holder_pid):
                if not announced_wait:
                    print(f"jCodeMunch watcher waiting for existing process {holder_pid}", flush=True)
                    announced_wait = True
                time.sleep(2)
                continue
            LOCK_PATH.unlink(missing_ok=True)
            continue
        with os.fdopen(descriptor, "w", encoding="utf-8") as lock_file:
            json.dump({"pid": os.getpid(), "workspace": str(WORKSPACE)}, lock_file)
        break


def release_lock() -> None:
    try:
        holder = json.loads(LOCK_PATH.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return
    if holder.get("pid") == os.getpid():
        LOCK_PATH.unlink(missing_ok=True)


def canonical_repositories(store: IndexStore) -> dict[str, Path]:
    entries = store.list_repos()
    repositories: dict[str, Path] = {}
    errors: list[str] = []

    for source_root in EXPECTED_SOURCE_ROOTS:
        source_root = source_root.resolve()
        matches = [
            entry
            for entry in entries
            if Path(entry.get("source_root", "")).resolve() == source_root
        ]
        if len(matches) != 1 or not matches[0]["repo"].startswith("local/"):
            identities = ", ".join(entry["repo"] for entry in matches) or "none"
            errors.append(f"{source_root} has indexes [{identities}]")
            continue
        repositories[matches[0]["repo"]] = source_root

    if errors:
        raise RuntimeError(
            "Expected exactly one local jCodeMunch index per workspace repository: "
            + "; ".join(errors)
        )
    return repositories


def configure_jcodemunch() -> None:
    jcodemunch_config.load_config(storage_path=str(INDEX_ROOT))
    jcodemunch_config.load_project_config(str(WORKSPACE))
    extra_extensions = jcodemunch_config.get(
        "extra_extensions",
        {},
        repo=str(WORKSPACE),
    )
    LANGUAGE_EXTENSIONS.update(extra_extensions)


def is_skipped(path: Path) -> bool:
    try:
        relative = path.relative_to(WORKSPACE)
    except ValueError:
        return True
    if path.name == ".env" or path.name.startswith(".env."):
        return True
    if any(part in SKIP_DIRECTORY_NAMES for part in relative.parts[:-1]):
        return True
    return any(relative == prefix or prefix in relative.parents for prefix in SKIP_RELATIVE_PREFIXES)


def is_supported(path: Path) -> bool:
    return not is_skipped(path) and get_language_for_path(str(path)) is not None


def scan_files() -> dict[Path, tuple[int, int]]:
    files: dict[Path, tuple[int, int]] = {}
    for root, directories, filenames in os.walk(WORKSPACE):
        root_path = Path(root)
        directories[:] = [
            directory
            for directory in directories
            if not is_skipped(root_path / directory / ".placeholder")
        ]
        for filename in filenames:
            path = root_path / filename
            if not is_supported(path):
                continue
            try:
                stat = path.stat()
            except OSError:
                continue
            files[path.resolve()] = (stat.st_mtime_ns, stat.st_size)
    return files


def matching_repository(
    repositories: dict[str, Path], path: Path
) -> tuple[str, Path] | None:
    match: tuple[str, Path] | None = None
    best_length = -1
    for repo, source_root in repositories.items():
        try:
            path.relative_to(source_root)
        except ValueError:
            continue
        if len(str(source_root)) > best_length:
            match = repo, source_root
            best_length = len(str(source_root))
    return match


def remove_indexed_file(store: IndexStore, repo: str, relative: str) -> None:
    owner, name = repo.split("/", 1)
    index = store.load_index(owner, name)
    if index is None or relative not in index.file_hashes:
        return
    store.incremental_save(
        owner=owner,
        name=name,
        changed_files=[],
        new_files=[],
        deleted_files=[relative],
        new_symbols=[],
        raw_files={},
        git_head=index.git_head or "",
    )
    print(f"jCodeMunch removed {repo}:{relative}", flush=True)


def remove_deleted_file(
    store: IndexStore, repositories: dict[str, Path], path: Path
) -> None:
    match = matching_repository(repositories, path)
    if match is None:
        return
    repo, source_root = match
    relative = path.relative_to(source_root).as_posix()
    remove_indexed_file(store, repo, relative)


def refresh_file(path: Path) -> None:
    result = index_file(
        path=str(path),
        use_ai_summaries=False,
        storage_path=str(INDEX_ROOT),
    )
    if not result.get("success"):
        error = result.get("error", result)
        print(
            f"jCodeMunch update failed for {path}: {error}",
            file=sys.stderr,
            flush=True,
        )
        return
    print(f"jCodeMunch updated {result.get('repo')}:{result.get('file')}", flush=True)


def current_git_head(source_root: Path) -> str:
    result = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=source_root,
        capture_output=True,
        text=True,
        timeout=2,
        check=False,
        creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
    )
    return result.stdout.strip() if result.returncode == 0 else ""


def synchronize_git_heads(
    store: IndexStore,
    repositories: dict[str, Path],
) -> None:
    for repo, source_root in repositories.items():
        owner, name = repo.split("/", 1)
        index = store.load_index(owner, name)
        git_head = current_git_head(source_root)
        if index is None or not git_head or index.git_head == git_head:
            continue
        store.incremental_save(
            owner=owner,
            name=name,
            changed_files=[],
            new_files=[],
            deleted_files=[],
            new_symbols=[],
            raw_files={},
            git_head=git_head,
        )
        print(f"jCodeMunch updated {repo} Git HEAD to {git_head[:12]}", flush=True)


def synchronize_startup(
    store: IndexStore,
    repositories: dict[str, Path],
    current: dict[Path, tuple[int, int]],
) -> None:
    for path, (mtime, _) in current.items():
        match = matching_repository(repositories, path)
        if match is None:
            continue
        repo, source_root = match
        owner, name = repo.split("/", 1)
        index = store.load_index(owner, name)
        relative = path.relative_to(source_root).as_posix()
        if (
            index is None
            or relative not in index.file_hashes
            or index.file_mtimes.get(relative) != mtime
        ):
            refresh_file(path)

    for repo, source_root in repositories.items():
        owner, name = repo.split("/", 1)
        index = store.load_index(owner, name)
        if index is None:
            continue
        for relative in list(index.file_hashes):
            path = (source_root / relative).resolve()
            match = matching_repository(repositories, path)
            owns_path = match is not None and match[0] == repo
            if not path.exists() or not is_supported(path) or not owns_path:
                remove_indexed_file(store, repo, relative)


def main() -> int:
    print("jCodeMunch watcher starting", flush=True)
    acquire_lock()
    atexit.register(release_lock)

    configure_jcodemunch()
    store = IndexStore(base_path=str(INDEX_ROOT))
    repositories = canonical_repositories(store)
    current = scan_files()
    synchronize_startup(store, repositories, current)
    synchronize_git_heads(store, repositories)
    print("jCodeMunch watcher ready", flush=True)

    next_git_poll = time.monotonic() + GIT_POLL_SECONDS
    while True:
        time.sleep(POLL_SECONDS)
        next_files = scan_files()
        for path in sorted(current.keys() - next_files.keys()):
            remove_deleted_file(store, repositories, path)
        for path in sorted(next_files.keys()):
            if current.get(path) != next_files[path]:
                refresh_file(path)
        current = next_files
        if time.monotonic() >= next_git_poll:
            synchronize_git_heads(store, repositories)
            next_git_poll = time.monotonic() + GIT_POLL_SECONDS


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except KeyboardInterrupt:
        raise SystemExit(0)

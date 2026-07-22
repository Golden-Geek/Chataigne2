"""Source and file discovery for the current Chataigne product."""

from __future__ import annotations

import hashlib
import json
import os
import re
from collections.abc import Iterable, Mapping
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any


IGNORED_DIRS = {
    ".git",
    ".idea",
    ".svelte-kit",
    ".vscode",
    "build",
    "coverage",
    "dist",
    "gen",
    "node_modules",
    "target",
}
IGNORED_PREFIXES = (
    "docs/product/manifests/",
    "tools/migration/",
)
TEXT_SUFFIXES = {
    ".c",
    ".cc",
    ".cpp",
    ".css",
    ".h",
    ".hpp",
    ".html",
    ".js",
    ".json",
    ".jsx",
    ".md",
    ".mjs",
    ".rhai",
    ".rs",
    ".scss",
    ".svelte",
    ".toml",
    ".ts",
    ".tsx",
    ".yaml",
    ".yml",
}
FORMULA_SUFFIXES = {
    ".chai",
    ".chataigne",
    ".formula",
    ".json",
    ".lrg",
    ".toml",
    ".yaml",
    ".yml",
}
SCRIPT_TEMPLATE_SUFFIXES = {".js", ".mjs", ".rhai", ".ts", ".txt"}
FIXTURE_DIR_NAMES = {
    "fixtures",
    "test-data",
    "test-fixtures",
    "test-samples",
    "testdata",
}


@dataclass
class Discovery:
    kind: str
    name: str
    method: str
    path: str
    line: int = 1
    certainty: str = "declared"
    facts: dict[str, Any] = field(default_factory=dict)
    stable_name: str | None = None


def _line_number(text: str, offset: int) -> int:
    return text.count("\n", 0, offset) + 1


def _slug(value: str) -> str:
    slug = re.sub(r"[^a-z0-9]+", "-", value.casefold()).strip("-")
    return slug or "unnamed"


def _stable_product_path(path: str) -> str:
    """Return the Phase 0 logical path used for path-derived capability IDs."""
    normalized = path.replace("\\", "/")
    alchemist_icons = "apps/chataigne/ui/src/lib/assets/icons/"
    if normalized.startswith(alchemist_icons) and Path(normalized).name in {
        "formula.svg",
        "formula_library.svg",
        "lock.svg",
    }:
        return (
            "src-ui/src/lib/golden_alchemist_ui/icons/"
            + normalized.removeprefix(alchemist_icons)
        )
    replacements = (
        ("apps/chataigne/ui/", "src-ui/"),
        ("apps/chataigne/icons/", "icons/"),
        ("packages/golden-ui/", "src-ui/src/lib/golden_ui/"),
        ("apps/chataigne/", ""),
        ("crates/core/", "submodules/golden-core/crates/core/"),
    )
    for current, baseline in replacements:
        if normalized.startswith(current):
            normalized = baseline + normalized.removeprefix(current)
            break
    if normalized == "src/module/script_templates/spatializer_module.js":
        return "src/module/script_templates/spatializer.js"
    return normalized


def _canonical_bytes(path: Path) -> bytes:
    content = path.read_bytes()
    try:
        text = content.decode("utf-8")
    except UnicodeDecodeError:
        return content
    return text.replace("\r\n", "\n").replace("\r", "\n").encode("utf-8")


def _sha256(path: Path) -> str:
    return hashlib.sha256(_canonical_bytes(path)).hexdigest()


def _read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8", errors="replace")


def _is_test_path(path: str) -> bool:
    lowered = f"/{path.casefold()}"
    return (
        "/tests/" in lowered
        or lowered.endswith("_tests.rs")
        or lowered.endswith(".test.ts")
        or lowered.endswith(".spec.ts")
    )


def _iter_files(root: Path) -> Iterable[tuple[str, Path]]:
    for base, directories, filenames in os.walk(root):
        directories[:] = sorted(
            directory
            for directory in directories
            if directory not in IGNORED_DIRS and not directory.startswith(".")
        )
        base_path = Path(base)
        for filename in sorted(filenames):
            path = base_path / filename
            relative = path.relative_to(root).as_posix()
            if (
                filename == ".git"
                or filename.casefold().endswith(".backup")
                or relative.startswith(IGNORED_PREFIXES)
            ):
                continue
            yield relative, path


def _balanced_block(text: str, opening: int) -> tuple[str, int] | None:
    if opening < 0 or opening >= len(text) or text[opening] != "{":
        return None
    depth = 0
    quote: str | None = None
    escaped = False
    line_comment = False
    block_comment = False
    index = opening
    while index < len(text):
        char = text[index]
        next_char = text[index + 1] if index + 1 < len(text) else ""
        if line_comment:
            if char == "\n":
                line_comment = False
            index += 1
            continue
        if block_comment:
            if char == "*" and next_char == "/":
                block_comment = False
                index += 2
            else:
                index += 1
            continue
        if quote is not None:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == quote:
                quote = None
            index += 1
            continue
        if char == "/" and next_char == "/":
            line_comment = True
            index += 2
            continue
        if char == "/" and next_char == "*":
            block_comment = True
            index += 2
            continue
        if char in "'\"`":
            quote = char
        elif char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return text[opening : index + 1], index + 1
        index += 1
    return None


def _brace_depth(text: str, offset: int) -> int:
    depth = 0
    quote: str | None = None
    escaped = False
    for char in text[:offset]:
        if quote is not None:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == quote:
                quote = None
            continue
        if char in "'\"`":
            quote = char
        elif char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
    return depth


def _associated_const(text: str, type_name: str, const_name: str) -> str | None:
    impl_pattern = re.compile(
        rf"\bimpl(?:\s*<[^{{>]*>)?[^{{;]*\b{re.escape(type_name)}\b[^{{;]*\{{"
    )
    const_pattern = re.compile(
        rf"\b{re.escape(const_name)}\s*:\s*[^=;\n]+\s*=\s*[r#]*[\"']([^\"']+)[\"']"
    )
    for impl_match in impl_pattern.finditer(text):
        opening = text.find("{", impl_match.start())
        block = _balanced_block(text, opening)
        if block is None:
            continue
        const_match = const_pattern.search(block[0])
        if const_match:
            return const_match.group(1)
    return None


def _preceding_node_attribute(text: str, offset: int) -> str | None:
    attributes = list(
        re.finditer(
            r"#\s*\[\s*(?:[A-Za-z0-9_]+::)*node\s*\(\s*[\"']([^\"']+)[\"']",
            text[:offset],
        )
    )
    return attributes[-1].group(1) if attributes else None


class InventoryBuilder:
    def __init__(self) -> None:
        self._items: dict[tuple[str, str], dict[str, Any]] = {}

    def add(self, discovery: Discovery) -> None:
        name = discovery.name.strip()
        if not name:
            return
        key = (discovery.kind, name)
        item = self._items.setdefault(
            key,
            {
                "kind": discovery.kind,
                "name": name,
                "sources": set(),
                "discovery_methods": set(),
                "certainty": discovery.certainty,
                "facts": {},
                "stable_name": discovery.stable_name or name,
            },
        )
        stable_name = discovery.stable_name or name
        if item["stable_name"] != stable_name:
            raise ValueError(
                f"conflicting stable names for {discovery.kind}/{name}: "
                f"{item['stable_name']} != {stable_name}"
            )
        item["sources"].add((discovery.path, max(1, discovery.line)))
        item["discovery_methods"].add(discovery.method)
        if discovery.certainty == "registered":
            item["certainty"] = "registered"
        for fact_name, fact_value in discovery.facts.items():
            if fact_value is None or fact_value == "":
                continue
            previous = item["facts"].get(fact_name)
            if previous is None:
                item["facts"][fact_name] = fact_value
            elif previous != fact_value:
                values = previous if isinstance(previous, list) else [previous]
                if fact_value not in values:
                    values.append(fact_value)
                item["facts"][fact_name] = sorted(values, key=lambda value: str(value))

    def entries(self) -> list[dict[str, Any]]:
        entries: list[dict[str, Any]] = []
        used_ids: dict[str, str] = {}
        for (kind, name), item in sorted(
            self._items.items(),
            key=lambda pair: (pair[0][0], pair[0][1].casefold(), pair[0][1]),
        ):
            base_id = f"{kind}/{_slug(item['stable_name'])}"
            entry_id = base_id
            if base_id in used_ids and used_ids[base_id] != name:
                suffix = hashlib.sha256(name.encode("utf-8")).hexdigest()[:8]
                entry_id = f"{base_id}-{suffix}"
            used_ids[entry_id] = name
            entries.append(
                {
                    "id": entry_id,
                    "kind": kind,
                    "name": name,
                    "certainty": item["certainty"],
                    "discovery_methods": sorted(item["discovery_methods"]),
                    "sources": [
                        {"path": path, "line": line}
                        for path, line in sorted(item["sources"])
                    ],
                    "facts": item["facts"],
                }
            )
        return entries


def _scan_registered_panels(
    path: str, text: str, builder: InventoryBuilder
) -> set[str]:
    components: set[str] = set()
    declaration = re.compile(
        r"\b(?:const|let|var)\s+(?:userPanels|[A-Za-z0-9_]*PanelDefinitions)\b[^=]*="
    )
    for match in declaration.finditer(text):
        opening = text.find("{", match.end())
        block_result = _balanced_block(text, opening)
        if block_result is None:
            continue
        block = block_result[0]
        for property_match in re.finditer(
            r"(?m)^\s*(?:[\"']([^\"']+)[\"']|([A-Za-z_$][\w$-]*))\s*:\s*\{",
            block,
        ):
            if _brace_depth(block, property_match.start()) != 1:
                continue
            panel_id = property_match.group(1) or property_match.group(2)
            property_opening = block.find("{", property_match.start())
            property_block = _balanced_block(block, property_opening)
            if property_block is None:
                continue
            body = property_block[0]
            title_match = re.search(r"\btitle\s*:\s*[\"']([^\"']+)[\"']", body)
            component_match = re.search(
                r"\bcomponent\s*:\s*([A-Za-z_$][\w$]*)", body
            )
            component = component_match.group(1) if component_match else None
            if component:
                components.add(component)
            builder.add(
                Discovery(
                    "panel",
                    panel_id,
                    "registered_panel_definition",
                    path,
                    _line_number(text, opening + property_match.start()),
                    "registered",
                    {
                        "title": title_match.group(1) if title_match else None,
                        "component": component,
                    },
                )
            )
    for match in re.finditer(
        r"\bregister(?:User)?Panel\s*\(\s*[\"']([^\"']+)[\"']", text
    ):
        builder.add(
            Discovery(
                "panel",
                match.group(1),
                "register_panel_call",
                path,
                _line_number(text, match.start()),
                "registered",
            )
        )
    return components


def _scan_commands(path: str, text: str, builder: InventoryBuilder) -> None:
    call_pattern = re.compile(
        r"\b(registerCommandHandler|register_command(?:_handler)?|registerCommand)"
        r"\s*\(\s*[\"']([^\"']+)[\"']"
    )
    for match in call_pattern.finditer(text):
        builder.add(
            Discovery(
                "command",
                match.group(2),
                match.group(1),
                path,
                _line_number(text, match.start()),
                "registered",
            )
        )
    if "command" not in path.casefold():
        return
    for match in re.finditer(
        r"\b(?:COMMAND_ID|command_id|commandId)\s*[:=][^\n]*?[\"']([^\"']+)[\"']",
        text,
    ):
        builder.add(
            Discovery(
                "command",
                match.group(1),
                "command_id_declaration",
                path,
                _line_number(text, match.start()),
            )
        )
    if _is_test_path(path):
        return
    for match in re.finditer(r"\b(?:pub\s+)?struct\s+([A-Za-z0-9_]*Command)\b", text):
        type_name = match.group(1)
        node_type = _associated_const(text, type_name, "NODE_TYPE") or _associated_const(
            text, type_name, "ITEM_NODE_TYPE"
        )
        node_type = node_type or _preceding_node_attribute(text, match.start())
        builder.add(
            Discovery(
                "command",
                node_type or type_name,
                "command_node_declaration",
                path,
                _line_number(text, match.start()),
                "declared",
                {"rust_type": type_name, "node_type": node_type},
            )
        )


def _scan_node_types(path: str, text: str, builder: InventoryBuilder) -> None:
    patterns = (
        (
            "node_type_const",
            re.compile(
                r"\b(?:NODE_TYPE|ITEM_NODE_TYPE)\s*:\s*[^=;\n]+\s*=\s*[r#]*[\"']([^\"']+)[\"']"
            ),
        ),
        (
            "node_defaults_macro",
            re.compile(r"\bimpl_node_defaults!\s*\([^,]+,\s*[\"']([^\"']+)[\"']"),
        ),
        (
            "node_inspector_registration",
            re.compile(r"\bregisterNodeInspector\s*\(\s*[\"']([^\"']+)[\"']"),
        ),
        (
            "serialized_node_type",
            re.compile(r"[\"']node_type[\"']\s*:\s*[\"']([^\"']+)[\"']"),
        ),
        (
            "node_type_attribute",
            re.compile(
                r"#\s*\[\s*node_type\s*(?:=|\()\s*[\"']([^\"']+)[\"']"
            ),
        ),
        (
            "node_macro_attribute",
            re.compile(
                r"#\s*\[\s*(?:[A-Za-z0-9_]+::)*node\s*\(\s*[\"']([^\"']+)[\"']"
            ),
        ),
    )
    for method, pattern in patterns:
        for match in pattern.finditer(text):
            builder.add(
                Discovery(
                    "node_type",
                    match.group(1),
                    method,
                    path,
                    _line_number(text, match.start()),
                    "registered" if "registration" in method else "declared",
                )
            )


def _scan_anodes(path: str, text: str, builder: InventoryBuilder) -> None:
    if _is_test_path(path):
        return
    lowered = path.casefold()
    if "anode" not in lowered and "alchemist" not in lowered and "ANode" not in text:
        return
    implementations = re.compile(
        r"\bimpl(?:\s*<[^>{}]*>)?\s+(?:[A-Za-z0-9_:]+)?ANode"
        r"(?:\s*<[^>{}]*>)?\s+for\s+([A-Za-z0-9_]+)"
    )
    for match in implementations.finditer(text):
        type_name = match.group(1)
        node_type = _associated_const(text, type_name, "NODE_TYPE")
        builder.add(
            Discovery(
                "anode",
                node_type or type_name,
                "anode_trait_implementation",
                path,
                _line_number(text, match.start()),
                "declared",
                {"rust_type": type_name, "node_type": node_type},
            )
        )
    for pattern, method in (
        (
            re.compile(r"\bregister_anode(?:_type)?\s*::\s*<([A-Za-z0-9_]+)>"),
            "anode_registration",
        ),
        (
            re.compile(
                r"\bregister_anode(?:_type)?\s*\(\s*[\"']([^\"']+)[\"']"
            ),
            "anode_registration",
        ),
        (re.compile(r"\bimpl_anode!\s*\(\s*([A-Za-z0-9_]+)"), "anode_macro"),
    ):
        for match in pattern.finditer(text):
            builder.add(
                Discovery(
                    "anode",
                    match.group(1),
                    method,
                    path,
                    _line_number(text, match.start()),
                    "registered" if method == "anode_registration" else "declared",
                )
            )
    for function_match in re.finditer(
        r"\b(?:pub\s+)?(?:const\s+)?fn\s+(?:type_name|type_id)\s*\([^)]*\)[^{]*\{",
        text,
    ):
        opening = text.find("{", function_match.start())
        block = _balanced_block(text, opening)
        if block is None:
            continue
        for arm in re.finditer(
            r"\bSelf::([A-Za-z0-9_]+)\s*=>\s*[r#]*[\"']([^\"']+)[\"']",
            block[0],
        ):
            builder.add(
                Discovery(
                    "anode",
                    arm.group(2),
                    "registered_anode_kind_type_name",
                    path,
                    _line_number(text, opening + arm.start()),
                    "registered" if "ANodeRegistry" in text else "declared",
                    {"enum_variant": arm.group(1)},
                )
            )
        constants = {
            constant.group(1): constant.group(2)
            for constant in re.finditer(
                r"\b([A-Z][A-Z0-9_]*)\s*:\s*[^=;\n]+\s*=\s*[r#]*[\"']([^\"']+)[\"']",
                text,
            )
        }
        for arm in re.finditer(
            r"\bSelf::([A-Za-z0-9_]+)\s*=>\s*([A-Z][A-Z0-9_]*)(?:\s*,|\s*})",
            block[0],
        ):
            type_id = constants.get(arm.group(2))
            if type_id is None:
                continue
            builder.add(
                Discovery(
                    "anode",
                    type_id,
                    "registered_anode_kind_type_id",
                    path,
                    _line_number(text, opening + arm.start()),
                    "registered" if "ANodeRegistry" in text else "declared",
                    {"enum_variant": arm.group(1), "constant": arm.group(2)},
                )
            )
    for match in re.finditer(
        r"\bANodeTypeId\s*::\s*(?:new|from_static)\s*\(\s*[\"']([^\"']+)[\"']",
        text,
    ):
        builder.add(
            Discovery(
                "anode",
                match.group(1),
                "anode_type_id_declaration",
                path,
                _line_number(text, match.start()),
                "declared",
            )
        )


def _scan_modules(path: str, text: str, builder: InventoryBuilder) -> None:
    if _is_test_path(path) or "/module" not in f"/{path.casefold()}":
        return
    for match in re.finditer(
        r"\b(?:pub(?:\([^)]*\))?\s+)?struct\s+([A-Za-z0-9_]*Module)\b", text
    ):
        type_name = match.group(1)
        node_type = _associated_const(text, type_name, "NODE_TYPE") or _associated_const(
            text, type_name, "ITEM_NODE_TYPE"
        )
        node_type = node_type or _preceding_node_attribute(text, match.start())
        builder.add(
            Discovery(
                "module",
                node_type or type_name,
                "module_node_declaration",
                path,
                _line_number(text, match.start()),
                "declared",
                {"rust_type": type_name, "node_type": node_type},
            )
        )
    for match in re.finditer(
        r"\b(?:register|declare)_module!\s*\(\s*(?:[A-Za-z0-9_]+\s*,\s*)?"
        r"[\"']([^\"']+)[\"']",
        text,
    ):
        builder.add(
            Discovery(
                "module",
                match.group(1),
                "module_registry_macro",
                path,
                _line_number(text, match.start()),
                "registered",
            )
        )


def _script_scope(path: str) -> str:
    stem = Path(path).stem
    return stem.removesuffix("_template") or "global"


def _scan_script_surfaces(path: str, text: str, builder: InventoryBuilder) -> None:
    lowered = path.casefold()
    if "script" not in lowered and "script" not in text.casefold():
        return
    scope = _script_scope(path)
    for match in re.finditer(
        r"\b(?:register_fn|register_native_fn|set_native_fn)\s*\(\s*[\"']([^\"']+)[\"']",
        text,
    ):
        builder.add(
            Discovery(
                "script_method",
                f"{scope}::{match.group(1)}",
                "script_host_registration",
                path,
                _line_number(text, match.start()),
                "registered",
                {"method": match.group(1), "scope": scope},
            )
        )
    for match in re.finditer(
        r"\b([A-Z][A-Z0-9_]*CALLBACK[A-Z0-9_]*)\s*:\s*[^=;\n]+\s*="
        r"\s*[r#]*[\"']([^\"']+)[\"']",
        text,
    ):
        builder.add(
            Discovery(
                "script_callback",
                match.group(2),
                "callback_constant",
                path,
                _line_number(text, match.start()),
                "declared",
                {"constant": match.group(1)},
            )
        )
    script_template_source = (
        "script_templates/" in lowered
        or ("script" in lowered and "template" in Path(path).name.casefold())
    ) and Path(path).suffix.casefold() in SCRIPT_TEMPLATE_SUFFIXES
    snippet_path = "snippet" in lowered
    if script_template_source and not snippet_path:
        builder.add(
            Discovery(
                "script_template",
                path,
                "template_file",
                path,
                1,
                "file_discovered",
                {"scope": scope},
                stable_name=_stable_product_path(path),
            )
        )
    if script_template_source:
        for match in re.finditer(
            r"(?m)^\s*(?://\s*)?(?:export\s+)?(?:async\s+)?function\s+"
            r"([A-Za-z_$][\w$]*)\s*\(",
            text,
        ):
            method = match.group(1)
            kind = (
                "script_callback"
                if method.casefold().startswith("on")
                else "script_method"
            )
            builder.add(
                Discovery(
                    kind,
                    f"{scope}::{method}",
                    "template_function",
                    path,
                    _line_number(text, match.start()),
                    "declared",
                    {"method": method, "scope": scope},
                )
            )
    if snippet_path:
        builder.add(
            Discovery(
                "script_snippet",
                path,
                "snippet_file",
                path,
                1,
                "file_discovered",
                stable_name=_stable_product_path(path),
            )
        )
    for match in re.finditer(
        r"\b([A-Z][A-Z0-9_]*SNIPPET[A-Z0-9_]*)\s*:\s*[^=;\n]+\s*="
        r"\s*[r#]*[\"']([^\"']+)[\"']",
        text,
    ):
        builder.add(
            Discovery(
                "script_snippet",
                match.group(1),
                "snippet_constant",
                path,
                _line_number(text, match.start()),
                "declared",
                {"preview": match.group(2)[:120]},
            )
        )


def _formula_name(path: Path) -> tuple[str, dict[str, Any]]:
    facts: dict[str, Any] = {}
    name = path.stem
    if path.suffix.casefold() == ".json":
        try:
            value = json.loads(_read_text(path))
        except (OSError, json.JSONDecodeError):
            value = None
        if isinstance(value, Mapping):
            for key in ("label", "name", "title", "formula_name"):
                if isinstance(value.get(key), str) and value[key].strip():
                    name = value[key].strip()
                    facts["declared_name_field"] = key
                    break
    return name, facts


def _is_fixture_path(relative: str) -> bool:
    return any(part.casefold() in FIXTURE_DIR_NAMES for part in Path(relative).parts)


def scan_product(root: Path) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    """Return deterministic product-surface and product-file entries."""
    builder = InventoryBuilder()
    file_entries: list[dict[str, Any]] = []
    registered_panel_components: set[str] = set()
    panel_component_candidates: list[tuple[str, Path]] = []
    for relative, path in _iter_files(root):
        lowered = relative.casefold()
        suffix = path.suffix.casefold()
        if path.name.endswith("Panel.svelte"):
            panel_component_candidates.append((relative, path))
        is_fixture = _is_fixture_path(relative)
        is_asset = any(
            marker in f"/{lowered}"
            for marker in ("/assets/", "/icons/", "/public/", "/static/")
        )
        if is_fixture or is_asset:
            file_entries.append(
                {
                    "id": (
                        f"{'fixture' if is_fixture else 'asset'}/"
                        f"{_stable_product_path(relative)}"
                    ),
                    "kind": "fixture" if is_fixture else "asset",
                    "name": path.name,
                    "path": relative,
                    "bytes": len(_canonical_bytes(path)),
                    "sha256": _sha256(path),
                }
            )
        formula_path = (
            "formula" in lowered
            and suffix in FORMULA_SUFFIXES
            and not _is_test_path(relative)
            and not relative.endswith(("package.json", "tsconfig.json"))
        )
        if formula_path:
            formula_name, facts = _formula_name(path)
            facts.update({"file": relative, "sha256": _sha256(path)})
            builder.add(
                Discovery(
                    "formula",
                    formula_name,
                    "formula_file",
                    relative,
                    1,
                    "file_discovered",
                    facts,
                )
            )
        if suffix not in TEXT_SUFFIXES:
            continue
        text = _read_text(path)
        registered_panel_components.update(
            _scan_registered_panels(relative, text, builder)
        )
        _scan_commands(relative, text, builder)
        _scan_node_types(relative, text, builder)
        _scan_anodes(relative, text, builder)
        _scan_modules(relative, text, builder)
        _scan_script_surfaces(relative, text, builder)
    for relative, path in panel_component_candidates:
        component = path.stem
        if component in registered_panel_components:
            continue
        builder.add(
            Discovery(
                "panel",
                component,
                "panel_component_file",
                relative,
                1,
                "file_discovered",
                {"registration": "not_discovered"},
            )
        )
    return builder.entries(), sorted(
        file_entries, key=lambda item: (item["kind"], item["path"])
    )

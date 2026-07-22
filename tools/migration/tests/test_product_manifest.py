from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from tools.migration.product_manifest import (
    ManifestError,
    check_documents,
    generate_documents,
    validate_schema,
    write_documents,
)
from tools.migration.product_manifest.discovery import _stable_product_path


class ProductManifestTests(unittest.TestCase):
    def test_monorepo_relocations_preserve_phase0_capability_identity(self) -> None:
        self.assertEqual(
            _stable_product_path("apps/chataigne/test-samples/test_perf.noisette"),
            "test-samples/test_perf.noisette",
        )
        self.assertEqual(
            _stable_product_path(
                "packages/golden-ui/style/icons/parameter/control/manual.svg"
            ),
            "src-ui/src/lib/golden_ui/style/icons/parameter/control/manual.svg",
        )
        self.assertEqual(
            _stable_product_path(
                "apps/chataigne/ui/src/lib/assets/icons/formula_library.svg"
            ),
            "src-ui/src/lib/golden_alchemist_ui/icons/formula_library.svg",
        )
        self.assertEqual(
            _stable_product_path(
                "apps/chataigne/src/module/script_templates/spatializer_module.js"
            ),
            "src/module/script_templates/spatializer.js",
        )
        self.assertEqual(
            _stable_product_path(
                "apps/chataigne/src/module/script_templates/artnet_module.js"
            ),
            "src/module/script_templates/artnet_module.js",
        )

    def fixture_repo(self, root: Path) -> None:
        (root / "docs/product").mkdir(parents=True)
        (root / "docs/product/source-imports.v1.json").write_text(
            json.dumps(
                {
                    "schema_version": 1,
                    "entries": [
                        {
                            "path": "former/shared-source",
                            "gitlink": "0" * 40,
                            "parent": "phase0-baseline",
                            "url": "https://example.invalid/shared-source.git",
                            "branch": None,
                        }
                    ],
                }
            ),
            encoding="utf-8",
        )
        (root / "apps/chataigne/ui/src/routes").mkdir(parents=True)
        (root / "apps/chataigne/ui/src/routes/+page.svelte").write_text(
            """
<script lang="ts">
const userPanels: UserPanelDefinitionMap = {
  modules: {
    title: 'Modules',
    component: ModulePanel
  }
};
registerCommandHandler('view.frame', () => true);
registerNodeInspector('module', { component: ModuleInspector });
</script>
""".strip(),
            encoding="utf-8",
        )
        (root / "apps/chataigne/src/module/modules/demo").mkdir(parents=True)
        (root / "apps/chataigne/src/module/modules/demo/mod.rs").write_text(
            """
pub struct DemoModule;
impl DemoModule {
    pub const NODE_TYPE: &'static str = "demo_module";
}
pub struct DemoANode;
impl ANode for DemoANode {}
impl DemoANode {
    pub const NODE_TYPE: &'static str = "demo_anode";
}
const DEMO_CALLBACK: &str = "demoValueChanged";
fn install(engine: &mut Engine) { engine.register_fn("sendDemo", send_demo); }
""".strip(),
            encoding="utf-8",
        )
        (root / "apps/chataigne/src/module/script_templates").mkdir(parents=True)
        (root / "apps/chataigne/src/module/script_templates/demo_module.js").write_text(
            "function send(value) {}\nfunction onMessage(value) {}\n", encoding="utf-8"
        )
        (root / "apps/chataigne/src/module/script_snippets").mkdir(parents=True)
        (root / "apps/chataigne/src/module/script_snippets/demo.js").write_text(
            "sendDemo(1);\n", encoding="utf-8"
        )
        (root / "apps/chataigne/builtin_formulas").mkdir(parents=True)
        (root / "apps/chataigne/builtin_formulas/Action.json").write_text(
            json.dumps({"name": "Action", "nodes": []}), encoding="utf-8"
        )
        (root / "fixtures").mkdir()
        (root / "fixtures/canonical.json").write_text("{}\n", encoding="utf-8")
        (root / "test-samples").mkdir()
        (root / "test-samples/baseline.noisette").write_text(
            "baseline project\n", encoding="utf-8"
        )
        (root / "test-samples/baseline.noisette.backup").write_text(
            "recovery copy\n", encoding="utf-8"
        )
        (root / "apps/chataigne/ui/static/assets").mkdir(parents=True)
        (root / "apps/chataigne/ui/static/assets/icon.svg").write_text(
            "<svg/>\n", encoding="utf-8"
        )
        (root / "apps/chataigne/ui/build/_app/immutable/assets").mkdir(parents=True)
        (root / "apps/chataigne/ui/build/_app/immutable/assets/generated.css").write_text(
            "body {}\n", encoding="utf-8"
        )
        (root / ".kilo/worktrees/donor/src-ui/src/routes").mkdir(parents=True)
        (root / ".kilo/worktrees/donor/src-ui/src/routes/+page.svelte").write_text(
            "registerCommandHandler('donor.only', () => true);\n",
            encoding="utf-8",
        )

    def test_generation_is_deterministic_and_never_claims_parity(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture_repo(root)
            first = generate_documents(root)
            second = generate_documents(root)
            self.assertEqual(first, second)
            surfaces = first["product-surfaces.v1.json"]["entries"]
            self.assertNotIn("command/donor-only", {entry["id"] for entry in surfaces})
            kinds = {entry["kind"] for entry in surfaces}
            self.assertTrue(
                {
                    "panel",
                    "command",
                    "node_type",
                    "anode",
                    "module",
                    "formula",
                    "script_method",
                    "script_callback",
                    "script_template",
                    "script_snippet",
                }.issubset(kinds)
            )
            self.assertGreater(first["product-surfaces.v1.json"]["category_counts"]["anode"], 0)
            files = first["product-files.v1.json"]
            self.assertEqual(files["category_counts"]["fixture"], 2)
            self.assertNotIn(
                "asset/apps/chataigne/ui/build/_app/immutable/assets/generated.css",
                {entry["id"] for entry in files["entries"]},
            )
            self.assertIn(
                "fixture/test-samples/baseline.noisette",
                {entry["id"] for entry in files["entries"]},
            )
            self.assertNotIn(
                "fixture/test-samples/baseline.noisette.backup",
                {entry["id"] for entry in files["entries"]},
            )
            self.assertEqual(
                files["fixture_requirements"],
                [
                    {"id": "P50-L1", "status": "absent", "matches": []},
                    {"id": "P5-L127", "status": "absent", "matches": []},
                ],
            )
            rows = first["functional-parity.v1.json"]["rows"]
            self.assertTrue(rows)
            for row in rows:
                self.assertEqual(row["migration_state"], "baseline")
                self.assertEqual(row["verification_state"], "pending")
                self.assertEqual(row["evidence"]["status"], "pending")
                self.assertEqual(row["user_workflow"]["status"], "pending_characterization")
                self.assertFalse(row["discovered_facts"]["discovery_is_behavioral_proof"])

    def test_generation_is_independent_of_checkout_line_endings(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            lf_root = root / "lf"
            crlf_root = root / "crlf"
            self.fixture_repo(lf_root)
            self.fixture_repo(crlf_root)

            for checkout, newline in ((lf_root, b"\n"), (crlf_root, b"\r\n")):
                for path in checkout.rglob("*"):
                    if not path.is_file():
                        continue
                    content = path.read_bytes()
                    try:
                        content.decode("utf-8")
                    except UnicodeDecodeError:
                        continue
                    normalized = content.replace(b"\r\n", b"\n").replace(b"\r", b"\n")
                    path.write_bytes(normalized.replace(b"\n", newline))

            self.assertEqual(generate_documents(lf_root), generate_documents(crlf_root))

    def test_drift_check_detects_source_changes(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture_repo(root)
            output = root / "docs/product/manifests"
            documents = generate_documents(root)
            write_documents(output, documents)
            self.assertEqual(check_documents(output, documents), [])
            page = root / "apps/chataigne/ui/src/routes/+page.svelte"
            page.write_text(
                page.read_text(encoding="utf-8")
                + "\nregisterCommandHandler('view.home', () => true);\n",
                encoding="utf-8",
            )
            changed = generate_documents(root)
            self.assertIn("changed: product-surfaces.v1.json", check_documents(output, changed))
            self.assertIn("changed: functional-parity.v1.json", check_documents(output, changed))

    def test_schema_validation_rejects_missing_required_property(self) -> None:
        schema = {
            "type": "object",
            "additionalProperties": False,
            "required": ["state"],
            "properties": {"state": {"type": "string", "enum": ["pending"]}},
        }
        with self.assertRaisesRegex(ManifestError, "missing required property"):
            validate_schema({}, schema)
        with self.assertRaisesRegex(ManifestError, "not an allowed value"):
            validate_schema({"state": "complete"}, schema)

    def test_generated_parity_schema_rejects_false_completion(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture_repo(root)
            documents = generate_documents(root)
            parity = documents["functional-parity.v1.json"]
            schema = documents["schemas/functional-parity-v1.schema.json"]
            parity["rows"][0]["verification_state"] = "complete"
            with self.assertRaisesRegex(ManifestError, "not an allowed value"):
                validate_schema(parity, schema)

    def test_canonical_performance_fixture_presence_is_explicit(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture_repo(root)
            (root / "test-samples/P50-L1.noisette").write_text(
                "performance fixture\n", encoding="utf-8"
            )
            requirements = generate_documents(root)["product-files.v1.json"][
                "fixture_requirements"
            ]
            self.assertEqual(
                requirements[0],
                {
                    "id": "P50-L1",
                    "status": "present",
                    "matches": ["test-samples/P50-L1.noisette"],
                },
            )
            self.assertEqual(requirements[1]["status"], "absent")

    def test_migration_python_sources_stay_below_line_limit(self) -> None:
        migration_root = Path(__file__).resolve().parents[1]
        oversized = {
            path.relative_to(migration_root).as_posix(): len(
                path.read_text(encoding="utf-8").splitlines()
            )
            for path in migration_root.rglob("*.py")
            if len(path.read_text(encoding="utf-8").splitlines()) >= 1000
        }
        self.assertEqual(oversized, {})


if __name__ == "__main__":
    unittest.main()

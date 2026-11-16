# M14/5 Composition enabled metadata

Updated 2026-11-16. M14/5 remains Active and unaccepted. This checkpoint implements the
MapGyver behavior but retains a reproduced product-browser history failure.

Placement no longer owns an Enabled Parameter. Child media metadata supplies the read
projection; ConfigureInstance edits that metadata in the same transaction as placement
Parameters. Capture uses effective media enablement. Disabled children remain in queries
and retain Inspector/viewport controls. A shared source stays demanded until its last enabled
consumer leaves, and re-enable restores demand. Copy/reload preserve the metadata.

MapGyver media menus now provide Enable/Disable through the public metadata intent, with
session/generation/UUID guards. Golden's existing projection helper reads ordinary-node
enabled metadata as well as Parameter metadata; value reads remain Parameter-only and
retain their event/generation fences. No Chataigne feature is included.

**Schema change:** Placement is v3. Earlier Placement versions reject explicitly, including
v2's duplicate Enabled Parameter; no migration is supplied. This prevents silently losing
a previous authored enable choice. Rust-generated InstanceParameterBindings no longer
advertises an Enabled Parameter UUID, and consumers are updated together.

Verified software:

- Composition/caller/geometry/render/deep-forest cohort:65 passed.
- Retained-source/final-use policy:7 passed; shared Reference pass-through:1 passed, using
  the same compiled backend test binary. The first direct launch lacked the app DLL path
  and ran no tests; the corrected process PATH includes target/debug. No budgets changed.
- Shared projection:13 passed; app projection/media-tree:3 passed.
- Pixel fixture software/oracle:7 passed,1 explicit hardware test ignored. Its two new
  enabled states bring the pending both-GPU fixture to19 states.
- Rust-owned protocol generation, strict app/test Clippy, MapGyver and shared-consumer UI
  typechecks, scoped Prettier and ordinary default app build pass. Root/Core formatting pass.

Product browser runs verify absence of duplicate Enabled, Inspector disable/re-enable,
editing Position while disabled, retained off-page selection, menu disable/re-enable and
restoration to the initial scene. The subsequent existing Position Undo/Redo assertion fails:
Redo restores the earlier disabled state instead of the completed Position change. Waiting
for the explicit edit-session boundary does not resolve it. Remaining later assertions in
that run are unexecuted, not passing. Both owned hosts exit normally.

Receipts: `target/m14-composition-enabled-tests-final-2026-11-16.log`,
`target/m14-composition-enabled-product-ui-2026-11-16.log` and
`target/m14-composition-enabled-product-ui-final-2026-11-16.log`. Tested default binary SHA256:
`69bdc89400e92eb1009a0cdae008e9702d42387f668dcaaf7987aef2ec3282fe`.
Earlier compile/count failures are preserved; count corrections remove exactly one Parameter
per Placement, including two per deep-forest branch.

**Next action:** use the two existing UI receipts to identify and correct the history cause
on the actual MapGyver metadata/number-edit path, then rerun the unchanged full browser
contract. Baseline plus one corrective UI comparison are exhausted; do not add another
unchanged harness/profiler/environment variant. A source-supported correction must change
the product decision before another comparison. Native pixels remain pending the original
current Overlay Off prerequisite; do not reuse an old observation or close M14/5 early.

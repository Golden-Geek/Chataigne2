# M14/5 Composition enabled metadata

Updated 2026-11-16. M14/5 is Accepted after the full software/product-browser contract and
both-GPU native pixel qualification. Together with M14/4, overall acceptance is90/106 (85%);
M14 is2/10 (20%). M14/3 Composition actions/order is the next active product deliverable.

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

- Composition/caller/geometry/render/deep-forest cohort:66 passed, including the exact
  metadata/Redo-branch/component/duplicate-commit regression.
- Retained-source/final-use policy:7 passed; shared Reference pass-through:1 passed, using
  the same compiled backend test binary. The first direct launch lacked the app DLL path
  and ran no tests; the corrected process PATH includes target/debug. No budgets changed.
- Shared projection:13 passed; app projection/media-tree:3 passed (unchanged scope retained).
  The complete shared session suite now passes82 tests across11 files.
- Pixel fixture software/oracle:7 passed,1 explicit hardware test ignored. Its two new
  enabled states bring the both-GPU fixture to19 states, now natively verified.
- Rust-owned protocol generation, strict app/test Clippy, MapGyver and shared-consumer UI
  typechecks, scoped Prettier and ordinary default app build pass. Root/Core formatting pass.

The full ordinary-product browser contract now passes: absence of duplicate Enabled,
Inspector/menu disable/re-enable, editing while disabled, off-page selection, Position
Undo/Redo, zero-scale recovery, independent panels, copy/removal history, layout reload,
reconnect, held-pointer New and same-document Load. Page errors/rejected requests are empty;
the owned host exits0. These are software browser checks, not physical/native pixels.

The earlier Redo failure exposed write admission at Golden's session boundary. History actions
wait behind edit barriers, but a new ordinary Parameter write could dispatch immediately when
preview admission returned null. It could overtake the pending Undo actions. Waiting for no
active edit session could not fix this: typed vector text does not start a gesture session.

New ordinary Parameter writes and metadata patches now capture and await the already-queued
barrier before dispatch, then recheck document/target identity. They preserve the ordering of
history requested later and do not block already-admitted confirmed work from draining. Preview
capacity fallback, backend validation, event policy and history budgets remain unchanged.
Focused tests prove both request orders and cancellation across document replacement. The
backend regression passes independently, so no engine-history change was needed.

Current receipts: `target/m14-history-admission-composition-tests-2026-11-16.log`,
`target/m14-history-admission-session-tests-2026-11-16.log`, strict lint/default-build/type
logs with the same prefix, and `target/m14-history-admission-product-ui-2026-11-16.log`
(SHA256 `32a8e60195150082df01100f599ebe6422c3fb454b8ca413e372a786fb2d3066`).
Tested default binary SHA256:
`b1b6221687537f8ddeac5448aa9ea4e4663056138a0a266e08f4c28737938b24`.
Both earlier failing product logs remain retained under `m14-composition-enabled-product-ui`.
Earlier compile/count failures are preserved; count corrections remove exactly one Parameter
per Placement, including two per deep-forest branch.

**Native acceptance:** Intel(R) Graphics and NVIDIA GeForce RTX5080 Laptop GPU complete the
existing19-state fixture in17.30s:114 independent images/87,552 texels through two outputs and
the production preview compositor. Disabled/restored child metadata suppresses/restores the
correct content while shared siblings remain valid. Maximum residual1 byte is within the
unchanged2-byte bound. All renderer plan allocations balance, source/output inventories empty
and readback pools return idle under the original120-second watchdog. See the
[receipt](evidence/mapgyver-m14-composition-acceptance-2026-11-16.json) for hashes and identities.
The user checked Overlay Off and established standing Off on2026-11-16; future cohorts use
that provenance until contradicted. No helper repair or recurring question is required.
M14/3 is next; all separate cadence, stress, growth, device recovery and final gates remain.

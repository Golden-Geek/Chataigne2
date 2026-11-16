# M14/3 Composition actions and ordering

MapGyver owns placement actions and authored child order. Golden supplies typed Parameters,
history, the ordinary node menu and canvas toolbar. Actions, front-first ordering and native
overlap pixels are verified. M14/3 is Accepted after correcting the Video-status lifecycle read
and completing the original seven-type creation series below.

The current product slice replaces Copy/Delete toolbar buttons with Fill/Fit and extends the
ordinary node menu with Transform → Reset/Fit/Fill. Copy/Duplicate/Delete remain standard node
controls. `CompositionEdit::TransformInstance` resolves canvas/source dimensions in the backend
and writes only transform Parameters in one transaction. Reset centers native size; Fit uses
uniform scale; Fill stretches to the whole canvas. All reset rotation and pivot. Opacity,
blend, enablement and source remain intact, including on collapsed or disabled media.

The typed Media Command and named `transformCompositionMedia` script method share this operation
and the existing committed callback path; Rust generates the protocol and command field list.
Menu actions use bounded backend ownership queries and session/generation/UUID fences.
See [spatial geometry ownership](../architecture/mapgyver-spatial-geometry.md).

Front/Back toolbar and ordinary Arrange menus now use `ArrangeInstance`, its typed Media Command
and named `arrangeCompositionMedia` method. Actual authored siblings are front-first; Golden's
back-to-front layer contract stays unchanged. Backend `selected_index` places an independent
selection on the correct side of a bounded page. Known pointer selection projects immediately
in the controller before a held gesture suspends reads. Existing projects now render their
first child on top; no migration preserves the former stacking interpretation. Composition
admits media children, including nested Compositions, rather than folders; foreign owners reject.

Verified transform checkpoint604ba10f on2026-11-16:

- 69 Composition/caller/geometry/render tests, the all-command-kinds test,3 source API tests
  and the generated-contract comparison pass. All three transform actions match UI, direct
  QuickJS, hosted generic QuickJS and named-method callers. Unknown dimensions reject atomically;
  Reset remains available, and exact history/reload preserve unrelated state.
- Strict app/library/test Clippy, both UI consumer typechecks, scoped Prettier, ordinary app
  build and root/Core formatting pass. No Chataigne feature was changed.
- The complete ordinary Composition browser contract passes with normal host exit0: toolbar
  Fill/Fit, menu Reset with Undo/Redo, metadata/zero-scale regressions,119 bounded reads,
  independent panels, menu duplication/deletion, reconnect, layout/project and held-pointer lifetime.
  Page errors and rejected requests are empty. `--parameter-ui` independently passes with
  ten distinct typed Parameter bindings, independent copy identities, and both expected
  scalar/vector rejection checks.

Transform-checkpoint product SHA256: `8f7b9c2a540c5c0575a3c7e8d3254efe334f2370fce3124494f1cfc8f6d79ab7`.
Receipts: `target/m14-transform-composition-tests-final-2026-11-16.log`, command/source API/
contract/Clippy/type/default-build logs with the same prefix, and:

- `target/m14-transform-ui-accepted-slice-2026-11-16.log`, SHA256
  `4644321ed80fd9d96c48b0aaf1386d9034bb5312293c437e0854a34ba6ad0aa7`.
- `target/m14-transform-parameter-ui-complete-2026-11-16.log`, SHA256
  `0fb3dac3767adbbea5233823f2b9398df53cb347983b46f55a08194709a413b2`.

The earlier named-method arity omission was corrected. Browser fixture reconciliation preserves
the current seven media types, real MouseEvent coordinates, busy-state admission and ordinary
menu shortcut labels. Overlapping duplicates are selected by their explicit keyboard target;
existing pointer-down/hit checks remain. Copy verifies a fresh owned output channel rather than
requiring the original child's channel UUID. Zero is valid; the rejected Scale value is a finite
f64 that overflows the f32 render transform. No product validation or numerical budget changed.

**Current ordering cohort:**73 Composition/backend/caller tests,18 editor/picking tests,
all-command1/source API3/generated-contract1, strict app/test lint, both UI consumer checks,
scoped Prettier, root/Core formatting and the ordinary app build pass. The full original browser
contract, extended with order actions and overlapping primary-down hits, passes with139 bounded
queries, no page errors/rejections and normal host exit0. The held-pointer regression is fixed
at the controller's cached selection boundary; subsequent gestures wait for the UI edit group
to finish, rather than mistaking an early backend idle read for terminal admission.

Both Intel and NVIDIA pass27 native states/162 independent images/124,416 texels in31.04s:
two outputs and the production preview compositor, arrangement Undo/Redo, Reset/Fit/Fill,
nested blends, zero/enable recovery, source removal/Undo and reload. Maximum residual1 byte
is within the unchanged2-byte bound. Native inventories balance/empty, payload pools return
idle, and the120-second watchdog is unchanged. User standing Overlay Off applies.
The [ordering receipt](evidence/mapgyver-m14-composition-order-2026-11-16.json) records identities,
hashes and exact scope. This is finite raster/input behavior, not cadence, physical image/control
alignment, stress, driver recovery or private-growth qualification. Other owning gates remain.

**Lifecycle correction:** `InspectVideoSource` now returns a generated nullable status for
retired media/channel UUIDs. Live wrong-kind and foreign-channel requests still reject.
The existing reader clears readiness on authoritative absence and preserves presentation
when an active edit defers a read. Removal/Undo/New/Load, immutable-query/operation parity,
all four public callers and late-reader fences are covered without reviving retained decoders.

110 Video tests pass sequentially (six existing hardware tests ignored), seven reader/operation
tests pass, and strict app/test lint, generated contract/command checks and both UI typechecks
pass. The current native executable again passes all 27 states/162 images on both GPUs in
24.25 seconds; original raster/resource budgets remain unchanged.

**Parallel software return:** the default parallel Video suite is not green. One run fails the
existing typed-policy generic QuickJS callback count; another fails the existing timed-column
preload `!idle.is_current()` assertion. Both exact cases pass in the sequential 110-test run;
their cause is unproven. Preserve `target/m14-video-status-tests-final-2026-11-16.log` and
`target/m14-video-status-tests-complete-2026-11-16.log` for source-supported correction and the
original parallel rerun before M09. No full parallel-suite or final qualification pass is claimed.

**Historical failure:** the seven-type creation function passed once in
`target/m14-transform-product-all-ui-2026-11-16.log`, but that aggregate command later failed
on a parameter-fixture name collision (corrected with an explicit fixture label). A subsequent
creation run completes the authored operations but records one unexpected background
`InspectVideoSource` rejection, `media reference is missing`, after lifecycle edits.
`target/m14-transform-composition-ui-complete-2026-11-16.log` SHA256
`b296716743b1d1ad76bf96cd7eb855728989b5a8584cbb9b4a1bb6d82f52b93b` preserves that failure.
The failure remains historical evidence; the nullable read correction above resolves it.

**Acceptance:** the rebuilt ordinary app passes the unchanged combined Composition review,
editor and seven-type creation contracts, with 139 bounded panel queries, empty page errors and
rejected-request lists, and normal host exit0. Reference, Solid Color, Picture, Video, Shared
Texture, Composition and Matrix creation, placement, copy/reorder/delete, Undo/Redo and project
identity pass. The explicit default-build exit is0; root/Core formatting and scoped Prettier pass.
Together with the preserved transform/order caller, Inspector, menu, picking and native proofs,
this closes M14/3. See the [acceptance receipt](evidence/mapgyver-m14-actions-acceptance-2026-11-16.json).
Overall becomes91/106 (86%), M14 3/10. Parallel-suite and unrelated qualification debt remain
open; no cadence, physical alignment or release-readiness claim follows from this finite gate.

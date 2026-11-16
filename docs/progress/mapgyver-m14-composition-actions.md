# M14/3 Composition actions and ordering

MapGyver owns placement actions and authored child order. Golden supplies typed Parameters,
history, the ordinary node menu and canvas toolbar. This gate stays Active until all actions,
front-first rendering/hits, public callers and native overlap pixels pass.

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

Verified on2026-11-16:

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

Current product SHA256: `8f7b9c2a540c5c0575a3c7e8d3254efe334f2370fce3124494f1cfc8f6d79ab7`.
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

**Retained failure:** the seven-type creation function passed once in
`target/m14-transform-product-all-ui-2026-11-16.log`, but that aggregate command later failed
on a parameter-fixture name collision (corrected with an explicit fixture label). A subsequent
creation run completes the authored operations but records one unexpected background
`InspectVideoSource` rejection, `media reference is missing`, after lifecycle edits.
`target/m14-transform-composition-ui-complete-2026-11-16.log` SHA256
`b296716743b1d1ad76bf96cd7eb855728989b5a8584cbb9b4a1bb6d82f52b93b` preserves that failure.
No complete creation-series or physical/native acceptance is claimed. Return after the ordering
slice, before M14/3 acceptance: reconcile the source-status read across removal/Undo/New/Load
at its existing Video query/footer boundary, then complete the unchanged creation contract.
Do not add another observer, profile or fixture variant.

Remaining gate scope: front/back buttons and Arrange menus, first-child-on-top hierarchy/
rendering/editor paint and hit agreement, public caller and history/copy/reload coverage for
ordering, supported nested ownership, mixed ratios, native overlapping alpha/order pixels and
the retained lifecycle read. M14/3 stays Active/unaccepted;90/106 gates and M14 2/10 are unchanged.

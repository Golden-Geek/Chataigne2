# M06/1 child-media Composition model

MapGyver owns Composition child-media structure, typed Placement, geometry and render projection.
Golden owns generic Parameters, subtree identity remapping, history and project codecs.
**M06/1 accepted 2026-11-03:** the complete structural/API/Parameter/history/project contract.
Native output, dependency/scale, new editor handles and independent panels retain M06/2–5.

Composition already contained real Media/Reference children from the accepted M04 ownership
foundation. This milestone completes its degree contract across all owning layers:

- `mapgyver.composition.placement.v2` stores the real `parameters/rotation_degrees` Parameter.
  The generic Inspector labels it Rotation and describes clockwise degrees. Negative angles and
  multiple turns retain their authored values; no normalization rewrites the document.
- Rust's `Transform2D.rotation_degrees` owns the generated TypeScript declaration. Parameter
  edits, public operations, geometry origins/replies, copied trees and saved projects all use
  degrees. Only transient trigonometry in geometry, canvas presentation and GPU mapping converts
  to radians. Renderer reference points and canvas hit/outline tests use explicit 90° cases.
- Placement v1 projects reject before Golden's recoverable codec can discard their nodes.
  Legacy operation field `rotation` rejects atomically rather than silently resetting an angle
  to zero. This is a deliberate schema/API break, with no compatibility migration.

| Original M06/1 requirement                   | Verified proof                                                                                                                                                                                                                                                                                                                                        |
| -------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Single output and actual ordered child media | Existing source catalog tests protect Composition's one fixed output. `composition/tests/owned_media.rs` verifies default Reference and independent Alt-copy drops, real parentage, all variants, nested identity remapping, replacement and project restoration. Canvas IDs are actual media UUIDs.                                                  |
| Real typed transforms                        | Eight Placement Parameter children own Output, Position, signed/nonuniform Scale, degree Rotation, Pivot, Opacity, Blend and Enabled. `tests/parameters.rs` verifies generic/direct writes, validation, no second opaque DTO, copied constraints/control subtrees and stable independent identities.                                                  |
| Structural copy/reorder/removal              | `tests/mod.rs` exercises atomic creation, independent settings, complete subtree duplication, ordering, rename/removal, Undo/Redo and sparse reload. Detached forest creation and existing batch limits are retained.                                                                                                                                 |
| Complete public callers                      | Current four-route Composition parity compares direct project/UI intents, QuickJS app operations, hosted `editMedia` and named/fallback source methods with nonzero degree transforms. The ordinary headless review exercises 30 absolute geometry samples grouped into three Undo/Redo gestures plus structural/rejection/source/project operations. |
| Parameter/history/versioned project          | New `tests/degrees.rs` verifies 90°, −450° and 720° Parameters, public projection, exact Undo/Redo/project identity, explicit prior-schema/field rejection, and inspection preserving an active degree edit and redo even for rejected reads.                                                                                                         |
| Actual generic UI                            | The bundled owned-media flow edits Position and Rotation=90 through real Inspector widgets, verifies independent peers and Undo/Redo, then saves/New/loads the actual child hierarchy and Composition state.                                                                                                                                          |

Checks:

- Rust source and generated DTOs were updated together using `generate_media_protocol`.
- `cargo test --locked -p MapGyver --features native-qualification --lib --test composition_rendering --test picture_rendering`:
  **344 library tests and 4 raster reference tests passed**, with native tests deliberately ignored.
  This includes generated-contract equality, public caller parity and known 90° renderer mappings.
- The additional active-edit/read/redo regression passed separately; the final affected Composition
  model suite passed **25 tests** after formatting. The corrected native-scene software suite passed **5 tests**.
- Strict all-test native-qualification Clippy, both Cargo formatters and diff checks passed.
- MapGyver Svelte check: **0 errors, 0 warnings**. Focused Composition/gesture/presentation and
  source-hit tests: **16 passed**.
- Rebuilt ordinary isolated `MapGyver-codex-review.exe` passed
  `tests/headless/media.mjs --composition-review --owned-media-ui` in **3.285 seconds**,
  including the generic degree edit, actual drops, source regression and project roundtrip.
  The browser and owned host exited normally. No user app process was stopped.

## Retained native gate and return action

Two native attempts of the existing full Composition matrix ran after compile/software/lint
success. Neither passed the native pixel gate. The first stopped at an obsolete fixture source
binding before GPU setup; the fixture now preserves the actual owned child's projected source,
with a software setup test. The second stopped at a stale producer-count assertion: 11 channel
metadata records include six automatic Reference aliases, while the original scene has five
actual producers. No native pixels were accepted from these attempts.

The shared native fixture now counts distinct produced targets, retaining the original required
count of **five**; its old assertion that all metadata records own distinct targets contradicted
Reference sharing. Existing software `demanded_nested_channels_render_once_for_shared_instances_and_outputs`
independently proves the distinction: seven metadata records, three render passes. Pixel scenes,
oracles, tolerances and runtime budgets remain unchanged. The corrected native assertion has
**not** been exercised on hardware. Attempts stop at the batch's two-run investigation limit.

M06/3 remains Pending. Return after M06/2 dependency/sharing acceptance and a software check of
the current compiled five-producer scene, then run the retained full native matrix. Carry this
cumulative two-attempt finding forward; do not reset it by renaming the batch. M06/4 still needs
the complete retained pointer/handle/history fixture port and revised anchors; the legacy browser
fixture's unit fields were updated here, but it is not accepted. M06/5 retains bounded independent
follow/lock panels and removal of internal controls. No physical, timing or release gate closes.

Local receipts: `target/mapgyver-composition-degrees-{software,final-model,fixture-software,clippy,svelte,ui-tests,bundled,native}.log`
and `target/mapgyver-composition-inspect-baseline.log`. The checked-in tests preserve assertions;
transient logs are not product assets.

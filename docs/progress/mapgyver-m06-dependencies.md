# M06/2 Composition dependencies and shared production

MapGyver owns all-media dependency admission, Reference resolution and compiled channel plans.
Golden owns the document, history, worker execution and GPU resources. **M06/2 accepted
2026-11-03** against `c09cf57c` plus the compiled-topology regression below. No runtime code or
qualification budget changed in this milestone.

| Original requirement                 | Current authoritative proof                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| ------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| All-media dependency/cycle admission | `media/dependencies.rs` builds Composition/Matrix edges, merges outgoing retained runtime edges and canonicalizes Reference chains. Current tests cover direct/mixed Reference/Composition feedback, Matrix-owned Reference feedback, missing-channel retarget feedback, generic Parameter writes and atomic final-graph admission. Rejection preserves project state and history.                                                                                                                                                                                                                         |
| Missing targets, deletion and Undo   | `dependencies/tests::every_media_kind_remains_a_reference_after_source_removal_and_undo` exercises Solid Color, Picture, Shared Texture, Composition and Matrix sources twice in a Composition. Authored child references persist through missing/deletion/Undo/Redo, without source duplication. Capture guards invalidate on removal/recovery. Rendering tests omit unavailable layers while healthy peers remain.                                                                                                                                                                                       |
| Nested/deep/large graphs             | The current suite passes a 100,000-node dependency validator on a 128 KiB stack, including unresolved leaves, duplicate edges and a closing cycle. An actual 4,096-deep Composition dependency forest uses one detached document edit and one shared leaf, rejects feedback atomically, and validates sparse save/load with unchanged node counts. The 1,000-instance detached-forest/history test also passes. These are backend scope proofs; whole-app Q04 remains open.                                                                                                                                |
| Shared targets and output demand     | Existing nested tests prove seven metadata records compile to three producer passes and two outputs share one root. Automatic Reference aliases reuse source targets; explicit Resolution adds one shared resampling pass. Disabled instances withdraw source demand; unavailable Spout inputs warm without drawing. The new five-producer case below matches the native fixture's topology/extents.                                                                                                                                                                                                       |
| No source/decoder duplication        | Current source/Reference/Picture software tests preserve one shared producer and retained image identity. Accepted [Reference native evidence](evidence/mapgyver-reference-native-2026-11-03.json) exercises Solid Color/Picture/Shared Texture through Composition and Matrix on both GPUs, including shared targets/subscriptions/uploads, resampling, independent copies and retirement. [Current Picture recovery](mapgyver-m04-source-parity.md) additionally verifies shared uploads and no per-frame reupload. These unchanged source contracts retain their acceptance. Video decoders remain M10. |

The new test is
`media::rendering::tests::composition::native_scene_five_producers_share_six_aliases_and_recover_without_duplicate_passes`.
It uses three 1×1 procedural leaves, an 8×4 inner Composition and a 32×24 outer Composition,
with the same six automatic Reference aliases as the retained native pixel fixture. It asserts:

- **11 captured/compiled channel records**, **5 distinct targets**, **5 producing passes**.
  The set of pass outputs equals the target set, so each target has exactly one producer.
- Two independent output routes share the root target, with no external input textures.
- Source deletion fences the old capture, does not increase the five-pass bound, and leaves
  the healthy peer source in the inner Composition. Undo and sparse reload restore exact
  five-target/five-pass sharing.

This is new evidence for the named native-fixture decision: Reference metadata must not be
counted as extra rendered production. It supports the corrected distinct-target assertion from
M06/1 without increasing its original five-target limit. It does not prove native pixels.

Checks completed:

- `cargo test --locked -p MapGyver --features native-qualification --lib`:
  **346 passed, 0 failed, 2 explicit qualification tests ignored**, **11.61 seconds**.
  Includes the new compiled graph, 100k graph, 4,096-deep authored graph, all-media missing/Undo,
  mixed cycles, renderer/Picture sharing, Reference behavior and generated-contract checks.
- `cargo clippy --locked -p MapGyver --features native-qualification --lib --tests -- -D warnings`:
  passed. Both Cargo formatters and diff checks passed.
- No UI or runtime source changed; M06/1's actual bundled Inspector/drop/history/project checks
  remain valid. The separately qualified 10k Reference backend case is retained from M04/7;
  it was not rerun or counted among these 346 tests.

Receipts: `target/mapgyver-composition-dependencies-tests.log` and
`target/mapgyver-composition-dependencies-clippy.log`; the checked-in tests retain the assertions.

M06 is **2/5 accepted**. M06/3 becomes the sole Active gate after this milestone is pushed.
Its original full native matrix remains required. Carry the two prior stale-fixture attempts
forward; they accepted no native pixels. The new compiled-plan proof resolves the counter's
meaning and supplies the previously recorded return trigger, allowing the corrected fixed
matrix to resume with unchanged scenes, oracle, tolerances and budgets. M06/4's revised
pointer anchors and M06/5's bounded independent panels remain Pending. This does not accept
whole-app scale/performance, physical display or release qualification.

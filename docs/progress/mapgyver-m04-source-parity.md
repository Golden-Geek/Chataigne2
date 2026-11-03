# M04/6 source parity and Picture recovery

MapGyver owns source definitions, typed operations, asset identity and app preparation.
Golden owns script execution, history/persistence and native rendering resources.
**Accepted 2026-11-03**, completing M04's eight functional gates. The native receipt is
[source parity evidence](evidence/mapgyver-source-parity-2026-11-03.json).

| M04/6 requirement                    | Current proof                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| ------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Revised UI/script/headless semantics | `media/tests/scripting/parity.rs` compares direct project/UI intents, QuickJS app operations, hosted `editMedia` and named source methods. Source creation/allocation, media Resolution/Sampling, variant source fields, assets, reorder/removal, inspection and rejection preserve committed results, peer state and history. The current run also covers nullable missing-channel reads and wrong-owner rejection through all four routes.                                                                                                                        |
| Real bundled callers                 | The retained Picture reload fixture now selects the media and its real typed Channels child. It sends exactly one selected-asset reload, preserves catalog/identities and the preceding deletion's redo/undo, and reports no unexpected alerts, rejected requests or page errors. Actual script template/editor/callback and typed command configuration/firing/history checks pass in the same isolated ordinary app. M04/5 retains the full source authoring and generic widget matrix.                                                                           |
| Duplication, Undo/Redo and project   | New `media/tests/duplication.rs` exercises whole-media and individual-variant copies for Solid Color, Picture and Shared Texture: six cases. Copies receive fresh identities, preserve ordered variants and shared Resolution/Sampling, retain external Picture asset references, and edit independently. A copied media's Resolution can return to Automatic without changing its original. Exact Undo/Redo and sparse reload restore the expected catalogs. Existing caller tests also verify full and sparse project codecs and exclude runtime resource fields. |
| Missing/relinked/replaced Picture    | The existing native Picture matrix passes on Intel and RTX. A missing JPEG is unavailable while its PNG peer stays healthy, then recovers when the file appears. Asset relink and in-place replacement through public `ReloadAsset` update pixels without changing identity or authored reload history. Unsupported gamma rejects visibly and a valid JPEG relink recovers. In-place replacement uses explicit Reload; no file watcher is claimed.                                                                                                                  |
| Sharing and native recreation        | Three native consumers use first/first/second channels. Shared JPEG consumers reuse the retained upload; later frames do not upload the same image again. Sparse project reload into a fresh renderer reproduces 3840×2160 RGBA output and stable asset IDs. Observer pools, Picture workers and GPU resources retire before the fixture finishes. This is functional renderer recreation, not genuine device-removal qualification.                                                                                                                                |

Verification used the existing fixed matrix, with no tolerance or budget changes:

- Software/native-target baseline: `cargo test --locked -p MapGyver --features native-qualification --lib --test picture_rendering`
  passed **341 library tests**, with hardware tests ignored, and compiled the native executable.
- Final library after the additional duplication/caller cases: **342 passed, 2 hardware tests ignored**,
  including Rust-to-TypeScript contract equality. Strict library/all-test Clippy with the same
  feature set and `-D warnings` passed. Both required Cargo formatters and diff check passed.
- Ordinary bundled review app at `958d6002`, selected through `MAPGYVER_TEST_BINARY`, passed
  `node apps/mapgyver/tests/headless/media.mjs --ui-reload --media-script-ui --media-command-ui`
  in **6.43 seconds**, including source HTTP peer/history/Save/New/Load and clear rejection of an
  unsupported project schema. The owned app exited normally. Production code is unchanged in
  this acceptance milestone; prior M04/5 Svelte/widget checks remain valid.
- After terminal software/compile/lint success, the compiled `picture_rendering` executable ran
  `picture_pixels::authored_picture_pixels_recover_share_relink_replace_reject_color_and_reload_4k`
  with `--exact --ignored --nocapture --test-threads=1`: **1 passed in 45.69 seconds**, one native
  invocation. Both hardware adapters passed **54 complete output images / 33,177,836 texels**
  at the original **2/255 per-component tolerance**. The test process exited zero and no owned
  Picture test process remained.

Local receipts are `target/mapgyver-source-parity-ui.log`,
`target/mapgyver-source-parity-software.log`, `target/mapgyver-source-parity-final-software.log`,
`target/mapgyver-source-parity-clippy.log` and `target/mapgyver-source-parity-native.log`.
The checked-in tests and compact native receipt preserve the verification contract.

## M04 phase audit

| Gate                                           | Acceptance evidence                                                                                                                                                                                                               |
| ---------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1. Creation and complete source API            | [Source API](mapgyver-m04-source-api.md); current library and bundled script/command checks.                                                                                                                                      |
| 2. Native variants and shared media settings   | [Both-adapter variant matrix](mapgyver-m04-variants.md); unchanged renderer contracts.                                                                                                                                            |
| 3. Worker Picture bounds, color and retirement | Retained [asset/admission evidence](evidence/mapgyver-m03-picture-assets-2026-10-29.json) and accepted Picture matrix; current native recovery/alpha/4K/retirement result above. This gate was not reopened by the plan revision. |
| 4. Spout lifecycle and failure isolation       | [Complete both-adapter Spout acceptance](mapgyver-m04-spout.md).                                                                                                                                                                  |
| 5. Actual typed source UI                      | [Complete source UI](mapgyver-m04-source-ui.md), including bounded Channels and closed folder capabilities.                                                                                                                       |
| 6. Caller/Picture/project/resource parity      | This report and its native receipt.                                                                                                                                                                                               |
| 7. Reference media                             | [Complete functional Reference evidence](evidence/mapgyver-reference-native-2026-11-03.json).                                                                                                                                     |
| 8. Independent Media Preview                   | [Complete bundled/native preview evidence](evidence/mapgyver-media-preview-native-2026-11-03.json).                                                                                                                               |

M04 is **8/8 accepted**. Video remains M10; whole-app Q04, genuine GPU removal, timing/memory
qualification and the full physical display matrix remain in their owning gates. Next is M06/1's
complete child-media Composition/degree/structural/history/project contract, followed by its
remaining dependency/native/editor/panel gates.

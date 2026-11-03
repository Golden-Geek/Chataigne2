# M04/2 native source variants acceptance

MapGyver owns media/variant Parameters, captured source definitions and channel preparation.
Golden Video owns source workers, GPU plans, shared outputs and retirement. The native
acceptance test uses those production boundaries directly; it does not replace them with a
UI projection or null renderer.

**Accepted: 2026-11-02.** Revised M04/2 passed for Solid Color, Picture and Shared Texture on
both available hardware adapters, in one bounded native matrix after compile/software success.

| Hardware                           | Adapter identity                    | Verified source cases                |
| ---------------------------------- | ----------------------------------- | ------------------------------------ |
| Intel(R) Graphics                  | `AdapterId { low: 89313, high: 0 }` | Solid Color, Picture, Shared Texture |
| NVIDIA GeForce RTX 5080 Laptop GPU | `AdapterId { low: 90608, high: 0 }` | Solid Color, Picture, Shared Texture |

The owning test is
[`same_media_variants_share_resolution_and_preserve_native_identity_through_history_and_reload`](../../apps/mapgyver/tests/shared_texture/variant_resolution.rs),
under `channel_rendering::shared_texture::variant_resolution`. Each case creates two channels
in one actual media and three simultaneous native consumers: first, first, second. The
renderer exposes two distinct channel targets; the plan compiler routes repeated consumers to
the existing target rather than producing a third channel. Shared Texture has exactly two
native subscriptions; Picture/Solid Color have none.

| Requirement                              | Verified behavior                                                                                                                                                                                                                                       |
| ---------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Disabled Resolution                      | Picture and Shared Texture resolve distinct 2×2 and 3×2 source sizes. Both procedural variants resolve exactly 1920×1080.                                                                                                                               |
| Enabled media Resolution                 | One typed operation forces both channels to 3×2 and creates one history step. Undo restores their natural sizes; Redo restores both forced sizes.                                                                                                       |
| Independent content during shared sizing | Red/blue variants remain distinct at all three consumers. The second variant changes to cyan; Picture/Shared Texture inputs resize to 4×3 while both forced targets remain 3×2. Shared Texture waits for resized metadata and a new source publication. |
| Disable after source resize              | Both channels return to source-native extents, including 4×3 for the changed Picture/Shared Texture source; procedural variants return to 1920×1080.                                                                                                    |
| Stable ordering/history                  | Reorder changes the actual document order to second/first. Native channel UUIDs, targets, source bindings and content stay associated with their channel, including Undo/Redo.                                                                          |
| Project reload                           | Sparse save/load retains the reordered media/channel identities, settings and content. The loaded project publishes through the native renderer and passes pixel checks again.                                                                          |
| Sharing/pixels/cleanup                   | All three consumers pass red/blue/cyan comparisons within 2/255. Each renderer explicitly retires before the next case; owned producer processes exit successfully. No producer process remained after terminal success.                                |

Consumer output sizes are fixed at 2×2, 2×2 and 3×2 to bound readback. Actual channel target
sizes are asserted separately from output sizes; this is not a claim of full-resolution pixel
coverage. Picture/Shared Texture retain the existing tiny fixture limits. Solid Color uses
the existing 64 MiB functional allowance to accommodate two required 1920×1080 targets.
No production qualification budget changed.

Checks completed in this order:

1. Exact native test and `native_source_fixture` compiled with `native-qualification`.
2. `cargo test --locked -p MapGyver --features native-qualification --lib media:: --quiet`:
   **143 passed**. Strict Clippy for `--test channel_rendering` passed with `-D warnings`.
3. Final exact native test compilation succeeded. The compiled test executable ran the named
   case with `--exact --ignored --nocapture --test-threads=1`: **1 passed, 0 failed**, six
   hardware/source cases, **86.86 seconds**. One native attempt; no correction rerun was needed.

Reproduce after the same prerequisites:

```powershell
cargo test --locked -p MapGyver --features native-qualification --test channel_rendering same_media_variants -- --ignored --nocapture --test-threads=1
```

This report closes M04/2 only. The broader [M04/4 Spout acceptance](mapgyver-m04-spout.md)
subsequently passed on 2026-11-03: full lifecycle, cross-adapter mismatch/recovery, RGBA byte-storage
pixels and declared-input overlap/capacity. The linked partial 2026-11-02 evidence remains history.
[M04/6's phase audit](mapgyver-m04-source-parity.md) subsequently records accepted generic
interaction, revised caller/Picture/resource parity, Reference and Media Preview gates.
M03 timing/memory/recovery and M05 physical output gates remain unchanged.

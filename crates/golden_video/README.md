# Golden Video

Golden Video owns app-neutral video execution contracts. MapGyver owns media/channel identity,
authored size policy, source selection, mapping and editor behavior. The desktop host owns native
window construction and WebView UI-thread access. Video plans depend on neither app nodes nor Tauri.

The first M03 delivery is the immutable render-plan compiler:

- `identity.rs` names logical textures, passes and output bindings. These are plan identities,
  separate from persistent media/channel IDs and native resource ownership/generations.
- `resource/` declares nonempty texel extents, storage, RGB transfer encoding and alpha convention.
  Source metadata is explicit; renderer-internal targets use linear-light premultiplied alpha.
- `color/` validates finite SDR premultiplied clear colors and unit-range opacity.
- `plan/` validates the complete submitted dependency graph and caller-supplied limits, then
  compiles only demanded sources/passes in deterministic dependency order. Multiple output
  bindings can share one texture without duplicating its producer or logical storage charge.

Run compilation on a preparation worker when desired state changes, outside engine ticks,
UI callbacks and per-frame rendering. Compiled storage prunes inactive metadata and compacts
retained buffers; the executable plan does not keep the submitted inactive vector capacity.

The execution DAG describes texture dependencies and operations. It is distinct from Golden
Graph's authored graph document/ports/history/canvas, and is neither persisted nor transported
as JSON. Operation input references are the dependency source of truth. Composite layers keep
their authored order; independent passes use stable pass-ID ordering.

The footprint counts tight logical source/target bytes once per demanded texture. It is a
preparation bound, not actual GPU admission or an assertion about native allocation usage.
The runtime must additionally charge padding, bridge/presentation pools, prepared/live/in-flight/
retiring generations and retained cache leases. Missing DXGI counters do not establish a zero
usage/budget policy.

Worker ownership, atomic generation installation, null/native execution, source supervision and
GPU qualification belong to the remaining [M03 acceptance gates](../../docs/progress/mapgyver-status.md#m03-acceptance-items).
Reuse `golden_io` for bounded worker/cancellation/publication fences; keep native resources and
final disposal on their worker. Shared time and persistent asset prerequisites remain explicit
Golden-owned work rather than app-local clock/cache protocols.

Run `cargo test --locked -p golden_video` for the platform-independent contract/compiler tests.

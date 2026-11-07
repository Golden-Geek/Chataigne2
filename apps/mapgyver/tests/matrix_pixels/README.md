# Matrix native raster/runtime acceptance

[Current revised M07/4 acceptance](../../../../docs/progress/mapgyver-m07-native.md) records
the complete both-adapter run, original limits and bulk typed-authoring correction.

Matrix owns document/playback and Layer/Group/Final production. The shared `../support` fixture
uses public ChannelRenderer, production preview compositing, explicit D3D11 adapters and finite
SDK/device readbacks. Its raster oracle propagates documented UNORM conversion ranges per pass.

```text
cargo test --locked -p MapGyver --features native-qualification --test matrix_rendering --test composition_rendering
cargo clippy --locked -p MapGyver --all-targets --features native-qualification,application-process-fixture -- -D warnings
cargo test --locked -p MapGyver --features native-qualification --test matrix_rendering -- --ignored --nocapture --test-threads=1
```

Run the complete retained and M12 native suites serially, after
inspecting terminal software and strict lint success. Use Cargo's optimized **test** profile,
including `cargo rustc --profile test` when
building an isolated executable. The dev profile and `test-fast` are not equivalent qualification
builds. Keep the original 180/90-second watchdogs and 60-second preview lease unchanged.

M11/8 makes Layer/Group outputs opt-in. The retained export/fade observations explicitly
expose only their public output routes through typed Matrix intents. Scene defaults stay
hidden; the separate nested exposure test verifies Final rendering while hidden and exact
Reference/export recovery. The software preflight checks exposure, unchanged identities and
project preservation before native qualification. No raster oracle or runtime limit changes.

The main scene reuses the actual three-Layer/three-Column defaults and authors 32×24 source
media and Layer canvases explicitly. The software preflight checks ownership, sparse defaults,
extent, ordering and exact project roundtrip before any hardware run. Separate native tests cover
Cell position/opacity/enabling and Layer position/canvas/resize through Undo without stretching
child media.

M12's `spatial_acceptance.rs` exercises live Cell translation/resize through two nested Groups,
all four blends and alpha, fixed versus natural placement after source resize, exact Undo/Redo,
Column copy/project reload and an inactive original Cell editor while its copy keeps playing.
The independent oracle reads Layer/Final/preview images on each hardware adapter. The separate
Cell editor suite covers inactive Fit/Fill/Stretch and natural placement; the ordinary sizing
suite retains its patterned crop/edge proof. Qualification uses `cargo rustc --profile test`
for isolated binaries; a successful dev-profile run is functional evidence only.

The complete native suite covers all blend modes, post-composite group opacity, ordering/Undo,
shared source targets, missing source/Undo, empty columns, reload/recreation and independently
read Layer/Group/Final/preview rasters. A 1,000-layer ×1,000-column document has six populated cells; 1,000 configured launches
use a 10 ms schedule while the original admission/resource limits remain enforced. Report actual
launch span separately from outcome draining/recovery; a schedule alone is not measured cadence.

Fade checks use fresh single-image SpoutProbe observations. Layer/Group/Final receivers are
created and sampled concurrently on separate owning workers; serial device startup must not
consume the other outputs' observation window. Receiving, staging and CPU storage share nine
readback credits (three per consumer), released before the next observation group. This test-only
pool does not change renderer admission/resource budgets. Probes are not streaming receivers.
Opaque RGB basis clips test native weight conservation, a three-way interrupted mixture, stable
outgoing color ratio, monotonic progress and final outgoing-demand retirement. These observations
do not identify an exact sampled frame time or qualify the global CPU/GPU/cadence budgets.

Fixture drop waits for actual renderer retirement and asserts zero live allocations, balanced
created/released counts and empty output/source inventories. Pixel comparison checks every
texel against propagated legal UNORM ranges with the unchanged 2/255 residual tolerance.
The active ledger owns current gate acceptance; old-schema run history remains evidence only.
Global process/handle/private-growth, sustained CPU/GPU/cadence and integrated qualification
retain their original M03/M05/M09 owners.

M13/6's `scoped_stops.rs` uses the shared public-contract `../matrix_controls` scene with
`application_configuration/scoped_stops.rs`. Six scopes preserve an unrelated independent
Cell and empty Column anchor. The software suite holds null-renderer final completion and
proves aggregate process retention through Stop All/project/renderer close. The selected
native test checks real output/preview pixels, shared process identity and zero balanced
native inventories on both GPUs:

```text
cargo test --locked -p MapGyver --features native-qualification,application-process-fixture --test application_runtime --test matrix_rendering
cargo test --locked -p MapGyver --features native-qualification,application-process-fixture --test matrix_rendering native_scoped_stops -- --ignored --nocapture --test-threads=1
```

Cargo's integration-test binary prerequisites can try to replace a running development app.
The [M13/6 receipt](../../../../docs/progress/evidence/mapgyver-m13-control-acceptance-2026-11-07.json)
records the isolated Cargo-package workaround used while preserving that app: existing test
and process-fixture sources, the public MapGyver library, original vendor patches, unchanged
dependency versions and the optimized test profile. No alternate rendering or completion
implementation is introduced. Its temporary manifest/lock and exact binary hashes are recorded.

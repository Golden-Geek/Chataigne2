# Matrix native raster/runtime acceptance

Matrix owns document/playback and Layer/Group/Final production. The shared `../support` fixture
uses public ChannelRenderer, production preview compositing, explicit D3D11 adapters and finite
SDK/device readbacks. Its raster oracle propagates documented UNORM conversion ranges per pass.

```text
cargo test --locked -p MapGyver --features native-qualification --test matrix_rendering --test composition_rendering
cargo clippy --locked -p MapGyver --all-targets --features native-qualification -- -D warnings
cargo test --locked -p MapGyver --features native-qualification --test matrix_rendering native_matrix_pixels -- --ignored --nocapture
```

The native test covers all blend modes, post-composite group opacity, ordering/Undo, missing
source/Undo, empty columns, reload/recreation and independently read Layer/Group/Final/preview
rasters. A 1,000-layer ×1,000-column document has six populated cells; 1,000 configured launches
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

Both native attempts in batch 76 failed: first an overly narrow oracle, then transition probe
credit exhaustion before fade sampling. Static rasters, sparse-grid stress and reload have partial
evidence. Fresh-probe sampling and explicit adapter logging compile, but are not yet native verified.
M07 gate 4 stays open until the corrected complete test passes; no third run belongs to batch 76.
M06's raster acceptance remains historical evidence from its recorded commit; shared fixture
software tests pass after this extraction, without a new M06 hardware claim.

Batch 79 confirms actual adapter identity and completes all checks on Intel. NVIDIA completes
static rasters, sparse-grid bursts and reload, but sequential receiver sampling does not observe
three interrupted-fade mixtures before its six-second deadline. Concurrent owning-worker sampling
is the precise fixture correction; it remains native unverified. The fade duration, sample count,
weight/ratio tolerances and original production budgets are unchanged.

Batch 80 observes all three required NVIDIA mixtures, but its unconditional pending deadline
rejects the already-completed set. Expiration now applies only while observations remain pending;
the final NVIDIA endpoint/demand checks are still unverified. The native chain stops for separate
M08 acceptance work before the exact corrected fixture returns. No further native retry in batch 80.

Batch 82 passes the complete fixture on Intel and NVIDIA: 112 full-image raster captures, three
interrupted mixtures per Layer/Group/Final on each adapter, final blue endpoints/demand retirement,
sparse-grid burst recovery, reload and six balanced renderer retirements. M07 native gate 4 is
accepted. Actual editor input remains gate 3; global process/handle/private-growth, sustained
CPU/GPU/cadence and integrated Q01–Q05 qualification keep their original M03/M05/M09 owners.

# M04/4 Spout source acceptance

**Accepted: 2026-11-03.** MapGyver owns sender-only authored Shared Texture variants, source
selection and channel preparation. Golden Video owns metadata discovery, native subscriptions,
adapter isolation, frame publication and completion-based retirement. No sender dimensions or
format are authored by the UI; actual native metadata determines source layout.

The existing app-path fixture now passes its complete lifecycle on Intel Graphics and the
NVIDIA RTX 5080 Laptop GPU. It creates two authored inputs and three native consumers, including
two consumers sharing the first input. All output pixels use the original **2/255** tolerance.

| Requirement | Verified behavior |
| --- | --- |
| Exact discovery/selection | SDK catalog tracks owned start/stop/rename; renamed senders never implicitly replace the selected name. Explicit authored selection recovers the intended input. |
| Absence/restart/resize | Missing input becomes transparent while the healthy peer keeps publishing; restart and native size changes recover without replacing the healthy input. |
| Limits and metadata | Oversized input is incompatible while the peer stays live; returning to an admitted size recovers. Fixed 3×2 output stays fixed while observed native input metadata becomes 4×3 and pixels update. |
| Adapter isolation | Both mismatch directions explicitly recover with a failed native HRESULT and zero foreign-source publications; selected GPU/device identity stays unchanged, healthy peer advances, and selecting the local sender recovers. |
| RGBA storage/alpha | Owned producer emits actual `Rgba8Unorm`, straight-alpha source metadata. Native app outputs retain red with alpha 128 (Intel) / 127 (RTX), against expected 128 within the unchanged tolerance. |
| Ownership/capacity | Shared consumers retain two subscriptions. Golden's native declared-binding test verifies replacement fencing, four-source overlap bound, storage ceiling rejection, continued healthy pixels, reusable retired capacity and clean shutdown. |

Checks ran after source fixture/harness compilation and strict Clippy succeeded. Golden's
qualification-feature software suite passed **188 tests**, with 24 explicit hardware ignores.
The complete app lifecycle passed in **88.18 seconds** on its first native run this batch;
Golden's declared-input case passed in **1.93 seconds**. Both used owned independent producer
processes; none remained after terminal success. App runtime code is unchanged from the
retained 340-test software success.

[Exact evidence and commands](evidence/mapgyver-spout-acceptance-2026-11-03.json) supersede the
[partial 2026-11-02 evidence](evidence/mapgyver-spout-metadata-native-2026-11-02.json). The source
fixture adds an explicit RGBA mode through the production runtime; no diagnostic framework,
app policy, protocol, admission limit or qualification budget changes.

After the same software/lint prerequisites, reproduce the native cases from the workspace root:

```powershell
cargo build --locked -p MapGyver --features native-qualification --bin native_source_fixture
cargo test --locked -p MapGyver --features native-qualification --test channel_rendering shared_texture::authored_shared_textures_isolate_failure_resize_rename_and_adapter_mismatch -- --exact --ignored --nocapture --test-threads=1
cargo test --locked -p golden_video --features qualification --lib native::backend::tests::inputs::hardware_inputs_receive_bound_resize_reconnect_and_retire_across_independent_processes -- --exact --ignored --nocapture --test-threads=1
```

[M04/5 source UI](mapgyver-m04-source-ui.md) and [M04/6 source parity](mapgyver-m04-source-parity.md)
subsequently passed, completing M04. Native physical/timing and genuine device-removal gates
remain separately open; this acceptance does not claim release readiness.

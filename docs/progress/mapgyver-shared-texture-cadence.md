# Shared Texture cadence repair — 2026-11-04

Golden Video's copy-only source worker now uses an explicit D3D11 fence for GPU completion.
Its existing two-use bound, source-device ownership, keyed-mutex bridge, reader retirement,
autorecovery and no-production-readback contract remain intact. This requires the selected
hardware device to support `ID3D11Device5` / `ID3D11DeviceContext4`; unsupported creation reports
the actual error through source recovery. Renderer/output completion still uses its existing
event queries. No presentation route or diagnostic collection is enabled as a workaround.

The production defect was hidden by the previous diagnostic test: enabling GPU timestamp
queries made source completion advance promptly. With diagnostics disabled, the same source
made only 6 copies in three seconds and six changing GPU center patches during the consuming
interval. The source's SDK and bridge reported no busy/error condition. Inside the actual app,
the source likewise advanced only about twice per second while window Presents remained 60/s.

After the fence repair, with diagnostics disabled, the source made **170 copies / 3.005 s**.
The consuming interval submitted 166 frames and observed **158 changing 8×8 GPU patches**;
the elapsed 3.406 s includes final drain. The bounded live regression now defaults to the
production diagnostic configuration and requires at least 90 pixel changes during its three
second moving-source interval. `GOLDEN_LIVE_SPOUT_DIAGNOSTICS=1` explicitly selects comparison
instrumentation. Production code never performs these test readbacks.

Actual foreground editor pixels were captured with the existing owned-PID/window guard:

| Scene | Before | After |
| --- | --- | --- |
| Screen Warping | 13 changes / 69 samples, about 2.2/s | 69 changes / 70 samples, 11.60/s |
| Media Preview | about 2.2–2.4/s | 69 changes / 70 samples, 11.72/s |

The desktop observer tops out near 12 samples/s. Every consecutive repaired capture changed;
this verifies removal of the reproduced freeze, not 60 FPS scanout or the full M03 timing gate.
A WebView repaint control and a separate DirectComposition candidate did not improve the
defect; the candidate was removed completely. Temporary app telemetry was also removed.

Checks: 184 Golden Video software tests pass (21 explicit hardware ignores); strict all-target
D3D11 Clippy passes; the existing native versioned preflight/busy/publication/reader test passes;
the production-mode live-pixel regression passes. Both actual native editor panels pass the
bounded moving-pixel witness and the owned host exits normally. Root/Core formatting passes.
No protocol, UI bundle, source policy or acceptance denominator changed.

Local evidence: `target/mapgyver-preview-cadence-baseline-2026-11-04`,
`target/mapgyver-preview-cadence-pixels-2026-11-04`,
`target/mapgyver-preview-cadence-composition-2026-11-04`,
`target/mapgyver-preview-cadence-source-state-2026-11-04`, and
`target/mapgyver-preview-cadence-fixed-2026-11-04`.
The tested isolated executable is `target/debug/deps/MapGyver-codex-shared-texture-fix.exe`,
SHA-256 `DE6F8F023B3D7E1E17978D2487B84B4DAC58619C96BE4B2FC0B6395030956D3E`.
The sender was Spout Demo Sender on NVIDIA RTX 5080 Laptop GPU, LUID
`dxgi:00000000:00016022`, observed source extent 1899×1100. Intel correctly rejected this
foreign-adapter sender with E_INVALIDARG; no adapter fallback is inferred.

The urgent reproduced regression is repaired. M03/9's original GPU critical-path/window timing
and private-growth qualification, and M03 device-removal acceptance, remain open.

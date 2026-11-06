# Shared Texture formats

Golden Video owns native storage, D3D11 views, Spout metadata admission, source budgets and
GPU sampling. MapGyver consumes the observed descriptor when preparing its media plan;
the UI supplies no format setting or conversion policy. Sender selection is independent;
see [sender creation and recovery](mapgyver-sender-selection.md).

## Current import path

Spout metadata discovery admits native RGBA8/BGRA8, RGBA16F/RGBA32F, RGBA16 UNORM/SNORM,
RGBA8 SNORM, RGB10A2 and BGRX8 without converting their receiving storage to eight bits.
`PixelFormat` is the Rust source of truth for logical
texel storage and checked byte accounting; native `texture::format` supplies the DXGI mapping.
`SpoutSource::observed_descriptor` assigns unsigned byte RGB its existing sRGB interpretation;
floating, signed-normalized and higher-precision normalized RGB are linear. Alpha is straight
where present; BGRX supplies opaque alpha. The generic sampling
shader premultiplies coverage in linear space. An explicit declared contract keeps its authored
transfer/alpha interpretation instead. No per-frame CPU image conversion is added.

The source worker reserves the actual format before allocating three bridge slots. Its existing
conservative charge includes those three images plus the sender alias, and remains held until
real source/renderer uses retire. Descriptor changes still withdraw the old publication and
retire imports before allocating replacement storage; failures retain the existing reconnect
and device recovery strategy. MapGyver's ordinary render/output precision policy is unchanged:
accepting RGBA32F input does not promise that every downstream pass/output is also32-bit.

The first-frame readiness probe shares the same native format/interpretation mapping and
reserves its full native image size. Its RGBA8 pixel-export API rejects other layouts before staging
allocation. The eight-bit window/Spout output contract is unchanged.
The source input enum is not duplicated in handwritten UI types. Existing generated contracts
remain checked by the app suite.

## SDK inventory and unfinished acceptance

The pinned SDK's [SpoutGL implementation](../../vendor/spout2-sys-0.1.1/vendor/Spout2/SPOUTSDK/SpoutGL/SpoutGL.cpp)
lists ten WGL/DX11 interoperability formats near `SetDX11format`; its `GLformat` switch also
mentions three-component float storage. The [DirectX sender](../../vendor/spout2-sys-0.1.1/vendor/Spout2/SPOUTSDK/SpoutDirectX/SpoutDX/SpoutDX.cpp)
passes the supplied texture descriptor to `CheckSender`, so that advertised list is not proof
that every other DXGI value is unavailable. Device-specific sharing/view support and any
additional admitted color formats still require inventory and explicit results. No claim of
complete format support is made by the float slice.

| Native value | SDK storage | Current import / remaining requirement |
| --- | --- | --- |
| 87 | BGRA8 UNORM | Existing byte import, sRGB / straight alpha |
| 28 | RGBA8 UNORM | Existing byte import, sRGB / straight alpha |
| 10 | RGBA16 FLOAT | Typed8-byte import; linear / straight alpha; both-GPU float receipt below |
| 2 | RGBA32 FLOAT | Typed16-byte import; linear / straight alpha; both-GPU float receipt below |
| 11 | RGBA16 UNORM | Typed8-byte import; both-GPU exact bits/shader/readiness/retirement pass |
| 13 | RGBA16 SNORM | Typed8-byte import; both-GPU exact bits/signed shader/readiness/retirement pass |
| 24 | RGB10A2 UNORM | Typed4-byte import; both-GPU exact packed bits/alpha/shader/readiness/retirement pass |
| 29 | RGBA8 UNORM SRGB | Pending exact native format/view handling without double gamma decode |
| 31 | RGBA8 SNORM | Typed4-byte import; both-GPU exact bits/signed shader/readiness/retirement pass |
| 88 | BGRX8 UNORM | Typed4-byte import, sRGB / opaque; raw SRV/shader decode; corrected both-GPU precision/readiness/retirement pass |
| 6 | RGB32 FLOAT | Additional GL mapping; sharing/required-view capability and implicit alpha remain unverified |
| 21 /22 | Legacy D3D9 ARGB/XRGB metadata | SDK compatibility mappings exist; exact alias/storage/alpha admission remains unverified |
| Other DXGI values | DirectX pass-through candidates | Device/backend capability inventory and interpretation remain unverified |

M13/2 remains Active until every required format and explicit hardware/API limitation is
accounted for, including both-GPU independent native pixels, fractional/float content,
live format/size changes and sender/device loss/recovery. A failed or unavailable case stays
unaccepted; no fallback or narrower successful subset closes the gate.

The source-format conformance test lives in `golden_video/src/native/inputs/tests/formats.rs`.
Its producer uses independent D3D11 Clear and the actual SDK, bypassing Golden's render
conversion. It checks all captured float bits, fractional/HDR/negative RGB and straight-alpha
sampling into a float target on every hardware adapter, followed by completion-safe source
budget reclamation. It also verifies automatic-adapter first-frame readiness and exact probe
storage admission/reclamation. The [four-case both-GPU result](../progress/evidence/mapgyver-m13-float-input-2026-11-06.json)
passes. It uses no desktop input and makes no physical latency/cadence claim.

The [normalized-format receipt](../progress/evidence/mapgyver-m13-normalized-input-2026-11-06.json)
retains the two failed cohorts and the corrected fourteen-case acceptance on both GPUs. The
new formats preserve independent source bits and match CPU-decoded color/alpha references.
BGRX's native sRGB decoder returned0.2163119 for128/255, versus the mathematical0.21586053,
exceeding this fixture's1/4096 linear tolerance. BGRX now uses its raw SRV and the existing shader
transfer decoder instead; no CPU conversion or precision-budget change is introduced. The
unchanged fixture passes the concrete view-selection correction on both GPUs: seven formats,
exact storage bits, float/normalized precision, opaque/straight alpha, actual first-frame
readiness and full source/probe budget reclamation. The two failed cohorts remain failed;
the corrected acceptance does not erase them or close M13/2. The remaining inventory,
actual-app integration and live format/size/sender/device recovery are still required.

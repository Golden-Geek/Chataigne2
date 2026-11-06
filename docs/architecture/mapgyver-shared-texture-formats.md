# Shared Texture formats

Golden Video owns native storage, D3D11 views, Spout metadata admission, source budgets and
GPU sampling. MapGyver consumes the observed descriptor when preparing its media plan;
the UI supplies no format setting or conversion policy. Sender selection is independent;
see [sender creation and recovery](mapgyver-sender-selection.md).

## Current import path

Spout metadata discovery admits native RGBA8/BGRA8, RGBA16F and RGBA32F without converting
their receiving storage to eight bits. `PixelFormat` is the Rust source of truth for logical
texel storage and checked byte accounting; native `texture::format` supplies the DXGI mapping.
`SpoutSource::observed_descriptor` assigns byte RGB its existing sRGB interpretation and
floating RGB a linear interpretation, with straight alpha in both cases. The generic sampling
shader premultiplies coverage in linear space. An explicit declared contract keeps its authored
transfer/alpha interpretation instead. No per-frame CPU image conversion is added.

The source worker reserves the actual format before allocating three bridge slots. Its existing
conservative charge includes those three images plus the sender alias, and remains held until
real source/renderer uses retire. Descriptor changes still withdraw the old publication and
retire imports before allocating replacement storage; failures retain the existing reconnect
and device recovery strategy. MapGyver's ordinary render/output precision policy is unchanged:
accepting RGBA32F input does not promise that every downstream pass/output is also32-bit.

The first-frame readiness probe shares the same native format/interpretation mapping and
reserves its full float image size. Its RGBA8 pixel-export API rejects floats before staging
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
| 10 | RGBA16 FLOAT | Typed8-byte float import, linear / straight alpha; current slice |
| 2 | RGBA32 FLOAT | Typed16-byte float import, linear / straight alpha; current slice |
| 11 | RGBA16 UNORM | Pending typed storage/import, precision and color/alpha proof |
| 13 | RGBA16 SNORM | Pending signed-normalized storage and sampling proof |
| 24 | RGB10A2 UNORM | Pending packed storage, precision and alpha proof |
| 29 | RGBA8 UNORM SRGB | Pending exact native format/view handling without double gamma decode |
| 31 | RGBA8 SNORM | Pending signed-normalized storage and sampling proof |
| 88 | BGRX8 UNORM | Pending exact native storage and opaque alpha proof |
| 6 | RGB32 FLOAT | Additional GL mapping; sharing/required-view capability and implicit alpha remain unverified |
| 21 /22 | Legacy D3D9 ARGB/XRGB metadata | SDK compatibility mappings exist; exact alias/storage/alpha admission remains unverified |
| Other DXGI values | DirectX pass-through candidates | Device/backend capability inventory and interpretation remain unverified |

M13/2 remains Active until every required format and explicit hardware/API limitation is
accounted for, including both-GPU independent native pixels, fractional/float content,
live format/size changes and sender/device loss/recovery. A failed or unavailable case stays
unaccepted; no fallback or narrower successful subset closes the gate.

The float conformance test lives in `golden_video/src/native/inputs/tests/formats.rs`.
Its producer uses independent D3D11 Clear and the actual SDK, bypassing Golden's render
conversion. It checks all captured float bits, fractional/HDR/negative RGB and straight-alpha
sampling into a float target on every hardware adapter, followed by completion-safe source
budget reclamation. It also verifies automatic-adapter first-frame readiness and exact probe
storage admission/reclamation. The [four-case both-GPU result](../progress/evidence/mapgyver-m13-float-input-2026-11-06.json)
passes. It uses no desktop input and makes no physical latency/cadence claim.

# Shared Texture formats

Golden Video owns native storage, D3D11 views, Spout metadata admission, source budgets and
GPU sampling. MapGyver consumes the observed descriptor when preparing its media plan;
the UI supplies no format setting or conversion policy. Sender selection is independent;
see [sender creation and recovery](mapgyver-sender-selection.md).

## Current import path

Spout metadata discovery admits native RGBA8/BGRA8, RGBA16F/RGBA32F, RGBA16 UNORM/SNORM,
RGBA8 SNORM, RGB10A2 and BGRX8 without converting their receiving storage to eight bits.
R/RG16F, R32F, single/two-channel UNORM/SNORM, R11G11B10 float and A8 coverage storage
also use direct native import. Their color is linear; missing color components are zero and
missing alpha is opaque. A8 supplies coverage with zero RGB, rather than replicated luminance.
RG32F and packed BGR use the float bridge where available; XR-bias uses compatible raw
R10 UNORM storage with explicit shader interpretation, as described below.
`PixelFormat` is the Rust source of truth for logical
texel storage and checked byte accounting; native `texture::format` supplies the DXGI mapping.
`SpoutSource::observed_descriptor` assigns unsigned byte RGB its existing sRGB interpretation;
floating, signed-normalized and higher-precision normalized RGB are linear. Alpha is straight
where present; BGRX supplies opaque alpha. The generic sampling
shader premultiplies coverage in linear space. An explicit declared contract keeps its authored
transfer/alpha interpretation instead. No per-frame CPU image conversion is added.

The source worker reserves the actual format before allocating three bridge slots. Its
charge includes those three images plus the sender alias, and remains held until
real source/renderer uses retire. Descriptor changes still withdraw the old publication and
retire imports before allocating replacement storage; failures retain the existing reconnect
and device recovery strategy. MapGyver's ordinary render/output precision policy is unchanged:
accepting RGBA32F input does not promise that every downstream pass/output is also32-bit.

The first-frame readiness probe shares the same native format/interpretation mapping and
reserves its full native image size. Its RGBA8 pixel-export API rejects other layouts before staging
allocation. The eight-bit window/Spout output contract is unchanged.
The source input enum is not duplicated in handwritten UI types. Existing generated contracts
remain checked by the app suite.

## Packed BGR4444 and XR-bias

BGR4444 (native115) uses two-byte storage, linear color and straight alpha, with the same
GPU-only float bridge as the other packed BGR layouts. Its full source charge is52 bytes per
texel. Independent native bits, fractional color/alpha and source/probe reclamation pass on
Intel. NVIDIA rejects the SDK's shared allocation flags with E_INVALIDARG; reception remains
explicitly unaccepted there. Mapping an enum never turns that allocation failure into Ready.

XR-bias (native89) is part of the
[R10G10B10A2 typeless family](https://learn.microsoft.com/en-us/windows-hardware/drivers/display/casting-ability-of-xr-formats).
Golden retains its distinct logical format and exact native alias/registration, while using
copy-compatible R10G10B10A2 UNORM receiving storage and views. No extra image/conversion pass
is required. The sampling shader reconstructs the ten-bit code and applies
[`(code - 384) / 510`](https://learn.microsoft.com/en-us/windows-hardware/drivers/display/xr-bias-to-float-conversion-rules)
before transfer decoding, premultiplication or filtering. Its two-bit alpha remains
[ordinary normalized coverage](https://learn.microsoft.com/en-us/windows-hardware/drivers/display/xr-format-alpha-content).
The untagged source default is linear/straight, and declared callers retain their interpretation.
The four-image direct charge remains16 bytes per texel; the probe charges one four-byte image.
Copy-compatible native24 and89 registrations remain semantically distinct and cannot stand in
for each other during exact alias authentication.

The independent producer first tried a UNORM render view on its native XR texture; Intel
rejected that view before submission. The corrected producer clears independent UNORM codes
and uses the documented [same-family GPU copy](https://learn.microsoft.com/en-us/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-copyresource)
to populate an actual XR resource before the SDK publishes it. Raw staging bits authenticate
that producer separately from Golden's receiver. The failed cohort is retained; production
decoding and the1e-6 target are unchanged. See the
[packed-format receipt](../progress/evidence/mapgyver-m13-packed-input-2026-11-06.json)
for native results and the remaining NVIDIA/API and full app/recovery requirements.

## GPU bridge for non-NT layouts

RG32 FLOAT, BGR565 and BGRA5551 cannot use the direct NT/keyed bridge on these adapters.
Golden keeps their original native descriptor and interpretation, receives into one charged
raw capture, then uses an integer-coordinate GPU Load/draw into RGBA32F sharing storage.
The renderer imports that bridge and draws back into the caller's original native texture.
Existing formats retain their direct receive/copy path. This internal transport adds no CPU
image conversion, authored format setting, transfer operation or filtering. Its shaders and
rasterizer are prepared once; it allocates no constant buffers or auxiliary image atlas.

`Surface` distinguishes the public source descriptor from its private bridge storage descriptor.
Only immutable metadata and the retained NT handle cross workers. Source completion covers
the SDK copy and bridge draw; renderer completion and reader leases cover import and the
native-target draw. Replacement and shutdown retain the original two-device drain ordering.
Declared metadata and discovery both retain the native layout; consumers never have to infer
an RGBA32F source from a two-channel sender.

Admission counts **two raw images** (sender alias and capture) plus **three RGBA32F bridge
images**: 64 bytes/texel for RG32F and 52 for either packed BGR layout. The public
`InputLimits::source_storage_bytes` applies this arithmetic to declared and discovered sources,
including checked overflow. Direct formats keep their existing four-image charge. The probe
needs no NT bridge and still charges only its native receiving image. These are logical
storage charges; no physical residency, peak-growth or timing qualification is implied.

Packed BGR sampling may expand five/six-bit channels through an eight-bit representation.
The private bridge preserves those component codes and the round trip restores native bits.
The final renderer sampling reconstructs the original normalized five/six-bit values before
transfer decoding, premultiplication or interpolation. The precision target is unchanged;
the first cohort's sampler error remains failed evidence rather than a tolerance revision.

The [conversion receipt](../progress/evidence/mapgyver-m13-conversion-input-2026-11-06.json)
passes 128 cases on Intel/NVIDIA: 24 new cases cover all three layouts with explicit,
unspecified and Declared metadata, plus the existing 104 regressions. Both BGRA5551 alpha
states preserve their raw bits and composite over white. Every new native round trip is
bit-exact and the largest shader error is 2.98023224e-8 against the original 1e-6 target.
Declared admission rejects a cap one byte below the full conversion charge before spawning
a worker. Real source/probe readiness and complete logical charge reclamation pass. The
three converted layouts bring supported native import to 28 formats, including 25 direct
layouts; the complete M13/2 gate, full app and recovery scope remain open.

## Native sRGB storage

Native RGBA8/BGRA8/BGRX8 sRGB storage (DXGI29/91/93) maps to the corresponding raw UNORM
logical format with explicit sRGB encoding. D3D11's
[CopyResource contract](https://learn.microsoft.com/en-us/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-copyresource)
permits copies within the same typeless family. This preserves the sender bytes rather than
applying a transfer conversion while receiving. The existing sampling stage decodes RGB once;
alpha stays coverage. Declared encoded-premultiplied sources also retain their bytes until the
shader can unpremultiply before decoding. No new logical enum or UI setting is needed.

The receiving endpoint retains one authenticated native handle, raw format, logical descriptor
and foreign alias. Preflight checks the raw format even when two endpoint layouts normalize to
the same descriptor, and authenticates the actual shared texture against that exact raw value.
Receive checks the caller's canonical descriptor and the SDK's exact raw layout after the call.
A change withdraws the old endpoint/counter lifetime; existing completion-safe retirement and
reconnect remain in force. The pinned SDK already uses the compatible D3D11 copy operation;
its receive API and implementation are unchanged.

## Legacy registry metadata

Spout's registered values21/22 are D3D9 ARGB/XRGB identifiers, not DXGI enum values. The
native `spout_layout` boundary maps both to authenticated BGRA8 storage. ARGB defaults to
straight alpha and XRGB to opaque alpha; declared callers retain their explicit interpretation.
Receiving and first-frame readiness share this policy. The endpoint retains both the registered
identifier and actual native format, validates the opened alias against the SDK mapping, and
compares the SDK's post-receive format with native storage. Mislabeled float storage cannot
satisfy this legacy BGRA contract or become Ready.

The sys crate exposes the SDK's existing sender-registry metadata update API for initialized,
caller-owned sender registrations. It changes metadata without changing texture storage;
its unsafe contract forbids receiver instances/foreign registrations. The owned qualification
producer advertises independent D3D11 BGRA storage through real legacy SDK metadata. These
results prove the metadata/import boundary; they make no D3D9 producer-device claim.

## Unspecified registration format

Registered format0 means unspecified storage. Golden opens the exact sender alias on its
selected adapter and authenticates its actual format, extent, mip/array and sampling layout
before choosing a descriptor. It does not treat native DXGI values21/22 as legacy identifiers
when resolving an unspecified alias. Nonzero registrations keep their exact native contract;
mislabeled known/legacy aliases remain rejected. Unchanged endpoints reuse the validated
alias, with no extra per-frame open. Mid-receive replacement waits for the caller's GPU fence
and returns through the same registered-metadata preflight instead of reinterpreting the
SDK's native format as registry metadata.

The first-frame probe resolves unknown storage before reserving or allocating a receiving
image. Its initial descriptor carries only the registered extent bound; it never allocates
that byte-format hint. Full native byte accounting then applies, including float images.
A cap sized for an8-bit image rejects a larger float image, releases the failed reservation
and cannot report Ready. Known-format probes retain admission before graphics initialization.

The [unspecified-format receipt](../progress/evidence/mapgyver-m13-unspecified-input-2026-11-06.json)
passes52 cases on both GPUs: the earlier28 known/legacy cases and24 unspecified registrations
covering all12 currently admitted native layouts. Bit identity, color/alpha, first-frame
readiness and source/probe retirement pass. Two retained source workers/probes also pass six
phases through RGBA32F2×2 → BGRA8 1×1 → RGBA16F2×1, with real sender loss/reappearance,
new pool generations, exact source pixels and full changing reservations/reclamation.
The owned producer is destroyed/recreated between phases; continuous-producer reformat/resize
remains separately required. Unsupported native layouts stay unaccepted; unspecified metadata
adds no fallback.

## Direct channel formats

The [channel-format receipt](../progress/evidence/mapgyver-m13-channel-formats-2026-11-06.json)
adds13 native layouts on both GPUs, with explicit and unspecified registration. These keep
one/two/four-byte storage and native normalized/float precision. The shared resource model
exposes physical alpha presence independently of transfer and premultiplication. Source
discovery uses opaque alpha for R/RG/R11G11B10 and straight alpha for A8; declared contracts
retain their interpretation. Formats without a typeless family do not request typeless/sRGB
views. No CPU image conversion or authored setting is introduced.

| Native formats | Storage | Default channels / interpretation |
| --- | --- | --- |
| 61 /63 | R8 UNORM /SNORM | (R,0,0,1), linear |
| 49 /51 | RG8 UNORM /SNORM | (R,G,0,1), linear |
| 56 /58 | R16 UNORM /SNORM | (R,0,0,1), linear |
| 35 /37 | RG16 UNORM /SNORM | (R,G,0,1), linear |
| 54 /34 | R16 /RG16 FLOAT | (R,0,0,1) /(R,G,0,1), linear |
| 41 | R32 FLOAT | (R,0,0,1), linear |
| 26 | R11G11B10 FLOAT | (R,G,B,1), linear |
| 65 | A8 UNORM | (0,0,0,A), linear coverage |

Two independently bounded native fixtures pass104 cases:52 earlier cases plus52 new
adapter/format/registration combinations. Every new case preserves source bits and matches
the float shader reference exactly, including fractional red, HDR above one, signed values
and implicit channels. A8 composites over white to verify real coverage with a nonblack
reference. Source/probe logical format charges, actual first-frame readiness and reclamation
pass; physical driver allocation growth/timing remain separate retained qualification.
The unchanged90-second guard applies separately to each fixture; no product timing or
memory limit changes. The existing live worker/probe and mismatch regressions also pass.

## SDK inventory and unfinished acceptance

The pinned SDK's [SpoutGL implementation](../../vendor/spout2-sys-0.1.1/vendor/Spout2/SPOUTSDK/SpoutGL/SpoutGL.cpp)
lists ten WGL/DX11 interoperability formats near `SetDX11format`; its `GLformat` switch also
mentions three-component float storage. The [DirectX sender](../../vendor/spout2-sys-0.1.1/vendor/Spout2/SPOUTSDK/SpoutDirectX/SpoutDX/SpoutDX.cpp)
passes the supplied texture descriptor to `CheckSender`, so that advertised list is not proof
that every other DXGI value is unavailable. Device-specific sharing/view support and any
additional admitted color formats still require inventory and explicit results. No claim of
complete format support is made by the verified subsets.

| Native value | SDK storage | Current import / remaining requirement |
| --- | --- | --- |
| 87 | BGRA8 UNORM | Existing byte import, sRGB / straight alpha |
| 28 | RGBA8 UNORM | Existing byte import, sRGB / straight alpha |
| 10 | RGBA16 FLOAT | Typed8-byte import; linear / straight alpha; both-GPU float receipt below |
| 2 | RGBA32 FLOAT | Typed16-byte import; linear / straight alpha; both-GPU float receipt below |
| 11 | RGBA16 UNORM | Typed8-byte import; both-GPU exact bits/shader/readiness/retirement pass |
| 13 | RGBA16 SNORM | Typed8-byte import; both-GPU exact bits/signed shader/readiness/retirement pass |
| 24 | RGB10A2 UNORM | Typed4-byte import; both-GPU exact packed bits/alpha/shader/readiness/retirement pass |
| 29 | RGBA8 UNORM SRGB | Byte-preserving canonical UNORM import; both-GPU bits/color/alpha/readiness/retirement and receiver replacement pass |
| 91 /93 | BGRA8 /BGRX8 UNORM SRGB | Same-family canonical UNORM import; both-GPU bits/color/alpha/readiness/retirement and receiver replacement pass |
| 31 | RGBA8 SNORM | Typed4-byte import; both-GPU exact bits/signed shader/readiness/retirement pass |
| 88 | BGRX8 UNORM | Typed4-byte import, sRGB / opaque; raw SRV/shader decode; corrected both-GPU precision/readiness/retirement pass |
| 16 | RG32 FLOAT | Native descriptor retained through RGBA32F bridge; both-GPU bits/shader/readiness/retirement pass |
| 85 /86 | BGR565 /BGRA5551 | Same GPU bridge, exact native round trips and five/six-bit normalized sampling; both GPUs pass |
| 115 | BGRA4444 | GPU bridge/native bits/fractional alpha pass on Intel; NVIDIA SDK shared allocation unavailable and unaccepted |
| 89 | RGB10 XR-bias /A2 | Exact native alias, compatible raw UNORM storage and explicit bias decoding; both GPUs pass |
| 6 | RGB32 FLOAT | Both GPUs reject the pinned SDK's shared allocation flags with E_INVALIDARG; API limitation stays unaccepted |
| 21 /22 | Legacy D3D9 ARGB/XRGB metadata | Both-GPU native bits/color/alpha/readiness/retirement and sender replacement pass over D3D11 BGRA storage; D3D9 producer-device scope unverified |
| Other DXGI values | DirectX pass-through candidates |70 Intel/68 NVIDIA shared native allocations open successfully; remaining import/conversion and typed-view/color proof below |

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

The [sRGB import receipt](../progress/evidence/mapgyver-m13-srgb-input-2026-11-06.json)
passes24 native cases on Intel/NVIDIA: all ten formats listed by the SDK plus DXGI91/93.
Native bits, color/alpha, first-frame readiness and source/probe retirement pass. Byte-source
RGBA/BGRA comparisons retain the existing2/255 native kernel target; BGRX1/4096 and
float/normalized1e-6 targets are unchanged. Six retained-receiver sequences cover18 phases
of sender destruction/reappearance through UNORM→SRGB→UNORM with one capture and unchanged
logical descriptor. Each observes the new exact bytes after real GPU completion. These
receiver-level results do not close the remaining worker/app/device recovery requirements.

The [legacy and backend receipt](../progress/evidence/mapgyver-m13-legacy-input-2026-11-06.json)
adds four legacy adapter/metadata cases to the regression cohort:28 format cases pass in total,
with real SourcePool/shader/readiness/storage retirement. Two retained legacy receivers pass
six ARGB→XRGB→ARGB replacement phases, while both reject a float alias advertised as ARGB;
registration alone never reports first-frame readiness. Earlier sRGB replacement cases pass.

## Remaining backend import work

The allocation inventory sweeps native DXGI ordinals1–191 using the pinned SDK's exact
8×8 shared texture descriptor, then opens an alias on a second same-adapter device and
checks NT/keyed bridge allocation. Reserved ordinals return native errors. It finds70 Intel
and68 NVIDIA allocations with exact foreign aliases, including12 formats already imported.
The subsequent channel slice raises direct native import to25 of those formats; allocation
counts remain unchanged and are not accepted-gate counts.
This is allocation/alias evidence, not typed-view, shader, pixel or complete import acceptance.
The receipt retains every available native value and rejected HRESULT; names come from the
pinned Windows bindings. SDK legacy0/21/22 normalization is separate from native DXGI values.

The remaining formats include RG32 integer/typeless, other unsigned/signed integer,
typeless and video-plane formats. RG32 FLOAT16 and packed BGR85/86 now use the GPU bridge
described above; they remain distinct from the 25 direct NT layouts. RG32 native15/17/18
still need typed interpretation. BGR4444 now uses the float bridge on Intel; NVIDIA's SDK
shared-allocation rejection remains unaccepted. XR-bias89 uses compatible UNORM storage and
explicit shader bias instead of requiring unsupported native shader/RTV operations. Native
import now covers30 layouts on Intel and29 on NVIDIA, including26 direct layouts; this does
not change the allocation counts or award a complete gate. Next is integer/typeless/video
handling with real source/bridge charges, followed by full app/recovery acceptance.

RGB32 FLOAT6 fails the SDK allocator's mandatory shader-resource/render-target shared flags
on both GPUs. It remains a named unaccepted API limit, with return triggered by an SDK/backend
that can publish such a shared source or a justified import/conversion path for an actual
producer. No successful subset, allocation result or rejection closes M13/2.
NVIDIA BGR4444 has the same concrete return trigger: an actual producer/backend that can
publish that native shared source, or new evidence supporting a different justified import
path. Repeating the same rejected SDK flags does not advance either requirement.

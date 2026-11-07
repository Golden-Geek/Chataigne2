# Shared Texture formats

Golden Video owns native storage, D3D11 views, Spout metadata admission, source budgets and
GPU sampling. MapGyver consumes the observed descriptor when preparing its media plan;
the UI supplies no format setting or conversion policy. Sender selection is independent;
see [sender creation and recovery](mapgyver-sender-selection.md).

MapGyver's media size resolver accepts the valid descriptor supplied by Golden, checking
the app's dimension bound without duplicating encoding/alpha defaults. The old sRGB/straight
restriction incorrectly classified linear float/integer and opaque video input as incompatible,
so planning replaced valid input with a transparent clear. The [actual-app receipt](../progress/evidence/mapgyver-m13-app-formats-2026-11-07.json)
retains that reproduced RGBA32F failure and the corrected both-GPU comparison. Four actual
native captures show RGBA32F in Media Preview and Screen Warping; fourteen representative
app/output cases pass with existing2/255 SDR output tolerance. Source precision evidence above
remains separate from the app's downstream pass/output policy. The
[continuous-source receipt](../progress/evidence/mapgyver-m13-live-formats-2026-11-07.json)
verifies24 actual-app stages across both GPUs: distinct unchanged-layout frames, float/integer/
sRGB/planar transitions, exact intrinsic dimensions and stop/restart of the selected sender.
The independent producer retains its SDK/device until explicit stop; new native storage waits
for prior GPU completion. The [device-reset slice](../progress/evidence/mapgyver-m13-format-device-reset-2026-11-07.json)
then observes actual Intel float-source device loss, same-adapter rebuilding and fresh float,
half-float, SINT and P010 output after external producer recreation. That default DXCap trigger
does not prove NVIDIA device loss; its strict assertion remains failed. NVIDIA device recovery,
D3D9, broader app classes and named interpretation/API limits still require acceptance.

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

## Packed video capture and decoding

Golden's `native/inputs/video` owns AYUV (100), Y410 (101) and Y416 (102) decoding.
Their native storage is separate from the canonical RGBA32F sampled descriptor. A read-only
native capture receives the SDK copy, and one GPU draw decodes it into the acquired bridge
slot. Y410/Y416 do not require an unsupported render-target view. Integer SRVs preserve the
original8/10/16-bit component codes and native alpha before conversion; no CPU image conversion
or eight-bit intermediate is used. The worker rebuild key includes the authenticated native
layout as well as the sampled descriptor, so RGB/video changes cannot reuse the wrong capture.

Spout does not supply matrix/range/transfer tags. The explicit automatic default is limited-range
BT.709 with inverse BT.709 transfer, preserving footroom/headroom in RGBA32F. Alpha is linear
coverage normalized by its native maximum. A declared sampled RGB contract may request sRGB
encoding and/or premultiplication after decoding; it does not describe different YUV colorimetry.
BT.601/BT.2020, full-range and HDR interpretations remain unaccepted pending an authoritative
producer contract and an explicit typed input-colorimetry boundary. No arbitrary inference is claimed.
Storage/view layouts follow Microsoft's [DXGI format reference](https://learn.microsoft.com/en-us/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format),
[YUV precision/alpha rules](https://learn.microsoft.com/en-us/windows/win32/medfound/10-bit-and-16-bit-yuv-video-formats),
[BT.709 matrix](https://learn.microsoft.com/en-us/windows/win32/medfound/recommended-8-bit-yuv-formats-for-video-rendering)
and [inverse transfer](https://learn.microsoft.com/en-us/windows/win32/api/dxva2api/ne-dxva2api-dxva2_videotransferfunction).

The [packed-video receipt](../progress/evidence/mapgyver-m13-packed-video-input-2026-11-06.json)
passes72 both-GPU cases with independent literal source codes and f64-derived references,
covering fractional alpha, bright headroom, below-black and three sampled contracts. One
failed NVIDIA power-precision baseline is retained. A ninth-root Newton refinement before
forming the BT.70920/9 power meets the unchanged1e-6 absolute target. The365 existing source
regression cases also pass; these results do not establish full app or live-change acceptance.

`InputLimits::source_storage_bytes_for_native` charges two raw images (sender alias and capture),
three decoded RGBA32F bridge images and one16-byte immutable decoder buffer before allocation.
Existing declared source reservations remain held; any native-layout excess is admitted separately.
At2×2, actual AYUV/Y410 storage costs240bytes and Y416 costs272bytes. A declared RGBA32F
reservation conservatively holds256bytes, so Y416 needs16 additional bytes. Existing source and
renderer completion authorities retain every image until actual uses retire. Probe readiness
charges only its raw16/32-byte capture and rejects byte export before allocating staging storage.

### Paired 4:2:2 storage

The same boundary imports YUY2 (107), Y210 (108) and Y216 (109). Their UINT view exposes
one `Y0,U,Y1,V` element per pair of logical pixels. The shader addresses `x/2`, chooses the
even/odd luma and uses the pair's shared chroma. This explicit pair-replication default avoids
the legacy hardware reconstruction view. Other chroma-siting/reconstruction interpretations
remain part of the unadvertised-colorimetry limitation above. Width must be even; native
metadata admission and capture construction reject odd widths before allocation.

YUY2 consumes two raw bytes per logical pixel; Y210/Y216 consume four. Y210 decodes its upper
ten bits and ignores the six padding bits in each word; Y216 retains all sixteen. These formats
have no alpha: automatic sampled descriptors are opaque RGBA32F and declared sampled RGB
contracts also receive coverage1. Raw V data never becomes alpha. The generic source contract
admits this opaque canonical float descriptor, just as legacy XRGB uses opaque canonical BGRA.
Native registration/alias authentication, sampled extent bounds and transfer validation still apply.

Source admission includes both packed raw images, three RGBA32F bridge images and the existing
16-byte decoder buffer. At4×2 this is432bytes for YUY2 and464bytes for Y210/Y216; declared
RGBA32F reservations conservatively hold512bytes. Probe captures charge only the raw16/32bytes.
The independent producer accepts literal rows with an explicit pitch, so native tests can vary
both lumas, neighboring pairs and rows without going through Golden's decoder.

The [paired-video receipt](../progress/evidence/mapgyver-m13-paired-video-input-2026-11-07.json)
passes30 both-GPU cases: known/unspecified metadata and three declared sampled contracts,
including physical opaque coverage under Straight/Premultiplied/Opaque interpretations.
Every4×2 image varies both lumas, neighboring chroma and rows; below-black/headroom,
Y210 padding and Y216 low bits match independent f64 references within unchanged1e-6.
Actual raw-copy readiness, denied extra-byte admission and complete charge retirement pass.
The72 existing4:4:4 and365 RGB/typeless regression cases pass unchanged. Full app/live/device
and alternative chroma reconstruction acceptance remain open.

### Planar 4:2:0 storage

NV12 (103), P010 (104) and P016 (105) use one native capture with two UINT views. NV12 maps
luma through R8 and interleaved UV through RG8; P010/P016 use R16 and RG16. The decoder
loads luma at `(x,y)` and UV at `(x/2,y/2)`, replicating each chroma sample over its2×2 block.
This default is explicit; unadvertised alternative chroma siting/reconstruction remains open.
P010 discards six padding bits per word before ten-bit normalization; P016 retains all sixteen.
Automatic sampled descriptors are opaque RGBA32F, with the same BT.709 and declared sampled
RGB rules as packed video. Native metadata and capture construction require both dimensions even.

Both views reference the same native image. The decoder binds/unbinds the optional chroma view
alongside its luma view, and the existing source completion authority retains that image. Admission
includes both planes in each of the two raw images, three decoded bridge images and the16-byte
decoder buffer. At4×4, NV12 raw storage is24bytes and the full source charge is832bytes;
P010/P016 raw storage is48bytes and the source charge is880bytes. Declared RGBA32F reservations
conservatively retain1024bytes. Driver row padding/residency is separate from these logical charges.
Probe readiness reserves the raw image only; it never stages or converts planar pixels on the CPU.

The independent finite producer uploads full luma rows followed by half as many interleaved UV
rows at the same explicit pitch. Its native staging reader includes both planes. This permits
nonuniform4×4 references with four different chroma blocks and sixteen distinct luma values.
P016's known NVIDIA SDK shared-allocation limitation remains unaccepted; its fixture checks
only that concrete allocator prerequisite, without repeating the format inventory.

The [planar receipt](../progress/evidence/mapgyver-m13-planar-video-input-2026-11-07.json)
passes25 available-path cases with all sixteen pixels compared against independent f64
references, below-black/headroom, P010 padding, P016 low bits and all sampled alpha modes.
Raw readiness, extra-byte denial and complete charge reclamation pass. The102 packed-video
and365 RGB/typeless regression assertions pass. A tool limit omitted24 RGB record rows;
the receipt states that limit and reuses unchanged case definitions and prior evidence, with
current terminal fixture success. Native value-set comparison covers the original70/68
allocation catalog completely; actual-app/live/device/D3D9 acceptance remains open.

## Typeless defaults and declared views

Typeless registrations identify storage families, not the producer's typed view. Golden uses
the following deterministic visual defaults for the twelve available native layouts:

| Native storage | Automatic interpretation |
| --- | --- |
| RGBA32 /RGBA16 typeless (1 /9) | RGBA32F /RGBA16F, linear /straight |
| RG32 /RG16 typeless (15 /33) | RG32F /RG16F, linear /opaque |
| R32 /R16 typeless (39 /53) | R32F /R16F, linear /opaque |
| RGBA8 typeless (27) | RGBA8 UNORM, existing sRGB /straight policy |
| RGB10/A2 typeless (23) | RGB10/A2 UNORM, linear /straight |
| RG8 /R8 typeless (48 /60) | RG8 /R8 UNORM, linear /opaque |
| BGRA8 /BGRX8 typeless (90 /92) | Existing BGRA8 /BGRX8 sRGB and physical-alpha policy |

`texture::typeless_default` owns those choices. Existing declared Core source contracts may
choose another supported member of the same native family, including UINT/SINT and XR-bias
interpretations. `spout_layout` validates that family while retaining the declared transfer,
alpha and extent bounds. The default and declared routes share the existing native-sized
storage/bridge admission, probe readiness and completion-safe retirement. No UI format
setting, CPU image conversion or extra image for typeless storage is introduced.

Exact registration/alias authentication is unchanged: known native metadata must still equal
the opened resource's actual format. A typed resource mislabeled as typeless, or the reverse,
is rejected even when their bits are copy-compatible. Unspecified0 resolves the native family
from an opened alias before choosing any receiving image. Compatible declared views apply
only after that authentication; they cannot authorize another family or exceed extent bounds.

Native producers use explicit compatible render views over actual typeless resources. Staging
uses a compatible typed member and compares raw bytes, independently of Golden's default
mapping. Declared integer cases upload literal32-bit values and preserve every bit through
typed integer bridges, including NaN-like bit patterns and values beyond float integer precision.

**Retained API limitation:** typeless metadata alone cannot identify an arbitrary producer
view or distinguish the same bits intended as FLOAT, UINT or another family member. Defaults
and compatible declared contracts are implemented and verified within that scope; arbitrary
automatic view inference remains named unaccepted. Return when a producer supplies a fully
typed format/contract or the backend exposes authoritative view metadata, rather than adding
a guessing heuristic or a manual UI format control. See the
[typeless receipt](../progress/evidence/mapgyver-m13-typeless-input-2026-11-06.json).
The first cohort passes all eight fixtures:72 default cases and24 declared integer override
cases on both GPUs, plus the unchanged269-case regression matrices. All96 new records retain
exact storage bits, ready-state and charge reclamation. Nonbyte references match exactly;
byte sRGB comparisons retain their original targets. Native defaults/view paths now cover61
Intel/60 NVIDIA layouts, with the arbitrary-view API limit and full gate still open.

## Integer color interpretation and transport

Golden admits every available typed integer color layout in the recorded backend inventory:
unsigned/signed R, RG and RGBA at8/16/32 bits per channel, plus unsigned RGB10/A2. These
remain distinct native formats and distinct logical `PixelFormat` values. Native metadata
and foreign aliases still require an exact match; integer bits are never treated as float bits.

For automatic visual sampling, unsigned channels use their full range as normalized color;
signed channels use the corresponding signed normalized range, clamping the minimum code
to−1. Signed alpha is clamped to coverage `[0,1]`. Packed RGB10/A2 uses1023 for color and3
for alpha. The default transfer is linear; declared callers retain their transfer/alpha policy.
Physical storage without alpha supplies coverage one even if a declared caller chooses
Straight or Premultiplied interpretation. A missing source still remains transparent.
Transfer decoding, unpremultiplication and interpolation occur after integer normalization.

| Native integer family | Transport and sampling |
| --- | --- |
| R/RG/RGBA8 and16 UINT/SINT | Direct typed NT/keyed images; typed integer Load, then normalized color |
| R32 and RGBA32 UINT/SINT | Same direct typed path; preserve all source bits, including codes beyond f32 integer precision |
| RG32 UINT/SINT | Raw capture and three matching RGBA32 UINT/SINT bridge images; typed integer pixel shaders in both directions |
| RGB10/A2 UINT | Direct native packed images; typed Load and independent RGB/alpha normalization |

`texture::sample_storage` and `integer_slot` select fixed shader tags/views. The generic
render pipeline binds float, unsigned and signed resources to their separate typed slots,
and unbinds each used view before returning. New import entry points return `uint4`/`int4`,
so the RG32 bridge preserves complete32-bit values rather than passing them through float
storage. Conversion shaders/state are prepared once and allocate no per-frame CPU image,
constant buffer or auxiliary image atlas. Existing source/renderer completion and reader
leases retain all storage through its real GPU uses.

Direct integer sources retain the four-native-image logical admission. RG32 integer sources
charge two raw eight-byte images and three sixteen-byte bridge images:64 bytes per texel.
No source count, storage ceiling or timing limit changes. Integer normalization converts to
the renderer's visual float precision only when sampling; downstream render/output precision
is unchanged and is separate from exact integer transport.

Qualification uses independent literal byte producers rather than float Clear values. It
authenticates native staging bits, round trips every bit, and compares a separate f64-decoded
reference with the production shader.32-bit cases include odd values above2^24, float-NaN
bit patterns interpreted as integers, extrema, fractional alpha and negative signed alpha.
Declared one/two-channel cases deliberately choose Straight alpha to verify implicit opaque
coverage. Wider matrices are separate finite fixtures with the original90-second guard.
The [integer receipt](../progress/evidence/mapgyver-m13-integer-input-2026-11-06.json) owns
results and scope; this does not close full backend/app/recovery acceptance.
Its first native cohort passes269 pixel cases:126 integer cases across42 independent
adapter/pattern combinations, each with all three metadata contracts, plus143 regressions.
All captured integer bits survive and the largest new shader error is5.96e-8 against the
unchanged1e-6 target.24 declared one/two-channel cases retain opaque physical coverage.
Native import now covers49 formats on Intel and48 on NVIDIA, including43 direct layouts.

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
| 100–102 | AYUV /Y410 /Y416 | Both-GPU literal native codes, BT.709 decode, alpha, sampled contracts, readiness and retirement pass through RGBA32F |
| 107–109 | YUY2 /Y210 /Y216 | Both-GPU nonuniform pair/row pixels,10/16-bit precision, opaque sampled contracts, readiness and retirement pass through RGBA32F |
| 103–105 | NV12 /P010 /P016 | Separate luma/UV UINT views and RGBA32F decoding pass on all available paths; NVIDIA P016 SDK allocation remains unavailable and unaccepted |
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

All19 available RGB integer layouts and twelve typeless default/view paths now pass on both
GPUs. Packed and planar video now pass through native read-only capture and GPU decoding.
Set comparison against the original inventory finds no missing or additional native value:
all70 Intel/68 NVIDIA available allocations have a tested default import path. P016 remains
unavailable on NVIDIA; it is implemented and verified on Intel, with no fallback or waiver.
RG32 integer sources use typed integer bridges; float/packed BGR bridges and XR-bias raw
compatible storage remain as described above. NVIDIA BGR4444 shared-allocation rejection
stays unaccepted. Recorded paths cover70 Intel/68 NVIDIA native layouts, with54 direct layouts;
the original70/68 allocation counts are unchanged and are never accepted-gate counts.
Next is complete actual-app/continuous-producer/device recovery and actual D3D9 producer
acceptance. Native default-path coverage does not resolve unadvertised colorimetry/chroma,
arbitrary typeless view inference or the named rejected allocations, and does not close M13/2.

RGB32 native5–8 (typeless/FLOAT/UINT/SINT) fail the SDK allocator's mandatory shader-resource/render-target shared flags
on both GPUs. It remains a named unaccepted API limit, with return triggered by an SDK/backend
that can publish such a shared source or a justified import/conversion path for an actual
producer. No successful subset, allocation result or rejection closes M13/2.
NVIDIA BGR4444/P016 have the same concrete return trigger: an actual producer/backend that can
publish that native shared source, or new evidence supporting a different justified import
path. Repeating the same rejected SDK flags does not advance either requirement.

# Video ownership and transport

MapGyver owns media transport policy in `apps/mapgyver/src/media/video`. Golden Time supplies
the monotonic authority and checked rational mapping. Golden Video owns file demux, decoder and
frame lifetime in its optional `decode` module. Neither the engine nor the UI decodes files.

`MediaTransport` is one owner per Video media. It starts paused at zero, with rate one and
looping off. Channel workers and References consume one immutable `TransportSample`; reading
does not advance time or accumulate rounded frame periods. Rate changes preserve the visible
position. Stop returns to zero. Completed non-looping playback restarts at the appropriate
endpoint when played again, including reverse playback.

Background metadata probing supplies the longest authored channel duration, including variants
without render consumers. Each channel samples at the lesser of
that shared position and its own duration, holding its final frame while longer channels run.
Temporary file loss must retain last-known metadata; clearing a channel's authored File is a
different operation from an unavailable endpoint. No whole-file timestamp index is required.

Explicit controls issue a new Golden Time epoch. Loop cycles are separately fenced, including
reverse boundaries, so an old seek result cannot publish into a later cycle at the same wrapped
position. Frame stepping pauses at an exact timestamp supplied by the selected channel's
decoder. The worker prepares the current, previous and next presentation timestamps; neither
the UI nor the operation adapter divides by a nominal frame rate.

Golden's `Decoder` retains original stream timestamps and explicit software or hardware storage.
Its ordinary open methods preserve HAP blocks and otherwise select software;
`open_native` requests D3D11 decode on an exact adapter while preserving HAP blocks for
GPU upload on that same selected adapter.
It performs no implicit colour conversion or GPU readback. Seeking uses the selected stream's
native time base and replaces codec reorder/drain state while retaining the file demux owner.
This prevents an old HEVC drain frame observed after buffer flushing in the pinned fixture.
Already-returned frames retain independent reference-counted storage. Seek setup cost remains
part of the later cold/warm seek and resource checks.

The D3D11 decoder uses FFmpeg's public hardware-device and frame-pool APIs, not a hand-written
copy of its platform ABI. A fresh DXGI lookup resolves the requested LUID, and every returned
surface verifies its actual adapter, extent, format and array slice. Format selection admits
only D3D11; unsupported codecs/profiles fail so the caller can explicitly reopen in software.
Before allocation, the fixed pool is limited to32 surfaces and the admitted padded pixel count.
The caller supplies `GpuFrameBudget`; a reservation for the pool's padded extent, native
format and surface count is acquired before allocation. FFmpeg's public pool destructor owns
that reservation, so old frames retain it across seeks and decoder shutdown. Replacement pools
must compete with those retained owners instead of receiving returned capacity early.

`DecodedFrame` exposes CPU planes only for software storage. `D3d11Frame` stays on its worker
and retains the actual FFmpeg frame, not just a COM texture reference: the frame lease prevents
the decoder from reusing that pool slice. Old frames therefore remain independently owned
across codec replacement and decoder shutdown. Software conversion rejects hardware storage;
only qualification tests map/read back pixels for independent luma checks. Current-device failures are
reported for withdrawal/reopen. Hardware decoder tests alone do not establish hardware playback
in MapGyver.

Golden's `GpuFramePublisher` now converts decoded NV12/P010 storage on that same device and
publishes completed RGBA images through `GpuFrameStream`. It copies only on the GPU into a
sampleable plane texture, then applies supported BT.709 matrix, full/limited range and
BT.709/sRGB/linear transfer metadata. Unsupported interpretation is rejected. The three fixed
NT/keyed-mutex slots carry encoded sRGB/straight-alpha RGB into the ordinary render plan through
`ExternalSource::GpuFrames`; CPU image streams remain separate and unchanged.

Submit retains an actual FFmpeg frame reference through GPU completion. Poll never waits and
returns only completed images; the media owner must validate its source/clock epoch before
publishing. Busy conversion or held slots retain the previous image. Drop drains real completion
or confirmed removal on the file worker and withdraws its stream. Source faults and released
slots wake that worker, with availability notification following strong-lease release.

The renderer acquires a nonblocking import lease before queuing the copy and retains it through
the matching render completion. Shared image and imported-view lifetimes both retain admission,
so closing a producer/NT handle cannot return its budget while another worker still owns the
allocation. Unused imports are pruned, and plan generations reuse their admitted target texture.
`GpuFrameBudget` bounds decoder pools and conversion/bridge storage across producers and retained
observers. Renderer targets have separate admission from the same measured adapter allowance.

Native File workers now request the renderer's exact adapter. The host carves out one quarter
of measured admission, capped at768 MiB, for their shared GPU budget and subtracts it from the
renderer allowance. That source cap survives renderer recreation; retained old allocations keep
their reservations. CPU fallback retains its separate existing768 MiB known-storage cap.
Returned-frame and driver-private allocation overhead is not claimed as measured VRAM usage.

The timestamp sampler is shared by software and hardware playback. A hardware worker permits
one conversion in flight, retains its original transport epoch/cycle, and checks that fence
again before publishing a completed image or its frame-step neighbours. A seek cannot install
late pixels. Held conversion slots preserve the previous image and retry even while paused.
No conversion completion is waited on by the engine, Inspector or render worker.

Unsupported hardware/profile/color, exhausted source admission and native source errors explicitly
withdraw that stream and reopen in software, preserving the same media clock and source binding.
Hardware is attempted again when demand is recreated or the native host recreates its selected
device. Existing software-file failure retry remains bounded and independent per channel.
Native session shutdown waits for file workers, decoder pools, published images and render imports
to retire before replacement. App hardware correctness and sustained performance still require
their owning acceptance evidence; selecting this path alone does not close M10.

HAP packets use pinned libavformat demux and the [HAP reference dependency](../../crates/golden_video/vendor/hap/README.md).
`HapFrame` holds immutable BC1 RGB, BC3 straight RGBA or scaled-YCoCg BC3 blocks with the visible
extent. Storage accounts padded4x4 blocks, including odd dimensions. HAP opens no libavcodec
pixel decoder; packet PTS and stream time base drive the same original-timestamp seek contract.
Returned block frames survive decoder shutdown. The software image converter reports typed
compressed storage instead of expanding HAP to RGBA.

The public reference header generates the FFI ABI. A private-prefixed C bridge uses pinned
Rust Snappy; local reference-source safety changes bound section/table/chunk/input/output access
before native pointer arithmetic. Exact block length and at most1024 synchronous chunks are
admitted on the owning file worker.

`GpuFramePublisher` now accepts HAP through the same three-slot ring, pending completion,
immutable image, import and retirement mechanism as decoded D3D11 frames. Its source texture
stores BC1/BC3 blocks directly; the GPU reconstructs pixels. Visible RGBA targets exclude padded
edge texels. BC1 RGB forces opaque alpha, BC3 RGBA preserves straight alpha, and HAP Q applies
the [reference YCoCg conversion](https://hap.video/developers) before publishing encoded sRGB
RGBA. Source storage, three bridge slots and the constant buffer are admitted before allocation.
No HAP CPU pixel expansion, pixel upload or readback is used by production playback.

The existing File native loop branches on typed `DecodeStorage` and shares the same reader,
clock/step fences, publication and cancellation lifecycle. Its CPU reservation covers three
maximum BC frames plus one16 MiB compressed packet allowance; ordinary native decoding reserves
the packet allowance. Those reservations share the existing768 MiB source cap with software
playback. A failed HAP native path retries natively after withdrawal, preserving the media clock;
it cannot fall through to software RGBA. Native GPU/worker retirement retains its normal actual
completion requirements. [M10/2 acceptance](../progress/evidence/mapgyver-m10-hap-acceptance-2026-11-09.json)
covers known chromatic/alpha/YCoCg-scale blocks, independently decoded external4K files,
three concurrent4K producers with six shared consumers, and actual HAP copy/Composition/Matrix/
Reference/reload pixels on both GPUs. The external test material stays in ignored `target`;
`MAPGYVER_HAP_EXTERNAL` selects its extracted `FFmpeg` directory for the owning native tests.
These short functional checks leave sustained performance and complete recovery to M10/4.

The default [source SDK build](../../tools/mapgyver/ffmpeg/source-build.json) pins the same
FFmpeg source with a complete reconstruction/source kit. It keeps built-in decoders, SIMD,
software scaling and D3D11/DXVA2 without optional external codec/filter/device/network libraries.
This makes the source/notices boundary independently reviewable and reduces its five DLLs to
22,422,528 bytes. Clean builds in two directories reproduce all five DLL bytes. Golden and
default-app software/strict, both-GPU native workers and actual app profile/routing/reload checks
precede its explicit runtime pin promotion. SDK, source kit and runtime staging live under
`target/ffmpeg-source` and `target/mapgyver-ffmpeg-runtime` respectively.

The [pinned dependency helper](../../tools/mapgyver/ffmpeg/README.md) verifies the exact Windows
FFmpeg/compiler/assembler sources and runtime bytes, with process-local build/runtime paths.
It builds missing SDK inputs and assembles corresponding sources and FFmpeg/GCC/MinGW notices.
Packet/image/work
admission is explicit; failures require reopening the owner. These limits do not claim a hard
ceiling on every temporary allocation inside third-party demuxers.

Video now participates in the real media catalog. Each channel owns a typed File Parameter;
Playback, Seek to (milliseconds), Rate and Loop are media-owned Parameters rendered by the
generic Inspector. Commands, named script methods and `MediaEdit` all edit those same nodes.
Generic edits enforce Stop's zero position and change a stopped media to Paused when seeking.
Sparse projects and history contain Parameters, never a serialized transport DTO or clock.

`VideoControl` is a shared runtime owner created per media incarnation. It projects the
Parameters onto `MediaTransport` and retains pending seeks until duration is known. A seek
clamped by partial duration discovery remains eligible for its requested position when a longer
variant becomes known. Repeating the same explicit seek reapplies the runtime action even when
its authored Parameter already has that value. Playback intent and actual readiness are
separate in `VideoTransportSnapshot`; `ready` means usable duration metadata and `playing`
reports the transport clock. Neither proves that a frame was decoded or presented. `Playing`
authored before metadata leaves the clock paused. Rates use a checked rational projection at
millionth precision.

The selected-channel Step operation reads prepared timestamps and commits Playback=Paused and
Seek to together through the existing atomic Parameter/history path. An unavailable File or
unprepared neighbour rejects without changing authored state. There is no pending authored
step to complete later, and no engine-side file or decoder wait. Source incarnation, exact
authored File binding and transport epoch/cycle fence every timestamp observation. The bounded
`InspectVideoFrame` query exposes readiness and actual neighbouring timestamps; compact
Inspector actions, typed commands, named scripts and headless operations use the same adapter.
At either endpoint, stepping holds the existing position and pauses; an already-paused endpoint
is a history no-op.

`InspectVideoSource`, its typed command and `inspectVideoSource` script method read one channel's
runtime health without opening a File. `Prepared` means known timing with no active decoder;
`Ready` means a decoded image stream exists, not that an exact seek or display presentation has
completed. Missing, unsupported, decode, capacity and filesystem failures carry typed kinds and
bounded256-character messages. A failed live decoder enters `Recovering`; initial failures are
`Unavailable`. Retry state is explicit. Builds without the Video file feature report unsupported
playback without promising a retry. Ordinary authored settings remain real Inspector Parameters.

The compact channel Inspector status uses a background read, at most one in flight and one
one-second retry timer. Active edit sessions defer its reads. Selection, File, connection and
project changes fence late replies and dispose timers. Native preview messages follow canonical
References and explicit Cell-editor dependencies, while disabled sources retain their normal
presentation. A failed Video never becomes a renderer/device failure or masks healthy sources.
The shared Footer continues to report renderer health separately.

Video operation adapters use Golden's public `settle_edit_notifications` inside the existing
atomic application operation. This bounded fixed point refreshes Parameter handles and runs
their callbacks before returning snapshots. Draining queued edits
alone leaves handles stale until notification dispatch. The shared API owns that ordering;
the app does not duplicate the engine's stabilization loop.

`video::Files` now owns demanded local-file workers. Captured channels carry the same media
control through References and retained Matrix producer graphs. Eight lifetime slots include
retiring workers; an explicit 768 MiB software storage admission follows every retained image
through its last observer. This accounts for admitted worker/frame storage, rather than
claiming a ceiling on all temporary allocations internal to FFmpeg. Cancellation interrupts
demux/open and wakes waits without joining on the engine or UI. Missing files retry once per
second. Relinking and project-root changes withdraw old sources before replacement.

At the eight-worker limit, an admitted replacement waits for actual old-worker retirement;
temporary slot pressure does not reject the new capture or fault healthy sources. Golden IO's
`RetirementPool::availability()` publishes a slot-release revision outside its state lock.
File demand captures that revision before cancellation/admission and retains a one-shot wake
subscription while pending. A release before subscription wakes immediately. The desired capture
coalesces newer edits, so neither a pending-job queue nor a polling timer is needed. Nine demanded
files still reject before displacing the eight admitted sources; worker and byte caps are unchanged.

The reader decodes at most eight frames per turn before checking controls/cancellation. It
samples original timestamps, holds the final frame at EOF, rewinds for backward seeks and
uses coalesced latest-image publication. After a paused keyframe seek, its preceding frame may
not yet be known. The worker searches earlier timestamps and decodes forward in bounded turns
to determine that exact neighbour while retaining the requested published image. Once ready,
paused workers wait for an explicit control wake.
Known duration survives consumer withdrawal; a returning Reference continues the media clock.
Per-channel owner tokens reject frames from superseded workers.

[Seek/retry/relink qualification](../progress/evidence/mapgyver-m10-seek-recovery-2026-11-09.json)
holds real decoded images across superseding controls, replaced decoders and revoked File bindings.
Both-GPU bundled-app fixtures verify latest-seek pixels, a delayed stale-image check and typed
missing/corrupt-file retry followed by valid-file recovery and authored relink. These short
functional checks leave combined device recovery and sustained resource/performance acceptance open.

The [native recovery fixture](../../apps/mapgyver/tests/native/README.md) uses full Golden device
identities and independent Spout probes around eight common/HAP File workers on each local GPU.
Its preflight preserves looping playback/authored history, then explicitly Pauses for static seek
comparisons and checks complete CPU/GPU retirement. Genuine recovery requires an observed new
device generation plus fresh native/BC output; an unavailable reset privilege earns no acceptance.

Channel lifecycle registrations bind each real File to its media control. File edits and actual
channel removal revoke the old generation and duration; consumer withdrawal does not. A counted
ordered duration set updates the longest duration in logarithmic time, avoiding whole-channel
rescans in lifecycle callbacks. A retired media control retains its bindings for already-admitted
Matrix clip producers until their final runtime owner disappears. Golden's snapshot-free
`ProcessCtx::is_destroying` identifies the owning media in the known destroy batch. The channel
releases membership during destroy, independently of nodes retained for Undo, and distinguishes
whole-media retirement from removing one channel.
Reparenting a channel registers it with its new media owner.

Golden's read-only `register_ancestor_runtime` callbacks bind shared runtime handles synchronously
before live activation/history restoration returns. This avoids inbox events that bootstrap
cleanup deliberately discards. The manager receives weak control registrations, including nested
owned media, without a deep event subscription or broader membership invalidation. A project-fenced registry lets one bounded
metadata service discover configured Files without scanning the document or starting renderers.
Desktop and headless host composition each retain this service independently of device creation.
Golden's demux-only timing probe retains no frame decoder. File generation and project fences
cancel obsolete probes and reject late results. Healthy metadata checks file fingerprints at a
coarse one-second interval; missing endpoints retry while retaining known timing. Relinking clears
old metadata, and root relocation reprobes the resolved path. All probing, filesystem checks and
waits run on the IO owner. The registry admits4096 media owners, each media admits4096 authored
channels, and metadata service work admits4096 configured Files across the project. Decode/image
worker and storage budgets remain separate and unchanged.

Golden's `ImageStream` supplies fixed-descriptor software frames to an installed render plan.
Its opaque publication identity is shared by clones and separates replacement streams whose
local sequence numbers coincide. The device-local weak texture cache keys software inputs by
that identity as well as their logical texture/layout. Uploaded sequence and withdrawal state
belong to the actual texture allocation, so compatible live plan generations retain held pixels
without uploading them again. New allocations after retirement/device recreation must upload
their current frame normally; the cache owns no source or GPU lifetime.
Native storage is updated only when its sequence changes. Frame delivery does not rebuild
plans or create textures per frame. The render worker uses a nonblocking observation and keeps
the previous upload during publication contention. Software conversion, upload and hardware
decoding are distinct: the current converter explicitly handles supported SDR BT.709/sRGB/
linear transfer, rejects other matrices/transfers, and never reads a hardware frame back.
Displaced pixel buffers are freed outside the media control lock.
Existing opt-in bounded native diagnostics record software `SourceUse` with stream identity and
the visible sequence (zero after withdrawal), plus an `ImageUpload` only at a real CPU upload.
That event reports tightly packed input row pitch, rows, bytes and API wall time. It does not
describe the original decoder planes, GPU execution, HAP block uploads or end-to-end source age;
software publication timing remains unknown. GPU-frame copy/import accounting is separate.
Software packing copies complete stride-trimmed RGBA rows, then applies the existing transfer
lookup in place. Alpha is untouched. This avoids four growing-vector pushes per pixel without
changing decoded pixels, thread admission or published-frame ownership. Q06's fixed ProRes
scenario and inward upload-end/output-API-begin age bounds live in `video_fixtures.py`,
`native-video-sustained.mjs` and `video_sample.py`; an age lower bound can prove failure, never
passing full source age. Each invocation is one independent warmed sample, not a three-run series.
`DecodeLimits::software_threads` separately admits one, two or four software codec workers.
Golden requests slice threading only when the codec advertises support, verifies the opened
context stays within its count and rejects any frame-thread flag. Unsupported codecs use one;
HAP keeps its compressed parser and hardware decode stays serial. Slice workers share the current
decoded frame, so this adds no frame-threading queue or delayed-frame policy. MapGyver selects
four for software File owners and one for native attempts; its eight-owner limit, CPU/frame pools
and GPU admission remain unchanged. Third-party codec scratch/stack costs still require measured
process accounting. Requested compute parallelism is operational admission, not authored state.
Both software and GPU File owners enable Golden IO's cancellable high-resolution deadlines.
Their shared pacing policy anchors each deadline before decoding/conversion: an overrun checks
controls and cancellation without adding another sleep. Pending work retains a1ms interval,
playing retains8ms and idle/paused playback waits for control notifications (with a one-hour
fallback). Timer creation errors surface as typed IO failures and retry through the existing
one-second recovery path. This changes neither authored clocks nor source/frame queue bounds;
it does not change process-wide Windows timer resolution or add stream callback state.
Golden's GPU-frame owner now supports source-worker software images. It uploads exact RGBA8 bytes only after claiming
a free keyed slot, retains CPU admission until real GPU completion, then publishes through the
existing GPU-frame stream. All render consumers share that completed
image; the render turn performs GPU imports/copies rather than CPU `UpdateSubresource` calls.
GPU-frame publishers use the same explicit timeline-fence completion as Golden's source workers;
copy-only progress must not depend on rendering/timestamp-query batches being submitted.
All three source slots are charged to existing GPU admission before native allocation.
MapGyver's attempted software File integration is retained in stash
`4717c50928733b156a19e7be03ccd5fdef17dfbe`: the fully validated owner works, but the combined
File loop reaches only HD39.70/4K24.38 updates/s versus the existing55.23/28.11 on NVIDIA.
Software File playback therefore retains ImageStream pending a pipelined integration that
meets the original throughput/ownership requirements. Do not automatically restore that stash.
Hardware/HAP File publishers use the timeline completion correction now; all original resource,
recovery and performance qualification remains required.
`SourceUseTrace::software_upload` retains the actual producer UUID/counter, byte/row layout and
optional source API QPC interval with the pixels copied into each renderer texture. Observers
deduplicate that identity across held frames/consumers. Timing is opt-in; absent timing or
uploads never sampled by a renderer remain unknown, including missing final producer counters.
The source-worker upload ends before GPU-frame publication, so its API end cannot serve as
a publication-age lower bound. The auditor keeps that age unknown rather than backdating it.
This does not replace full decoder/pool, per-format upload or source-age qualification.
For an explicit actual-app qualification run, set `MAPGYVER_NATIVE_DIAGNOSTICS` to an owned
capture directory before launch. The host creates a fresh PID/UUID file per renderer lifetime,
enables a16384-row ring and delegates writing to Golden's separate `DiagnosticCapture` I/O owner.
Capture stops at256MiB or ten minutes; failures/cutoffs remain visible and do not stop playback.
The host retires rendering/file workers before the final drain. With the variable absent or
empty, no capture file, collector thread or native timing queries are created. This operational
capture option is not authored project state. `native-video-playback.mjs` validates terminal
health and zero native allocations when the option is present; small functional captures do not
qualify sustained playback, decoder-pool accounting or source age.
Software conversion failures use Golden's typed `ConversionError`; app status never classifies
support by parsing human-readable decoder/scaler messages. Failure revokes prepared frame-step
timestamps. Healthy recovery clears the current source error without authored history. Metadata
errors retain known duration during endpoint loss and clear after successful re-probing, including
when the restored File has the same fingerprint as before its disappearance.

MapGyver enables `video-files` by default. Windows builds use the pinned dependency helper:

```powershell
powershell -NoProfile -File tools/mapgyver/ffmpeg/with-ffmpeg.ps1 run --locked -p MapGyver
```

The host binds these file sources to the existing Media Preview, Composition, Matrix and
Screen render graph. `tests/browser/native-video-playback.mjs` verifies the bundled app's real
File/Playback/Seek and frame-step controls, Undo/Redo, native preview installation, Reference
transport and save/New/load recovery. The existing read-only Screen observer compares physical
pixels with independent BT.709 or BC/sRGB/alpha/YCoCg references in linear space. It runs with an isolated
profile and `MAPGYVER_TEST_BINARY` pointing to the current app. The dependency helper verifies
and stages the five DLLs; the Windows Tauri resource map places them beside ordinary app builds
and carries notices under `licenses/video`. A staged app runs without an FFmpeg PATH entry.
`MAPGYVER_VIDEO_LOCAL_DLLS=1` verifies actual loaded module paths/sizes/hashes against the pin.
Tiny clips establish functional checkpoints, not sustained playback or
physical panel/cadence qualification.

M10/1 common-codec and M10/2 HAP functional gates are accepted in their linked audits.
Default source SDK/DLL/notice wiring and the complete local reconstruction/source kit are
implemented and functionally verified. Exact/coalesced seeks, relink/device recovery and sustained1080p/4K/resource/performance
acceptance remain M10/4 before M14. DXV/WebP remains M10/3
after M14. Final Windows artifact/source publication stays M09. Functional acceptance does
not close those retained requirements.

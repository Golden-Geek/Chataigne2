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
Its ordinary open methods select software; `open_d3d11` explicitly requests an exact adapter.
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
These per-pool bounds do not replace the caller's budget across retained old pools/generations.

`DecodedFrame` exposes CPU planes only for software storage. `D3d11Frame` stays on its worker
and retains the actual FFmpeg frame, not just a COM texture reference: the frame lease prevents
the decoder from reusing that pool slice. Old frames therefore remain independently owned
across codec replacement and decoder shutdown. Software conversion rejects hardware storage;
only qualification tests copy/map it for independent luma checks. Current-device failures are
reported for withdrawal/reopen. The app still selects software until native GPU conversion,
synchronized publication/import and generation/budget recovery are integrated. Hardware
decoder tests alone do not establish hardware playback in MapGyver.

The [pinned dependency helper](../../tools/mapgyver/ffmpeg/README.md) verifies the exact Windows
shared LGPL archive and establishes only process-local build/runtime paths. Packet/image/work
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

The reader decodes at most eight frames per turn before checking controls/cancellation. It
samples original timestamps, holds the final frame at EOF, rewinds for backward seeks and
uses coalesced latest-image publication. After a paused keyframe seek, its preceding frame may
not yet be known. The worker searches earlier timestamps and decodes forward in bounded turns
to determine that exact neighbour while retaining the requested published image. Once ready,
paused workers wait for an explicit control wake.
Known duration survives consumer withdrawal; a returning Reference continues the media clock.
Per-channel owner tokens reject frames from superseded workers.

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
Native storage is updated only when its sequence changes. Frame delivery does not rebuild
plans or create textures per frame. The render worker uses a nonblocking observation and keeps
the previous upload during publication contention. Software conversion, upload and hardware
decoding are distinct: the current converter explicitly handles supported SDR BT.709/sRGB/
linear transfer, rejects other matrices/transfers, and never reads a hardware frame back.
Displaced pixel buffers are freed outside the media control lock.
Software conversion failures use Golden's typed `ConversionError`; app status never classifies
support by parsing human-readable decoder/scaler messages. Failure revokes prepared frame-step
timestamps. Healthy recovery clears the current source error without authored history. Metadata
errors retain known duration during endpoint loss and clear after successful re-probing, including
when the restored File has the same fingerprint as before its disappearance.

During integration, enable `video-files` through the pinned dependency helper:

```powershell
powershell -NoProfile -File tools/mapgyver/ffmpeg/with-ffmpeg.ps1 run --locked -p MapGyver --features video-files
```

The host binds these file sources to the existing Media Preview, Composition, Matrix and
Screen render graph. `tests/browser/native-video-playback.mjs` verifies the bundled app's real
File/Playback/Seek and frame-step controls, Undo/Redo, native preview installation, Reference
transport and save/New/load recovery. The existing read-only Screen observer compares physical
pixels with the fixture's independent BT.709 reference in linear space. It runs with an isolated
profile, `MAPGYVER_TEST_BINARY` pointing to the current `video-files` app, and the pinned FFmpeg
DLL directory on `PATH`. Tiny clips establish functional checkpoints, not sustained playback or
physical panel/cadence qualification.

Default feature/redistribution wiring, broader actual-app visual qualification, complete profile/color/alpha coverage,
GPU-resident D3D11 hardware frames, HAP and sustained recovery/resource/performance acceptance
remain required by M10. The implemented software path does not close a codec acceptance gate.

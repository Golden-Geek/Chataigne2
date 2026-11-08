# Video ownership and transport

MapGyver owns media transport policy in `apps/mapgyver/src/media/video`. Golden Time supplies
the monotonic authority and checked rational mapping. Golden Video owns file demux, decoder and
frame lifetime in its optional `decode` module. Neither the engine nor the UI decodes files.

`MediaTransport` is one owner per Video media. It starts paused at zero, with rate one and
looping off. Channel workers and References consume one immutable `TransportSample`; reading
does not advance time or accumulate rounded frame periods. Rate changes preserve the visible
position. Stop returns to zero. Completed non-looping playback restarts at the appropriate
endpoint when played again, including reverse playback.

The file worker supplies the longest channel duration. Each channel samples at the lesser of
that shared position and its own duration, holding its final frame while longer channels run.
Temporary file loss must retain last-known metadata; clearing a channel's authored File is a
different operation from an unavailable endpoint. No whole-file timestamp index is required.

Explicit controls issue a new Golden Time epoch. Loop cycles are separately fenced, including
reverse boundaries, so an old seek result cannot publish into a later cycle at the same wrapped
position. Frame stepping pauses at an exact timestamp supplied by the selected channel's
decoder. The future operation adapter resolves that neighbour; it must not divide by a nominal
frame rate in the UI or inspector.

Golden's `SoftwareDecoder` retains original stream timestamps, planes and colour metadata.
It performs no implicit colour conversion or GPU readback. Seeking uses the selected stream's
native time base and replaces codec reorder/drain state while retaining the file demux owner.
This prevents an old HEVC drain frame observed after buffer flushing in the pinned fixture.
Already-returned frames retain independent reference-counted storage. Seek setup cost remains
part of the later cold/warm seek and resource checks.

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
Parameters onto `MediaTransport`, retains pending seeks until duration is known and coalesces
selected-channel step requests with revision fences. Playback intent and actual readiness are
separate in `VideoTransportSnapshot`; `ready` means usable duration metadata and `playing`
reports the transport clock. Neither proves that a frame was decoded or presented. `Playing`
authored before metadata leaves the clock paused. Rates use a checked rational projection at
millionth precision. The worker
must resolve exact neighbouring timestamps and publish resolved seek Parameters through the
engine before claiming complete frame stepping.

Video operation adapters use Golden's public `settle_edit_notifications` inside the existing
atomic application operation. This bounded fixed point refreshes Parameter handles and runs
their callbacks before returning snapshots or issuing step requests. Draining queued edits
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
uses coalesced latest-image publication. Paused workers wait for an explicit control wake.
Known duration survives consumer withdrawal; a returning Reference continues the media clock.
Per-channel owner tokens reject metadata/frames from superseded workers. Complete metadata
discovery for unopened variants and authored clear/removal synchronization remain required;
the current known-duration registry has a 4096-entry admission bound.

Golden's `ImageStream` supplies fixed-descriptor software frames to an installed render plan.
Native storage is updated only when its sequence changes. Frame delivery does not rebuild
plans or create textures per frame. The render worker uses a nonblocking observation and keeps
the previous upload during publication contention. Software conversion, upload and hardware
decoding are distinct: the current converter explicitly handles supported SDR BT.709/sRGB/
linear transfer, rejects other matrices/transfers, and never reads a hardware frame back.
Displaced pixel buffers are freed outside the media control lock.

During integration, enable `video-files` through the pinned dependency helper:

```powershell
powershell -NoProfile -File tools/mapgyver/ffmpeg/with-ffmpeg.ps1 run --locked -p MapGyver --features video-files
```

The host binds these file sources to the existing Media Preview, Composition, Matrix and
Screen render graph. Default feature/redistribution wiring, actual-app visual qualification,
exact frame-step commits, all-variant metadata, complete profile/color/alpha coverage,
GPU-resident D3D11 hardware frames, HAP and sustained recovery/resource/performance acceptance
remain required by M10. The implemented software path does not close a codec acceptance gate.

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

Current implementation stops at these transport and decoder boundaries. Video's typed media
and per-channel File Parameters, commands/functions/callbacks/snippets, worker queues and actual
preview/output integration remain to be wired. GPU-resident D3D11 frames, HAP compressed textures,
device/file recovery and final redistribution/performance remain required by M10. Software
fixtures do not establish actual-app playback or close a codec acceptance gate.

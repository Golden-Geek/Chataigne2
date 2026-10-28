# Golden Time

Golden Time owns shared time coordinates, affine mapping and activation observations. Apps own
play/pause/seek/loop/rate policy. Audio/video own device scheduling, preparation, queues and
once-only effect application. No audio, video, engine or IO dependency is required here.

`TimeAuthority<C: Clock>` is the single owner of a show mapping. It derives every observation
from the original monotonic/show anchor and rational rate, rather than advancing a timeline for
each consumer callback. `SystemClock` uses a monotonic `Instant`; clones preserve its origin.
Clock reads must be lightweight observations, without blocking native work or advancing effects.

The authority publishes immutable snapshots/mappings. A transport discontinuity reanchors the
show mapping and issues a checked next epoch. Epoch equality includes a fresh authority identity,
so replacement authorities cannot revive old work by both starting at generation zero. Regression,
source failure, overflow or epoch exhaustion commits no partial state.

`MonotonicTime`, `ShowPosition` and `EngineSamplePosition` keep units distinct. Monotonic scalars
belong to a declared clock origin; scheduled work also carries its epoch. A `SampleMapping`
separately declares continuous monotonic/sample anchors: show seeks do not reset engine samples,
and replacement with a different clock origin needs an explicit new sample mapping.

`RationalRate` supports exact rates such as 60000/1001 and 48000/1. Conversions use checked integer
arithmetic with explicit floor/ceil quantization. Schedule frame deadlines from an absolute tick
count using `duration_for`; repeatedly adding a truncated single-frame period accumulates drift.
Nanosecond resolution still quantizes subnanosecond periods. Show mappings retain their original
anchor, use signed floor for positions, and explicitly reject noninvertible pause or pre-anchor
targets instead of silently selecting transport policy.

`ActivationStamp::check` rejects stale epochs and reports pending duration or due lateness.
It is an observation, not a once-only effect claim or a live-resource lease. A scheduler uses a
fresh authority snapshot and owns cancellation, ready/late policy and idempotent activation.

This shared boundary supports MapGyver M03 scheduling. It does not complete Chataigne P14's
audio/DMX integration, output authority handoff, latency or native synchronization acceptance.

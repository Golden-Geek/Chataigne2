# Kinect2 worker ownership

Kinect2 is an app-owned input module under
`apps/chataigne/src/module/modules/controllers/kinect2`. Its Windows SDK bindings stay in
`kinect2_runtime/native/windows.rs`; Golden IO supplies bounded workers, lifetime admission,
source leases, notifications and retry deadlines. No Kinect types or policy live in Golden code.

## Native lifetime and preparation

The native owner loads the SDK, opens the sensor/reader, receives frames and releases SDK objects
on the same worker. A zero-timeout SDK event wait does not establish a bound on the following
native calls. The actor never performs those calls, waits for readiness or joins a thread.
Four process owner slots and one per-module slot include blocked setup, polling and cleanup.
Actor retirement immediately fences queued values; worker exit invalidates its native source
and notifies before provider destruction. Blocked cleanup retains its slots.

The SDK adapter admits six bodies and 25 fixed joints. It owns every returned body reference
before any fallible inspection, so an early error releases the unvisited references too. Native
decoding validates joint identity, uniqueness, tracking state, finite positions and nonnegative
100 ns timestamps. SDK position units remain unchanged. Native endpoints cannot open in tests.

`kinect2_runtime/preparation.rs` owns primary-body selection, reference transforms, hand distance,
XY direction, speed history, tracking messages and the existing 1e-6 publication noise threshold.
Speed uses SDK timestamps; repeated/backward timestamps, gaps above 0.5 seconds, body/reference
changes and sensor loss reset history. Reference changes reproject the last bounded raw frame
without reopening the sensor or counting that authored change as incoming device traffic.
Each snapshot carries the current tracking message, so cancelling an earlier revision cannot
consume its acquired/lost status. Unchanged messages use ordinary parameter no-op behavior.

The shared publication pool admits 128 original lifetimes / 2 MiB, charging 16 KiB per prepared
frame. The mailbox admits 32 rows / 512 KiB. A provider turn and actor drain admit at most sixteen
rows each. The native provider samples one SDK frame per turn and suppresses unchanged availability
reports without a frame. Accepted tracking transitions retain order. Source or mailbox pressure
keeps the same pending frame and pauses polling in cancellable 10 ms waits; configuration revisions
can cancel stale pending work. The worker's remaining idle wait is 8 ms and never runs on the actor.
Vendor buffering, physical SDK behavior and platform performance require separate qualification.

## Passive actor and recovery

`service.rs` projects prepared values through worker notifications. Healthy idle service has no
periodic actor callbacks, snapshots or wakes. Startup, native failure and full lifetime capacity
use absolute retry deadlines with backoff from 250 ms to five seconds. Disable/destroy retire
the source; enabling requests a bounded replacement.

`projection.rs` captures every fixed value target, its named ancestors, module identity and the
authored reference space before owned warning/value edits. Each prepared row adds its actor,
native and configuration fences. Queued moves, renames, settings changes, disable and retirement
reject stale projections. Missing snapshots leave input queued. The actor only projects prepared
state, connection status and incoming traffic.

Kinect has no custom output commands, script methods or tracking-specific script callbacks. Its
existing base functions, module callbacks and app-owned template remain registered. Local evidence
and remaining C11 work belong to the [execution ledger](../progress/implementation-status.md).
Hardware/platform and product-scale qualification retain their later plan owners.

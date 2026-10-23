# Ultraleap worker ownership

Ultraleap is an app-owned input module under
`apps/chataigne/src/module/modules/controllers/ultraleap`. Golden IO supplies bounded workers,
source leases, lifetime admission, notifications and recovery deadlines. LeapC bindings remain
in the existing `leap-sys` dependency; no tracking-service protocol is recreated in Golden code.

`ultraleap_runtime/native` owns dynamic SDK loading, connection setup, native polling, device
enumeration and final close/destruction. `native/tracking.rs` copies SDK data into fixed typed
hand snapshots before the next SDK poll invalidates native pointers. Hand traversal admits at
most two hands, rejects null arrays and invalid/duplicate identities, and converts millimeters
to meters. Device enumeration admits at most 64 entries before allocation and validates the
returned count. Library-path overrides admit 4096 bytes. Native endpoints cannot open in test builds.

## Lifetimes and publication

Four process owner slots and one per-module slot include setup, running and cleanup. The control
actor never waits for SDK readiness or joins a thread. Retirement invalidates actor publications
immediately; worker exit invalidates its native source and notifies before SDK cleanup. A blocked
open or cleanup retains both slots, preventing replacement thread growth.

The shared publication pool admits 128 original lifetimes / 2 MiB. Every prepared envelope charges
16 KiB, covering fixed frame data, bounded strings and source guards. The mailbox admits 32 rows /
512 KiB. Source leases survive draining, shared copies, queued application, rejection and disposal.
When source or mailbox capacity is full, the IO owner retains the same occurrence and pauses
provider polling in cancellable 10 ms waits. Cancellation needs no free queue slot.

Providers return at most 16 records with bounded backing capacity per turn. A native turn polls
once with a 25 ms timeout and at most fifteen further times without waiting. Consecutive frames
in that turn coalesce to the latest state only when service/device and hand-presence state match.
Status messages and hand-presence transitions delimit coalescing; their order is preserved. The
pending mailbox remains a bounded history of prepared publications, rather than an unbounded
SDK event backlog. Native service-unavailable responses retain the SDK connection and retry after
250 ms; the remaining worker idle wait is 8 ms. These waits never run on the control actor.
These application bounds do not qualify the vendor SDK's internal queues or memory use.

`ultraleap_runtime/preparation.rs` validates finite numbers, two-hand/device-count limits and
1024-byte message backing storage. It owns tracking-acquired/lost messages, service-loss reset
and inter-hand distance calculation. Invalid values fail before actor parameter edits. This module
has no custom output commands or script methods; its existing base functions, connection/parameter
callbacks and app-owned template remain unchanged.

## Passive actor service

`service.rs` uses worker notifications and drains at most 16 publications per turn. Startup and
worker failures use absolute deadlines with backoff from 250 ms to five seconds. Full retirement
capacity also backs off. Healthy idle service has no recurring actor updates, snapshots or wakes.
Disabling or destroying the module stops its source; enabling requests a new bounded owner.

`projection.rs` captures the module identity and every value target and named ancestor before a
bounded turn. Each publication adds its own actor/native fence at the projection position. Queued
renames, reparenting, disable and native retirement therefore reject stale values before mutation.
The actor only projects prepared state, emits ordinary incoming traffic and applies existing module
connection behavior. Missing snapshots leave input queued for a later real projection turn.

The [execution ledger](../progress/implementation-status.md) records local qualification and
remaining closure work. Tests use controlled providers and in-memory SDK records. Physical LeapC
service/device, cross-platform and full-product scale qualification retain P17/P18 and P16 ownership.

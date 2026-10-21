# OSC worker boundary

OSC is app-owned under `apps/chataigne/src/module/modules/protocol/osc`.
`osc_runtime` owns native socket setup, binding/DNS, polling, codec execution, transmission,
receive preparation and final native cleanup. `osc_outbound` captures immutable command, script
or latest-value sources and prepares the complete destination plan on that worker;
`osc_outbound_work` services its ordered destination effects.
`osc_input` prepares bounded received-value batches and `messageReceived` callbacks; `osc_input_queue`
retains their worker continuation and `osc_input_delivery` applies graph/edit and callback receipts.
The module owns authored configuration, value nodes, script/command surfaces and transport replacement. Reusable queue, lifetime and reply
mechanisms come from `golden_io` and the app's common native-reply helper.

`osc_discovery` independently owns native interface enumeration and immutable choice catalogs;
`osc_catalog_delivery` applies those catalogs through a guarded public Core edit.
`osc_service` coordinates passive wakes and absolute retry deadlines. OSC's P03 ownership boundary
is locally qualified with fake providers and UDP loopback. C03 remains open for MIDI; physical,
cross-platform and product-scale qualification remains separate.

## Admission and service

| Resource                   | Limit                                                                                           |
| -------------------------- | ----------------------------------------------------------------------------------------------- |
| Live/cancelling workers    | 16, reserved before native setup                                                                |
| Bind host storage          | 1024 bytes including spare string capacity                                                      |
| Outgoing commands          | 256 items / 4 MiB queued retained weight                                                        |
| Captured outgoing work     | 1 MiB reservation each: at most four queued plus one active request                             |
| One outgoing payload       | 128 KiB retained weight, 1024 stored values/expanded arguments                                  |
| Destination plan           | 256 outputs / 256 KiB including reserved vector storage; host 1024 bytes                        |
| Destination preparation    | 64 traversal edges per turn; 16384 edges per request                                            |
| Feedback admission         | 256 coalesced target IDs, at most 16 capture attempts per actor turn                            |
| OSC address                | 4096 bytes                                                                                      |
| UDP wire packet            | 65507 bytes                                                                                     |
| Received packet shape      | 256 packets including bundles, depth 32, 1024 type tags, array depth 32                         |
| Prepared input envelope    | At most 16 messages/callbacks, 2 MiB including callback storage                                 |
| Input value lifetime pool  | 1024 envelopes / 32 MiB of own storage; callbacks use the shared canonical native-callback pool |
| Received-value turn        | 256 work units / 64 KiB; one admitted larger scalar may consume a whole turn                    |
| Callback publication       | At most 16 independently receipted callback edits per envelope                                  |
| Published events           | 128 items / 2 MiB queued retained weight                                                        |
| Actor drain                | 64 events per service turn                                                                      |
| Worker turn                | At most 16 destination effects and 16 input publications                                        |
| Discovery workers          | 16 live/cancelling lifetimes, independent of socket workers                                     |
| Native interface inventory | 256 retained interface slots / 512-byte names, checked before app label conversion              |
| Interface catalog          | 64 retained options / 64 KiB; global 128 domains / 8 MiB                                        |
| Discovery mailboxes        | One coalesced refresh command; one latest event / 128 KiB                                       |

Capture retains the existing public snapshot/script-argument leases instead of copying values on
the actor. Snapshot, execution-event and script-argument ownership remains charged to its existing
canonical pools. The conservative
per-request reservation covers override decoding, prepared payload/codec storage and the destination
plan; collection spare storage is included in their bounds. Command overrides select borrowed parameter
values without cloning node metadata into snapshot overlays. Script admission checks cached retained
weight, count and borrowed address metadata. Command overrides and feedback values are materialized
only on the worker, after borrowed-value storage preflight. Addresses start with `/`, contain no NUL
and fit the address limit. Feedback paths are limited to 64 levels.

The complete destination plan and encoded datagram are validated before any DNS lookup or send.
A late invalid destination, oversized fan-out or wire packet rejects the whole request before
native effects. Target preparation yields every 64 edges so input service can progress. DNS runs
only on the native worker and may delay that worker; it cannot block an engine callback. Each
destination is attempted once in captured order. A failed destination does not replay successful
effects or prevent later destinations from receiving explicit outcomes. Exact source-lifetime and
transport-cancellation fences are checked again after DNS returns, before the native effect.

Command triggers and all three script aliases share this path. Tuple feedback coalesces by its
parent target and captures latest state; a full queue retains that state for a 250 ms retry.
Ordered command admission reports overload explicitly. Outgoing traffic and bounded logs follow
successful native sends; partial sends report an unconfirmed effect and are never retried.

The worker retains at most one command under execution, one decoded datagram continuation and
one pending reply for each of input and output in addition to its bounded mailboxes. Datagram
bytes and decoded shape are bounded before the recursive rosc codec runs. The framing guard
only enforces shape/storage policy; rosc remains the OSC implementation.

Full event storage pauses input rather than discarding decoded messages. A pending input
reply does not prevent independent output service. The rest of an admitted bundle is retained
through graceful retirement and terminal poll failure. Command/input continuations preserve
order; terminal setup failure closes admission and reports every accepted unsent command.
Native effects are never automatically replayed.

Incoming address splitting, payload conversion, callback JSON and bounded log preparation run on
the worker. Up to sixteen adjacent valid messages in one datagram share an envelope; decoder errors
remain in order between valid prefixes. Paths are limited to 64 segments and payloads retain the
128 KiB / 1024-value guard. The envelope includes callback weight before event admission. Canonical
leases move final value/JSON disposal off the actor and remain charged through ordinary event copies.

The worker keeps one measured draft when either value or callback lifetime capacity is full.
It retries admission without rebuilding JSON or discarding the native occurrence; independent
outgoing work continues. Capacity waits use the worker's coarse fallback deadline. Graceful and
terminal prefix completion retain that draft and the rest of the decoded datagram; explicit
cancellation interrupts the wait and discards the cancelled suffix.

The actor drains at most 64 events and stops after admitting one prepared input. It leaves later
inputs in the weighted worker mailbox while the retained input applies through `ReceivedValueCursor`.
Detached subtree creation, indexed-value pruning and value writes use that cursor's bounded turns
and edit completions. Failed edits abandon their unapplied suffix with an explicit diagnostic.
They do not replay an applied prefix. Once values settle, callbacks are submitted together in
captured order with independent publication receipts; only unpublished events retry. A later edit
or subscriber failure cannot duplicate a published callback. Counted feedback guards suppress every
internal appended write, including repeated writes to one parameter, and clear after delivery.

## Readiness, retirement and recovery

Starting a worker returns admission ownership, not a successful bind. The worker publishes
`Ready` with the actual local socket address; the module records consumption of that event
before reporting Connected. A scheduling notification is a hint and may legitimately produce
an empty drain. Fatal errors have a separate event and node warning from per-command errors.
Terminal receive/poll failure clears native readiness and notifies the actor immediately, even
while its accepted input prefix waits for publication capacity. Consuming a final input does not discard it merely because the
producer has already disconnected; disconnection is handled after that input completes.

Ordinary reconfiguration closes command admission and retains the old worker while it finishes
accepted work and cleanup. A replacement is opened after the old producer disconnects. Explicit
stop, disable, removal and owner drop cancel without a join. A blocked setup/cleanup retains
its lifetime slot; failed native wake never converts an admitted command into a reported
rejection. A 250 ms worker poll deadline provides fallback service for a failed native wake.
The module retries a failed lifetime after 250 ms instead of spawning on every engine tick.
The worker clears the native wake handle before releasing its lifetime slot; owner-side wake
calls borrow that handle without retaining a native strong reference.

The module uses `ModuleWake` and a passive execution rule, retaining its compiled-kernel identifier.
Unchanged discovery creates no actor wake, scheduled callback or idle snapshot. Socket and output
admission retries use 250 ms deadlines; failed discovery startup/worker exit retries after one
second. The earliest absolute deadline wins, so unrelated traffic cannot postpone recovery.
Only pending input, edits, catalog changes, configuration or feedback request further service.

Interface queries run once per second on their dedicated worker and can be prompted by a changed
selection. A short shared profile and one-slot wake coalesce selection changes; generation checks
discard old results. The provider runs outside that lock. A blocked query or provider destructor
retains its reserved lifetime while cancellation returns immediately to the actor. Native discovery
failure does not stop the independent socket service.

Catalogs retain a missing selected IPv4 address as an unavailable choice, preserving the configured
endpoint through removal and recovery. They never silently choose another interface. Unchanged
domains share the previous storage; the global payload pool tracks the exact domain Arc through
parameter, snapshot and history owners and disposes its final storage off the actor.

Core's generic `SetParamEnumOptions` applies only if the live UUID and selected value still match
the producer's observation. A stale or removed target is a successful no-op. It changes only the
live enum domain, preserving current labels and other constraints while reusing normal events,
history and undo/redo. The shared network-choice helper used by UDP also uses this intent instead
of replacing an entire parameter from an earlier observation.

Tests use fake sockets for blocked setup/cleanup/DNS, item/byte pressure, independent input/output
service, injected wake/poll failure, frozen capture, complete-plan rejection, retired sources and
accepted-prefix outcomes. Input tests cover canonical-pool pressure/recovery, a 700-value tuple,
trigger and callback multiplicity, bounded bundle preparation and publication failure after a
committed callback. UDP loopback covers native command/script output, vector casts, tuple feedback,
receive and bind-failure recovery. No physical devices or desktop control are required.
Discovery tests cover catalog quotas, missing-device recovery, unchanged storage, coalesced
generations, query failure recovery and blocked query/destructor lifetime ownership. Core and
shared-helper tests cover stale identity/selection, later edits, metadata preservation and undo/redo.
Passive module tests cover disable/enable output recovery and zero idle callbacks/snapshots.

## Remaining C03 work

MIDI native ownership and callback queues remain the other half of C03. Product-scale
latency/throughput and general parameter-callback conversion costs retain their owning gates;
these local ownership proofs do not establish a universal actor deadline.
The [status ledger](../progress/implementation-status.md) owns completion.

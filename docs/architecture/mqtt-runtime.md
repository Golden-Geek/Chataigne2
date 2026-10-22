# MQTT ownership

Chataigne owns MQTT behavior under `apps/chataigne/src/module/modules/protocol/mqtt`.
`golden_io` supplies reusable admission, notification, retained-payload and worker-lifetime
primitives. `rumqttc` remains the MQTT implementation; Golden packages contain no MQTT module policy.
The [implementation ledger](../progress/implementation-status.md) owns current acceptance and next work.

## Native protocol boundary

`transport/worker/mod.rs` owns connection setup, protocol polling, subscription admission, reconnect
backoff, native buffers and cleanup. One bounded worker retains its lifetime slot through teardown;
stop cancels out of band and never sends a stop command into a full queue or joins on the actor.
There are sixteen global live/retiring MQTT worker slots. A blocked native destructor keeps its slot.
The actor retries unavailable startup capacity through one coalesced deadline, from 250 ms to 8 s.
Native reconnect uses the same delay range; a connection setup attempt has its own one-second
library timeout, independent of ordinary connected polling.
One owned async protocol poll survives short service turns. A short wait expires without cancelling
the underlying connection or socket flush; another native operation cannot accumulate behind it.
The worker owns its current-thread runtime and drops pending protocol work before releasing payload
quotas. Protocol calls use rumqttc's public async API, not a second MQTT implementation.
Worker termination has a separate atomic signal, announced before native cleanup can block. The
actor can retire the failed handle and schedule recovery even when its event mailbox is full or its
destructor is stalled. The original worker slot and library-held payload reservations stay owned
until cleanup finishes; termination notification does not refund them.
DNS resolution runs serially on the same worker before the library is given a numeric address.
A blocked resolver retains that worker slot; timed-out connects cannot accumulate runtime DNS jobs.
Reconnect rotates through at most sixteen resolved addresses. Cancellation is checked after
resolution and before a new socket is polled.

`transport/admission.rs` validates configuration, packet sizes and retained capacities. Publish
admission includes wire headers inside the existing 1 MiB packet ceiling, 4096 topic bytes and
256 description bytes. Configuration bounds include 128 subscriptions, 1024 bytes per filter,
4096 bytes per credential field and 512 KiB total retained storage. Invalid inputs are rejected
before native setup or publication admission. Native diagnostics retain at most 1024 bytes.

The worker command mailbox has 100 entries / 16 MiB and services at most eight commands per turn.
Its event mailbox has 128 entries / 4 MiB. Separate global incoming and outgoing lifetime pools
each reserve 512 payloads / 64 MiB. Incoming packets keep their reservation after mailbox drain
and through app consumption. Outgoing reservations conservatively cover the retained request and
rumqttc's copy until the protocol disposition or teardown. Packet buffers and sixteen protocol
inflight slots have separate library bounds; these reservations are not whole-process RSS telemetry.
Full input capacity pauses the native reader with a cancellable wait; it never grows another queue.

## Accepted publication outcomes

`output/` captures canonical command snapshots/execute-event leases and Core-owned script arguments.
The actor performs bounded target inspection and queue admission; it does not clone payload text,
decode escapes or encode MQTT JSON. The existing native transport worker prepares each packet once,
validates its wire size, then preserves it across library backpressure. Command preparation borrows
only its four consumed fields and applies the last override without cloning a snapshot overlay.
Script JSON uses a bounded writer; prepared/library bytes and conversion scratch are reserved before
queue admission. Original snapshot, event and argument allocations keep their independent canonical
leases. Public commands, scripts and templates share this path.

Producer retirement before dispatch rejects queued output, including an already prepared packet
waiting for native capacity. Invalid topics, oversized encoded output and preparation failures report
an explicit **not sent** receipt and never dispatch an empty replacement packet. Refused command
admission logs the failure without rejecting unrelated queued edits. No output preparation worker or
second actor relay is introduced.

`transport/publications.rs` owns publish receipts independently of the event mailbox. Every
admitted command remains pending while queued or in protocol state. QoS 0 is explicitly dispatched
without broker confirmation. QoS 1 waits for PUBACK; QoS 2 waits for PUBCOMP. Retransmissions retain
the existing packet owner instead of consuming another queued command. A cancelled or failed worker
leaves an explicit unconfirmed outcome. Chataigne never automatically replays a failed publication.

A resumed broker session preserves protocol state. A new session clears the old library collision
and buffered-event identities through the public `MqttState` API and fails the dropped session's
receipts. Buffered state transitions from a failed native flush are accounted for before reconnect.
Native library payloads are destroyed before the tracking owners release their reservations, even
during panic or blocked cleanup. Controlled teardown reports unconfirmed in-flight work; this
runtime does not persist a protocol outbox across application restarts.

Outgoing activity follows a terminal protocol outcome. Optional outgoing logs are prepared natively
with a 256-byte payload preview. Incoming callbacks, command nodes, `publish`, `publishText` and
`publishJson` retain their app-owned contracts and script template.

## App service

`configuration/` owns bounded background configuration capture. The actor captures only a fixed
input key and a canonical snapshot lease; one shared preparation worker owns text validation,
subscription traversal, copies and exact configuration comparison. Thirty-two job reservations
include queued, running and ready work. Snapshot storage retains the public canonical lease
budget through cancellation and final disposal. Subscription traversal is iterative, preserves
authored order and rejects more than 8192 scanned nodes or 128 enabled subscriptions. Text limits
are checked on borrowed values before trimming or copying.

The key includes connection structure, the five connection scalars, credentials and subscriptions,
plus module UUID, effective enablement and engine lifetime. Runtime Connection/Connected and Values
traffic do not invalidate it. Inbox handling notices topology and authored changes; result application
also checks the current snapshot before using a prepared result. A stale or disabled preparation
cannot start a transport. Cancellation releases no queued job reservation prematurely and never joins.
Unexpected job destruction produces a retryable completion. Ready configurations retain their
payload admission across native-slot retries and transfer directly into the transport; unchanged
configurations reuse their existing lease. Transport startup adopts the shared configuration without
copying or comparing its text on the actor.

Fresh subscription managers create their default `#` subscription only when initially empty.
Loaded, duplicated and history-restored managers preserve deliberately empty authored subscriptions;
no recursive actor scan or load-time default reinsertion is performed. No persisted format changes.

`service.rs` owns passive worker wakes and coarse startup retry deadlines. Idle MQTT modules have
no periodic actor update and request no tree snapshot. An early retry wake preserves its deadline;
only configuration admission/completion, pending received data or a due retry asks for a snapshot.
An in-flight preparation waits for its completion notification without requesting actor snapshots.
Actor event and receipt service is capped at sixteen entries each. Local pending input and publish
receipt queues have explicit 64/100-entry limits. Receiving data, configuration changes and completed
receipts request continuations; pending protocol receipts alone do not busy-wake the actor.
Transport warnings live on the Connection group. Writing runtime feedback onto a watched connection
scalar would change its authored revision and invalidate the transport's own coarse retry deadline.

`input/` owns received-message preparation and delivery. A shared bounded worker parses payloads,
prepares callbacks and optional 256-byte log previews. Thirty-two preparation reservations include
queued, running and ready jobs. The actor captures only processing settings and source guards;
waiting preparation does not request snapshots. One pending draft waits for derived/callback capacity
without rebuilding its values or JSON. Cancellation and unexpected job destruction retain explicit
ownership and completion outcomes. Native protocol polling runs independently of this preparation.

Application input admits at most 64 KiB payload and 4096 topic bytes, independently of the protocol's
1 MiB packet safety ceiling. JSON uses the existing non-materializing validation pass with 16384
values and 24 levels before tree construction. Derived values admit 24 path levels, 8192 scalar/list
units and 4 MiB of source data. A prepared result, including its callback, must fit 16 MiB. The
derived pool admits 512 results / 64 MiB and charges a copy allowance for pending graph writes;
callbacks retain the shared app callback pool's independent lifetime admission. Oversized application
messages produce explicit warnings. A conversion failure retains the original raw callback with an
empty graph result and a diagnostic; Auto retains its existing text fallback.

`input/delivery.rs` uses the shared received-value cursor: at most 256 work units and a 64 KiB
value-copy allowance per turn, with an already-admitted large scalar receiving one whole turn.
The cursor waits for edit receipts before advancing. Applied prefixes are never reparsed or replayed,
and callbacks publish only after value completion/failure through their own receipt. Processing
fields, module lifetime and Values-root identity/placement are fenced before queued writes apply.
`input/targets.rs` also captures each existing write/replacement/removal target or insertion parent
and its ancestor path to Values. Shared ancestors are visited once, within a 282-read prefix bound.
Public read guards are split into groups of at most 32; every source/target check precedes every
prefix mutation. Replacement followed by child creation therefore uses the original binding checks
without invalidating later guards with its own edits. A moved, replaced, renamed or newly disabled
target rejects the queued prefix; ordinary scalar writes do not invalidate its structural binding.
The native prepared result reserves the worst-case queued read/receipt metadata before actor delivery,
and its existing edit-source lease retains that reservation through application or suffix discard.
Configuration/disable/destroy retire the source independently of blocked native cleanup. A prepared
result keeps its raw and derived reservations through delivery and queued edits, even after the
module is destroyed. Core's public `EditQueue::retain_payload` supplies only this generic lifetime
mechanism; app-specific capture, limits and MQTT callback policy stay here.

`payload.rs` owns only shared receive-callback and log formatting helpers. C04's local qualification
now includes pressure, passive receipts, failure/blocked-cleanup recovery, processing/session churn
and controlled actor service. The ledger records its acceptance matrix and exact artifacts.
Hardware/platform and full-product scale qualification remain with their later plan owners.

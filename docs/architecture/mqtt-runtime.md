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

`service.rs` owns passive worker wakes and coarse startup retry deadlines. Idle MQTT modules have
no periodic actor update and request no tree snapshot. An early retry wake preserves its deadline;
only authored configuration work, pending received data or a due retry asks for a snapshot.
Actor event and receipt service is capped at sixteen entries each. Local pending input and publish
receipt queues have explicit 64/100-entry limits. Receiving data, configuration changes and completed
receipts request continuations; pending protocol receipts alone do not busy-wake the actor.

`payload.rs` owns the app's payload modes and script argument conversions. These conversions,
incoming logging/callback preparation, configuration collection and graph-value publication still
run through the app actor. Their large-input, source-lifetime, pressure-feedback and edit-receipt
qualification is the remaining C04 work. The packet/queue caps and passive scheduling alone do not
qualify these actor paths. Follow the ledger rather than treating transport tests as whole-module
acceptance.

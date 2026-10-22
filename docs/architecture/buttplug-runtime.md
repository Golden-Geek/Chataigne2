# Buttplug worker ownership

Chataigne owns the Buttplug module, command nodes and script surface under
`apps/chataigne/src/module/modules/controllers/buttplug`. The app-owned `transport/`
boundary uses the locked Buttplug client and WebSocket transport crates for the protocol.
Golden supplies generic bounded workers, retained payloads, notifications and lifetime admission.

`transport/native.rs` owns the Tokio runtime, connection handshake, SDK event stream,
device inventory, output operations and cleanup. Construction and declaration materialization
do not open an endpoint. An engine-owned passive wake starts native work. The module has no
periodic actor callback; the worker polls the SDK stream with a ten-millisecond native timeout.

## Admission and lifetime

Sixteen global native slots include opening, active and retiring providers. Cancellation never
needs command-mailbox capacity or a join. Termination is announced and queued command
cancellations resolve before provider destruction; blocked cleanup retains the original slot.
The actor can recover independently, subject to the same fixed slot limit.

Each module also holds one connection lease through native destruction. Reconfiguration and
worker recovery wait on that lease with coarse actor deadlines. Native cleanup sends stop-all;
allowing a replacement connection to begin output before old cleanup finishes could let the
old connection stop the new output. The lease enforces this order without joining the actor.

Host/path/client-name limits are 1024/4096/256 retained bytes. The module checks borrowed
authored text before copying, allowing 4095 authored path bytes for a possible leading slash.
Command targets/output names/descriptions admit 1024/64/256 retained bytes. Device catalogs
admit 64 entries with 1024-byte names/display names and the fixed output capability vocabulary.
Server names and diagnostics admit 1024 bytes.

The command mailbox admits 64 records / 4 MiB. A captured source charges 64 KiB plus three
times execute-override storage for worker decoding scratch; configuration charges 8 KiB.
Global source/configuration lifetimes admit 512 records / 32 MiB. Each module admits 64 source
lifetimes across the internal inbox, outgoing queue, native work, completed receipts, queued
publication and disposal. The original source lease retains that admission throughout.
Failed disposer initialization remains retryable.

## Capture and preparation

Command nodes retain canonical snapshots and execute payloads. Scripts retain engine-owned
argument arrays and snapshots, with at most four arguments / 64 KiB. Both use typed local inbox
attachments, sorted by original engine time before native dispatch. There is no JSON request
relay or full snapshot overlay. The worker decodes the last matching execute override, checks
consumed text/numeric fields before copying/parsing, and resolves selected-device defaults and
the max-output clamp against the captured module snapshot.

Each source retains its producer node and actor/native/session revision guards. Removing or
disabling a source, replacing its transport, worker termination and connection loss invalidate
unstarted work. A source captured on a lost session cannot execute after reconnect. A full
native mailbox retains the same source behind an absolute 250 ms retry deadline; unrelated
wakes do not retry early and refused unpublished receipts do not emit cancellation wakes.
The actor submits at most eight captured sources per turn.

## Native events

The event mailbox admits 64 records / 4 MiB. Each event reserves a conservative 256 KiB,
so at most sixteen fit by weight. Global event lifetimes admit 128 records / 32 MiB.
The worker retains one pending event while waiting for capacity; it stops consuming native
input and cancellation wakes the wait. Native adapters return at most four records for one
SDK event and its catalog/state effects. Actor turns consume at most sixteen events and
sixteen completed receipts; disabled modules still finish receipt backlogs.

These bounds cover application-owned retained work. The locked client uses 256-item SDK
request/event channels; catalog size is exposed only after SDK enumeration. This is not an
RSS or transient SDK-allocation guarantee. The SDK event stream closes on broadcast receiver
errors, including lag; native stream closure becomes an explicit lost session and reconnect.

## Outcomes and recovery

Worker turns execute at most eight commands before checking native events. Handshake and
command waits have three-second timeouts. Retiring a native connection attempts stop-all and
disconnect within a two-second async timeout, with all cleanup remaining on the native worker.
An uninterruptible SDK call or destructor retains its lifetime slot.

Unstarted cancellation is **not sent**. A panic during a started action is **unconfirmed**.
Completed native success or failure is retained independently of event-mailbox pressure or
provider cleanup. Snapshot-free wakes keep completed receipts. Queued outcome edits carry the
module identity guard and original source lease through application/rejection; native/session
retirement does not erase a completed effect. Commands are never automatically replayed, including after a timeout or
partial device batch failure. Connection failures use 250 ms to five-second backoff;
worker-start failures and unexpected termination also schedule an absolute actor recovery
deadline. Unrelated wakes do not retry before that deadline.

Tests use controlled providers; native endpoint creation is disabled in test builds.
The [execution ledger](../progress/implementation-status.md) owns qualification and remaining
C07 acceptance. Queued native-event/callback/catalog publication lifetimes and fences, and full
package pressure/reconfiguration qualification remain open.

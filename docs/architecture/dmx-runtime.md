# DMX ownership

Chataigne owns Art-Net and sACN modules under `apps/chataigne/src/module/modules/protocol/dmx`.
`golden_io` supplies bounded workers, lifetime admission, retained payloads and notifications.
The existing `artnet_protocol` and `sacn` crates own wire encoding and protocol source behavior.
The [execution ledger](../progress/implementation-status.md) owns acceptance and remaining work.

## Native lifetime

`transport/native.rs` owns endpoint setup, sockets, packet conversion and sACN cleanup. Opening,
send/receive calls, stream termination and library destructors execute on the native worker.
`transport/worker.rs` services at most eight output commands and one input datagram per turn.
The existing five-millisecond receive wait remains; streaming semantics and receive throughput are P07.

Sixteen global worker slots include opening, active and retiring endpoints. Stop cancels out of
band, retires the input source and returns without joining or admitting a stop command. A blocked
setup or destructor retains its slot. Worker termination is announced before cleanup can block,
so the actor can schedule recovery while the old native lifetime remains reserved.

The output mailbox admits 64 frames / 1 MiB. Frame admission validates both length and capacity at
512 slots, protocol universe range, priority and the transport's single configured universe.
This also bounds sACN registered-universe bookkeeping. Separate global input/output pools admit
512 records / 8 MiB each, charging 16 KiB per record for frame, conversion and queued publication
storage. Library packet buffers and fixed endpoint state have separate bounds; these are not RSS claims.

## Outcomes and received state

`transport/receipts.rs` retains every admitted output through native completion or cancellation.
Sent means local native submission; UDP supplies no peer delivery acknowledgement. Failed or cancelled
commands report an unconfirmed outcome and are never automatically replayed. Pending actor receipts
are capped at 64 and serviced at most sixteen per turn. Outgoing activity follows native completion.

Input frames coalesce in one latest-state slot with explicit replacement accounting. Readiness,
pressure and failure use a separate bounded status mailbox, so frames cannot overwrite errors.
Native input preparation builds the bounded channel JSON and canonical `dmxFrameReceived` callback.
Callback payloads keep their existing independent shared admission. Input pressure reports a warning
and replacement count while keeping endpoint ownership intact.

Queued value/callback publication checks the input lifetime, module/target structure and five
configuration fields before writing. Core's public retained-edit payload contract keeps the original
input reservation through queued application or rejection. Stopping or reconfiguring closes the
source independently of native cleanup.

## Actor service and authoring

`service.rs` owns passive worker wakes and coalesced startup/recovery deadlines, from 250 ms to 8 s.
There is no periodic 120 Hz actor poll. Pending receipts and early retry wakes need no tree snapshot;
configuration changes and received graph publication request one. Disable retires the endpoint
immediately. Transport feedback belongs to the Connection group.

Frame JSON is bounded at 8192 bytes before parsing; endpoint text is bounded at 128 bytes before
address conversion. Commands update persisted output state after admission. Internal output changes
use Core parameter-event attribution instead of an ID-based ignore set, preserving later authored
changes after same-value updates. Existing command nodes, methods, callbacks and templates remain.

C05 remains open for command/script capture/preparation and complete engine pressure, failure and
reconfiguration qualification. P07 separately owns periodic refresh, filtering, sequencing, source
policy, synchronization and stream termination correctness. In particular, sacn 0.11 terminates via
multicast even after unicast sends; loopback tests suppress multicast delivery through its public
socket options while still exercising native destruction. They do not qualify termination semantics.

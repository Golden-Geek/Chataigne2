# DMX ownership

Chataigne owns Art-Net and sACN modules under `apps/chataigne/src/module/modules/protocol/dmx`.
`golden_io` supplies bounded workers, lifetime admission, retained payloads and notifications.
The existing `artnet_protocol` and `sacn` crates own wire encoding and protocol source behavior.
The [execution ledger](../progress/implementation-status.md) owns acceptance and remaining work.

## Native lifetime

`transport/native.rs` owns endpoint setup, sockets, packet conversion and sACN cleanup. Opening,
send/receive calls, stream termination and library destructors execute on the native worker.
`transport/worker.rs` services at most eight output commands, then at most 64 input datagrams or
two milliseconds of receive work per turn. Exhausting a receive budget returns immediately to
command/cancellation service; an empty socket uses the five-millisecond native idle wait.
Discarded traffic is distinct from an empty socket and cannot add an idle wait per datagram.

Native endpoints reuse one 64 KiB whole-UDP buffer, then check protocol size limits before
decoding: 1,024 bytes for Art-Net and 1,144 for sACN. This avoids treating Windows UDP truncation
errors as endpoint failures. The sixteen native slots therefore admit at most 1 MiB of this
scratch storage, separately from library state and retained payload quotas. Art-Net filters its
configured universe; non-DMX sACN START codes are ignored. Malformed/oversized datagrams are
consumed without retiring the endpoint and produce at most one warning per worker lifetime.
Actual socket errors still enter the existing explicit failure/reconnect path.

Sixteen global worker slots include opening, active and retiring endpoints. Stop cancels out of
band, retires the input source and returns without joining or admitting a stop command. A blocked
setup or destructor retains its slot. Worker termination is announced before cleanup can block,
so the actor can schedule recovery while the old native lifetime remains reserved.

The output mailbox admits 64 frames / 1 MiB. Frame admission validates both length and capacity at
512 slots, protocol universe range, priority and the transport's single configured universe.
The valid sACN priority range is 0–200; zero is accepted consistently by frame validation,
backend preparation and parameter metadata.
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
configuration changes, pending output and received graph publication request one. Disable retires the endpoint
immediately. Transport feedback belongs to the Connection group.

Endpoint text is bounded at 128 bytes before address conversion. Commands update persisted output
state after native-mailbox admission. Internal output changes
use Core parameter-event attribution instead of an ID-based ignore set, preserving later authored
changes after same-value updates. Existing command nodes, methods, callbacks and templates remain.

### Captured output

`outgoing/` captures command snapshots and execute overrides through their canonical Core leases;
scripts share their original Core-owned argument arrays. The actor resolves fixed configuration
fields and admits at most 64 pending preparations/native receipts per module. Global invocation
admission bounds 512 records / 16 MiB, charging 16 KiB plus three times execute-override storage
for worker scratch. Canonical snapshot/event/argument storage keeps its independent Core quotas.
Scripts admit at most two arguments / 64 KiB of retained input.

One shared preparation worker has 32 job tickets, including cancelled/retiring jobs. It decodes
only consumed command fields, borrowing authored values and applying the last matching override;
there is no whole-snapshot overlay or JSON command relay. Frame JSON is bounded at 8192 bytes,
and numeric text at 32 bytes, before parsing. Prepared records have a separate 512-item / 8 MiB
pool, charged at 16 KiB per operation. Their original invocation lease follows native submission,
completion/cancellation and queued output projection through the existing retained-edit contract.

The actor applies at most eight fixed-size prepared operations per turn. Engine timestamps order
property occurrences, script calls and command invocations before preparation. Property capture
copies only the bounded event text, because a snapshot may already contain a later authored edit.
Channel changes preserve the preceding frame; snapshots provide the initial authored baseline.
Only the final queued command reflects output state, behind configuration/output read fences.
A newer authored frame takes precedence over an older asynchronous reflection.

Temporary native-mailbox/payload pressure retains the same prepared operation behind an absolute
250 ms retry, without reparsing or retrying early on unrelated input wakes. Refused enqueue
disarms the unpublished receipt so its destructor cannot create a readiness feedback loop.
Producer lifetime, configuration identity and native session fence
preparation/application; the native worker checks producer/session lifetime again before sending.
A retired source reports **not sent**, separately from an unconfirmed native failure. Neither is
automatically replayed on reconnect.

C05 qualification is tracked in the execution ledger. P07 separately owns periodic refresh, sequencing, source
policy, synchronization and stream termination correctness. In particular, sacn 0.11 terminates via
multicast even after unicast sends; loopback tests suppress multicast delivery through its public
socket options while still exercising native destruction. They do not qualify termination semantics.

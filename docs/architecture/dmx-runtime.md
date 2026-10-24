# DMX ownership

Chataigne owns Art-Net and sACN modules under `apps/chataigne/src/module/modules/protocol/dmx`.
`golden_io` supplies bounded workers, lifetime admission, retained payloads and notifications.
`artnet_protocol` 0.4.4 owns Art-Net wire encoding. `x39-sacn` 0.2.1 (imported as `sacn`)
owns the sACN codec and clocked source state machine. Its std-only protocol core is used without
the Tokio adapter; no extra socket owner or runtime is introduced.
Versioned assumptions are exercised against the actual imported core and decoded loopback wire
packets. References: [x39-sacn 0.2.1](https://crates.io/crates/x39-sacn/0.2.1) and the
[Art-Net 4 specification, revision 1.4dp](https://art-net.org.uk/downloads/art-net.pdf).
The [execution ledger](../progress/implementation-status.md) owns acceptance and remaining work.

## Native lifetime

`transport/native.rs` owns endpoint setup, sockets, packet conversion and sACN cleanup. Opening,
send/receive calls, stream termination and library destructors execute on the native worker.
`transport/worker.rs` services at most eight output commands, then at most 64 input datagrams or
two milliseconds of receive work per turn. Exhausting a receive budget returns immediately to
command/cancellation service; an empty receiving socket uses the five-millisecond native idle wait.
Discarded traffic is distinct from an empty socket and cannot add an idle wait per datagram.
Output-only workers wait for commands or the next stream deadline, without a recurring receive poll.

Native endpoints reuse one 64 KiB whole-UDP buffer, then check protocol size limits before
decoding: 1,024 bytes for Art-Net and 1,144 for sACN. This avoids treating Windows UDP truncation
errors as endpoint failures. The sixteen native slots therefore admit at most 1 MiB of this
scratch storage, separately from library state and retained payload quotas. Art-Net filters its
configured universe; unsupported sACN START codes are ignored. Malformed/oversized datagrams are
consumed without retiring the endpoint and produce at most one warning per worker lifetime.
Actual socket errors still enter the existing explicit failure/reconnect path.

## Receive source state

`transport/input` owns caller-clocked receive state. Art-Net uses the established codec, two
IP/Physical source slots and HTP merging. Its sequence filter handles non-zero wrap and disabled
ordering; ArtSync follows the current controller, is ignored across different-IP merges and
returns to live input after four seconds. Ten-second source expiry frees admission; losing every
source holds the last published look.

sACN uses eight admitted CIDs, with a fixed-capacity protocol receiver per CID and the crate's
fixed-capacity merger for the combined look. This composition matters: x39-sacn 0.2.1's combined
receiver keys synchronization by address and drops the synchronization CID and sequence before
release. The app scopes those state machines per CID and rejects unrelated/old sync packets at
admission. The library continues to own data sequencing, source loss, per-address priority,
synchronization buffering and timeout policy. Each CID admits eight synchronization-history
addresses; current multicast interest remains at most one address per CID. Native membership
updates leave obsolete groups before joining replacements, bounding memberships through churn
to eight sync groups plus the configured data group. Output-only endpoints allocate no receive state.

The module selects one local interface, so input starts without interface sampling or a wait
for potential per-address-priority data. Actual PAP packets remain supported. Preview and other
START codes are filtered. Source overflow is rejected with a bounded diagnostic; source loss or
termination frees its slot. If every source is lost, the module publishes its held look with
`metadata.stale` and a separate input warning; valid input clears that warning.

Receive frames have their own typed metadata instead of reusing output packet metadata. The
existing callback keeps `(universe, channels, metadata)`, adding bounded source identities,
effective sACN slot priorities, synchronization and stale flags. A combined result has no invented
wire sequence or singular priority: these fields are nullable. Raw universe priorities remain
on the source entries. The app-owned callback snippet documents these fields.

Sixteen global worker slots include opening, active and retiring endpoints. Stop cancels out of
band, retires the input source and returns without joining or admitting a stop command. A blocked
setup or destructor retains its slot. Worker termination is announced before cleanup can block,
so the actor can schedule recovery while the old native lifetime remains reserved.

The output mailbox admits 64 frames / 1 MiB. Frame admission validates both length and capacity at
512 slots, protocol universe range, priority and the transport's single configured universe.
The valid sACN priority range is 0–200; zero is accepted consistently by frame validation,
backend preparation and parameter metadata.
The sACN source uses the crate's fixed-capacity storage policy: one transmit universe, its derived
bounded pending/transmission collections, and no receiver or discovery-catalog storage. Separate global input/output pools admit
512 records / 8 MiB each, charging 16 KiB per record for frame, conversion and queued publication
storage. Library packet buffers and fixed endpoint state have separate bounds; these are not RSS claims.

## Clocked output

`transport/output.rs` owns one universe's last look, sequence and caller-supplied elapsed time.
`transport/native.rs` supplies monotonic time and UDP submission. Art-Net uses non-zero sequence
numbers with 255-to-1 wrap, a 23 ms minimum explicit-send interval, and a 900 ms static refresh.
The sACN core owns its three-packet pre-suppression burst, 900 ms keepalive,
sequence wrap, discovery and three-packet termination. A source CID comes from the app node's
persistent UUID, so reopening the endpoint keeps its identity. `transport/continuity.rs` retains
the pure source state and last confirmed look across matching endpoint lifetimes. Old admitted
commands and their receipts are never replayed automatically.
The caller applies a conservative 23 ms floor to all universe data packets, including sACN's
scheduled bursts and termination; the core's 22 ms quantum remains an earliest poll hint.
Returned deadlines include this floor so a suppressed timer cannot spin on a past deadline.

The worker retains at most eight ordered commands outside its bounded mailbox while waiting
for the send clock. Each keeps the original payload/source lease and receipt; explicit identical
frames and blackout are submitted separately. Input/cancellation service continues between due
sends. Automatic refresh retains one already-submitted look independently of command receipts.
Only actual local datagram submission completes a receipt as Sent.

Live priority edits share that bounded FIFO as typed controls. The native worker applies them
to confirmed channels, so a metadata edit cannot resurrect an unconfirmed frame. A control with
no confirmed look completes as Applied, emits no packet and does not consume authored
initialization. Frame and control submissions share the data clock and retain their original
source leases. Art-Net retains priority as metadata; sACN sends it on the wire.

sACN unicast uses the configured destination for both ordinary data and stream termination;
discovery packets are skipped for a unicast-only endpoint. Multicast output uses the crate's
IPv4/IPv6 universe address helpers. Shutdown drains termination on the retiring native worker,
with a 200 ms ceiling; it never waits on the engine. Submission failures retire through the
existing explicit recovery path. The execution ledger records the remaining qualification.

### Recovery ownership

The app module owns `OutputContinuity`. Its current stream is keyed by protocol, CID and
universe; up to sixteen weak references let still-retiring streams retain sequence state during
rapid route/universe changes. Those weak references do not retain old protocol storage. Socket
opening and protocol initialization remain native work. Actor lifetime claims and authored
restarts use only atomics and bounded weak-reference bookkeeping; the actor never locks native
protocol state or joins cleanup.

A successful local submission confirms one recovery look. An I/O error or panic suspends the
source without terminating it, retaining its sequence, clock and confirmed channels. The next
matching endpoint restores that look at the existing send clock. Failed or cancelled queued
frames cannot become a recovery look; failure before the first confirmed frame stays silent
until a new intent. Unconfirmed attempts still consume their sequence and pacing slot.
Recovery restores those confirmed channels with the current authored priority. A separate atomic
priority value lets edits during socket opening reach native activation without actor/native
locking; it does not change the metadata of older commands already queued in the active lifetime.

Native activation, rather than a potentially failed socket-open reservation, transfers egress
ownership. The final native submission gate orders packets across distinct protocol locks.
Old cleanup can finish termination while a replacement is opening, but cannot emit after that
replacement activates. Deliberate shutdown drains termination with no locks held across timed
waits, rechecks its 200 ms budget after waking, and stops if a replacement takes ownership.

Fresh/loaded/re-enabled authored state initializes once through the ordinary guarded frame
preparation path. Current explicit intent defers initialization and consumes it on native-mailbox
admission. Retired receipts do not consume it. Monotonic authored revisions prevent an old
in-flight attempt from consuming or rearming a newer lifecycle's initialization. Re-enabling or
restoring an authored lifecycle uses its current saved channels; automatic socket recovery uses
the last confirmed channels. Neither path invokes old script/command calls or changes their
original Sent, NotSent or unconfirmed outcomes. Empty authored channels do not start a stream.

## Outcomes and received state

`transport/receipts.rs` retains every admitted output through native completion or cancellation.
Sent means local native submission; UDP supplies no peer delivery acknowledgement. Failed or cancelled
commands report an unconfirmed outcome and are never automatically replayed. Pending actor receipts
are capped at 64 and serviced at most sixteen per turn. Outgoing activity follows native completion.

Input frames coalesce in one latest-state slot with explicit replacement accounting. Readiness,
pressure and failure use separate bounded status state, so frames cannot overwrite errors.
Readiness, the latest warning and current input health coalesce independently. A terminal error
supersedes older health and survives subsequent updates. Two diagnostics of at most 1 KiB each
bound storage; the native worker never waits for actor consumption to publish health.
Native input preparation retains bounded typed slots and builds the canonical `dmxFrameReceived` callback.
Callback payloads keep their existing independent shared admission. Input pressure reports a warning
and replacement count while keeping endpoint ownership intact.

Queued value/callback publication checks the input lifetime, module/target structure and five
configuration fields before writing. Core's public retained-edit payload contract keeps the original
input reservation through queued application or rejection. Stopping or reconfiguring closes the
source independently of native cleanup.

## Actor service and authoring

`service.rs` owns passive worker wakes and coalesced startup/recovery deadlines, from 250 ms to 8 s.
Repeated native open failures grow that backoff through the cap. Worker admission does not reset
it; actual native readiness does.
The module re-arms any later owned deadline after an earlier coalesced wake. Readiness clears
obsolete connection deadlines while retaining a live output retry; unrelated input cannot push
an absolute retry deadline into the future.
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
there is no whole-snapshot overlay or JSON command relay. Frame buffers are typed bytes, bounded
at 512 slots. Explicit `sendFrame` JSON strings remain bounded at 8192 bytes, and numeric text at
32 bytes, before parsing. Prepared records have a separate 512-item / 8 MiB
pool, charged at 16 KiB per operation. Their original invocation lease follows native submission,
completion/cancellation and queued output projection through the existing retained-edit contract.

The actor applies at most eight fixed-size prepared operations per turn. Engine timestamps order
property occurrences, script calls and command invocations before preparation. Property capture
copies only the bounded event slots, because a snapshot may already contain a later authored edit.
The first later priority occurrence's old value supplies metadata for an earlier property frame.
Deferred command capture may also see later authored metadata: the first later control in the
ordered queue supplies that command's occurrence priority. Script calls keep their prefix snapshot
metadata. These lookups inspect only the admitted prefix and the current inbox. Priority controls
never rewrite persisted channels or suppress a preceding command's final reflection. Internal
echo suppression applies only to channel reflection, so module-attributed priority edits still work.
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

C05 and P07 qualification are tracked in the execution ledger. Receive source/sync state and
termination/recovery are qualified locally. P07's final priority-control and complete local
acceptance gates are also locally verified. The replaced sacn 0.11 multicast-only termination behavior is removed.

### Typed frame publication

Golden's generic `ByteArray` / `ParamValue::Bytes` owns binary parameter storage, persistence,
script conversion and generated protocol transport. Its canonical runtime extension contains
raw bytes. DMX keeps the 512-channel policy and channel numbering in the app layer. Output,
received and Send Frame command parameters use this type; the generic inspector/dashboard
editor accepts decimal bytes separated by spaces or commas.

`setChannels(start, { Bytes: [...] })` and the Set DMX Channels command apply a contiguous range
as one ordered operation and one universe frame. The range must be nonempty and fit within
channels 1..512. Scalar `setChannel` ordering remains explicit. The preparation worker borrows
the typed authored baseline, and only the final queued command projects typed output state.
No internal sample reparses or persists a JSON channel string. The existing receive callback
keeps its `(universe, channels, metadata)` shape and is prepared at the native boundary.

The public `Node::project_migrate_children` persistence hook runs before sparse declaration
matching and dense child decoding. Unchanged records are borrowed. DMX's narrow migration
changes only its former string frame parameters and their saved/default values, preserving
UUIDs and metadata. Ordinary string parameters do not gain implicit binary coercion. New script
arrays use `{ Bytes: [...] }` to preserve existing untagged vector/color array semantics;
explicit `sendFrame("[...]")` calls remain supported at the script boundary.

## Local qualification map

All paths below are relative to the app's `src/module/modules/protocol/dmx` directory. The
execution ledger records the exact artifact, results and remaining plan scope.

| P07 contract | Owning qualification |
| --- | --- |
| Static looks past source-loss timeout | `transport/output/tests` drives eight seconds of fake time for both protocols; `transport/tests/stream.rs` exercises native deadline-driven refresh. |
| Universe/START-code filtering and bursts | `transport/native/tests` checks real datagrams; `transport/tests/lifecycle.rs` checks continuous malformed/ignored input alongside output. |
| CID, sequencing and priority | `transport/output/tests` covers wrap/pacing; `transport/input/tests` covers source merging/PAP; `outgoing/tests/priority.rs` covers ordered live control and retained native metadata. |
| Synchronized input | `transport/input/tests` covers source/address/sequence fences, merged Art-Net controllers, independent sACN synchronization addresses and hold/live timeouts. |
| Explicit frames, batches and blackout | `outgoing/tests` checks property/script/command order and atomic ranges; output and native stream suites preserve every explicit submission. |
| Shutdown and termination | Output fake time verifies three spaced sACN termination packets; native streams verify the original unicast route and replacement-owner fences. |
| Overload and source retention | Lifecycle suites check bounded mailbox/payload/preparation admission, retained queued publication, cancellation and immutable receipts. |
| Recovery without actor blocking | Continuity and engine lifecycle suites check confirmed-only restoration, current priority, failed opening/backoff, actual coalesced deadlines, blocked cleanup and passive idle turns. |

These checks use deterministic clocks, controlled workers and IPv4 loopback. Native webview,
other operating systems, physical interfaces and named lighting devices keep their P17/P18 owners.

# MIDI runtime and authored structure ownership

MIDI is app-owned under `apps/chataigne/src/module/modules/protocol/midi/midi_module`.
`midi_runtime` owns native Midir discovery, input/output creation, port resolution, sends,
presence checks and final cleanup. Midir and Midly remain the device and codec implementations.
Generic admission, bounded mailboxes, cancellation and lifetime reservation come from `golden_io`.

The module owns authored configuration, graph values, CC modes, clock/MTC state, commands,
script callbacks and feedback. Its focused `incoming`, `structure`, `timing`, `outgoing`,
`events` and `transport` modules keep those responsibilities separate from native ownership.
`helpers` holds shared value-tree policy and `script_values` holds callback/scalar conversion.
`midi_output` captures canonical command/script sources; `commands/preparation` builds the thirteen
command types on the output worker. `midi_runtime/output/schedule` owns complete outgoing batches
and their monotonic deadlines. This closes outgoing preparation and delayed storage/service.
`midi_input` prepares immutable incoming values, callback payloads and logs on the input worker.
`input_delivery` owns graph and callback completion without replaying applied prefixes.
`structure/cc` owns bounded authored CC conversion and `structure/locks` owns startup surface
permissions. `midi_runtime/discovery` prepares and retains port choices; `catalog_delivery`
publishes them through guarded edit receipts. Passive module service remains open in C03.

## Native admission

| Resource                        | Limit                                                                                                                         |
| ------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| Live/cancelling native workers  | 32 shared input/output/discovery lifetimes, reserved before thread creation                                                   |
| Selected port identifier        | 1024 retained bytes                                                                                                           |
| Native port inventory           | 64 retained ports per direction; 512-byte native names                                                                        |
| Discovery metadata              | 1024-byte variant IDs / 512-byte labels / 128-byte details                                                                    |
| Discovery publication           | One result slot / 1 MiB; one pending worker result and one actor delivery; previous/observed catalogs share canonical choices |
| Prepared port choices           | 66 options / 256 KiB per direction, including None and a missing selected port                                                |
| Canonical choice lifetimes      | Global 128 direction catalogs / 32 MiB, including engine parameters, snapshots, history and pending edits                     |
| Callback ingress                | 256 messages / 2 MiB retained weight                                                                                          |
| MIDI message or outgoing packet | 64 KiB                                                                                                                        |
| Prepared input events           | 256 events / 16 MiB, including decoded SysEx, prepared hex and both callback payloads                                         |
| Worker input turn               | 32 raw messages, with cancellation-aware publication capacity waits                                                           |
| Actor input admission           | At most 32 statuses or the first message; one graph cursor and sixteen callback-waiting deliveries                            |
| Output commands                 | 256 requests / 2 MiB queued; captured command/script preparation reserves 1 MiB each                                          |
| Live output requests            | 2048 per output lifetime; global 4096 requests / 128 MiB including queued, preparing and scheduled work                       |
| One output batch                | One or two packets, each 64 KiB; complete validation precedes every first native effect                                       |
| Worker output turn              | One source preparation and at most 16 native effects                                                                          |
| Output events                   | 256 replies / 2 MiB                                                                                                           |
| Actor output drain              | 32 events per active or retiring handle                                                                                       |
| Output description / log bytes  | 512-byte description; log displays at most the first 256 payload bytes                                                        |

The native input callback timestamps arrival and copies only messages within the byte limit.
It never waits for downstream capacity. Full or oversized ingress is rejected before runtime
admission and counted explicitly; accepted messages remain ordered. Decode and 24-tick clock
analysis run on the input worker. BPM uses captured arrival times, so publication pressure does
not turn worker service timing into source timing. All bytes and decoded SysEx capacity count
toward input event admission. Undelivered callback storage and native shutdown remain worker-owned.
Prepared events use 256 slots / 16 MiB per native lifetime, including callback payload weights.
The canonical input disposer admits 1024 envelopes / 64 MiB of message, hex and envelope storage;
callbacks use the shared native callback disposer (16384 payloads / 256 MiB). Ordinary event/replay
copies keep the same callback reservation after the input envelope retires. A preparing worker
retains one measured draft and its admitted callback prefix while capacity is unavailable,
without reconstructing JSON or losing the accepted message. Cancellation releases that draft
on the worker.

## Input publication

The actor holds one graph delivery and at most sixteen deliveries awaiting callback receipts.
It stops consuming native messages at that limit. A message advances note/CC/clock/MTC domain
state once and produces at most twelve scalar graph operations. A graph service turn admits
at most sixteen operations / 64 KiB; a single bounded SysEx string may occupy its own turn.
Touched value-tree parents are limited to 256 children. A complete missing path is one detached
`NodeTree`, including a CC's rotary selector. Numeric insertion uses a predecessor lookup;
only a new minimum needs one follow-up move after its runtime ID is known.

An owning edit receipt fences each graph segment. Failure abandons its unapplied suffix with
a diagnostic and never replays an applied trigger or relative increment. A newly created scalar
is initialized once; a newly created trigger receives one explicit occurrence after creation.
Changes to auto-add, channel CC widths or BPM precision cancel unapplied graph work. A changed
rotary mechanism or disappeared relative CC target also cancels its stale operation.

Generic and message-specific callbacks have separate completion receipts. A failed publication
retries only the unpublished callback; later edit/subscriber failures cannot replay a committed
one. Callback receipts retain canonical payloads and do not stall subsequent clock inputs after
their graph segment settles. Disable/removal cancels pending graph/callback ownership and clears
held-note state. Accepted authored structure work has a separate lifetime and may finish disabled.

The input worker services its callback mailbox with a 10 ms background fallback, independently
of the actor. Presence checks run once per second on that worker. This is not an audio callback
or a product latency qualification.

## Authored structure

CC width changes coalesce by channel and retain the first unapplied width for rejection recovery.
One channel's fixed 32 controller pairs is prepared per service turn. Every read fence precedes
authored effects; data plans contain at most 200 requests plus one ordering callback. Touched
parents admit at most 256 children, and existing parameter publication plus retired subtrees
share a 256-node footprint. Oversized or unsupported shapes reject before any CC data changes.
New parameters and rotary selectors are complete detached `NodeTree` values.

An existing parameter's value, default and constraints change together in one public node mutation.
Normalization completes before assignment, so a later drain failure cannot leave a widened range
with an unconverted value. Golden records the change as one state/history effect and attributes
its parameter event to the module. Follow-up edits retain their own provenance.

Receipts separate read rejection, applied data and ordering. Read rejection restores an explicit
unapplied width if no newer request supersedes it. After an applied prefix, a fresh plan uses each
parameter's actual declared 7-bit or 14-bit range, preserving completed conversions without shifting
twice. Narrowing writes the low-bit control before replacing the combined source, so an interrupted
split retains all bits for reconciliation. Startup reconciliation uses the same idempotent path. Numeric ordering uses a separate
bounded pass of at most 129 requests; one deferred module callback starts it after the data drain.
No per-parameter snapshot callback or whole-tree sorting loop is needed.

Startup permission locking follows only the known MIDI declaration shape. Each turn visits at
most 64 cursor steps and emits at most 24 patches behind one read fence. The cursor keeps at
most five ancestry frames and does not traverse control-owned subtrees. Failed metadata receipts
recheck the current owned surface. Pending receipts request no new snapshot or repeated edits.

Input consumption pauses during structure work. Worker wakes service accepted structure even
while disabled, without reopening native IO. Removal cancels the cursor and invalidates deferred
callbacks through the module generation and closed wake source. The first coalesced callback
also has a completion receipt: discarding it cannot leave a permanent queued flag or pause input
after the retained plan completes. The overall module's periodic
native/clock service remains a separate unfinished item below.

## Outgoing preparation

Command nodes and scripts retain frozen snapshot/override/argument leases rather than copying
values, serializing a module request or parsing bytes on the actor. Source leases keep their existing
canonical budgets. MIDI adds a conservative 1 MiB preparation reservation before output admission;
feedback has an 8 KiB reservation and at most two fixed-size messages. The complete packet plan,
retained capacities, descriptions and delayed deadlines are checked before any native effect.
Command/script sources are limited to 256 KiB; byte parsing limits text to 256 KiB and output to
64 KiB. Exact SysEx storage includes required F0/F7 bytes. Selectors have a 128-byte guard, and
extreme octave arithmetic rejects safely. Command overrides use borrowed parameter values without
cloning a snapshot overlay. All thirteen commands and twelve script aliases share this path.

After preparation, unused reservation bytes are released. The measured plan retains a budget
lease for its full lifetime, including a 4 KiB allocation/scheduler allowance. Private batch storage
never grows beyond its admitted shape and is destroyed on the native worker before that lease
releases. The per-output quota also covers commands that have left the mailbox but retain a future
packet. Full queues or exhausted global/local capacity reject the whole request before transfer.

A batch uses capture-time monotonic deadlines, with the authored i32 millisecond limit
(2147483647 ms) and checked native deadline construction. The worker services due packets in
stable deadline/arrival order. Zero-delay pairs remain ordered; later immediate requests can run
while an older batch waits for its note-off. Actor ticks are not needed for delayed transmission.
The actor's unbounded delayed-packet vector and serialized MIDI module-request consumer are removed.

Success means queued, not transmitted. Only successful native replies produce outgoing traffic
and bounded logs; a later success clears an earlier output warning. A failed send has an unconfirmed
native outcome and is never replayed. Admission closes immediately; accepted unsent requests/batch
suffixes receive explicit not-sent outcomes. Full reply storage retains the current receipt and
backpressures native service without another unbounded queue.

## Feedback

Received values, clock/MTC publications and CC conversion writes carry the module's runtime
producer ID through Golden's [parameter event attribution](application-contracts.md#parameter-event-attribution).
The module suppresses only changes from that producer. There is no ignore-next-change map:
unchanged writes and rejected suffixes cannot swallow later authored changes. Constraint-induced
clamping carries the same provenance as its owning request. Undo/redo has no received-input source.

Feedback uses each event's captured new value and old value. Several appended note/CC writes
therefore retain their individual values, relative deltas and 14-bit packet pairs even when the
tree snapshot already contains the final value. The resolver accepts fixed MIDI value-tree paths
and typed scalars, without arbitrary ancestor traversal or string-to-number parsing. Clock/system
triggers retain their occurrences. Each feedback request retains the existing 8 KiB admission
reservation and native complete-batch guarantees; rejected output admission reports explicitly.

## Lifetime and recovery

Spawning a worker does not report a successful connection. Connected state requires native
readiness for every selected direction. Setup failure, worker exit and native device loss have
explicit diagnostics. Readiness clears before failure publication or blocked cleanup, with a
worker notification independent of reply capacity; an unwind guard also clears readiness.

Ordinary output reconfiguration closes old admission, drains its accepted sends and receipts,
and waits for producer disconnection before opening a replacement. Scheduled follow-ups stay on
the admitted native lifetime and their original deadlines; they are never redirected to a new port. A retiring handle does not
force repeated tree snapshots while native work is blocked. Disable, removal and explicit stop
cancel without a join. A blocked setup, send, query or destructor retains its lifetime slot until
it returns, so repeated reconfiguration cannot create unlimited threads. Module retry remains
on its one-second refresh path at this stage.

Discovery runs independently once per second, publishes only changed results and recovers from
query errors. Native inventory count and name storage are checked before app-owned conversion.
Availability and connection resolution use the same port identity rule: an indexed selection
cannot silently bind another index just because its label matches. Unindexed label selections
retain their existing resolution semantics. Missing selected ports stay configured and visible.

Choice updates use Core's guarded `SetParamEnumOptions`, preserving live metadata and newer
selections instead of replacing whole parameters. The discovery worker builds choices and the
missing-selection row after admitting the complete raw inventory. Selection strings are limited
to 1024 bytes. A coalesced one-slot command captures the latest input/output selection profile;
generation-tagged results from an older profile cannot replace its newer selection.

Prepared choices use `ParameterEnumOptions` and the public `PayloadPool::try_retain_arc` contract.
Unchanged choices reuse the previous canonical identity. The pool retains each immutable choice
array through its last external owner, so dropping a module, edit or snapshot cannot release its
quota early or run the final array destructor on the actor. Exhausted lifetime capacity reports
an explicit diagnostic; discovery retries and can recover after owners release old catalogs.
Disposer startup failure is also retryable rather than cached permanently.

One actor delivery publishes at most six edits for both directions. Read fences cover choice
updates and warning clears. Receipts retain the same canonical arrays after rejected or discarded
edits, while pending receipts request no repeated snapshot or publication. Results whose selection
has changed are discarded. Discovery uses worker wakes for result publication; disable/removal
cancels discovery and drops its pending delivery. Ordinary discovery restart still uses the
module's existing one-second refresh path until passive scheduling is completed.

A source that retires before its first native effect cancels the whole request. Once a first effect
succeeds, the worker finishes that admitted batch even if its command node disappears, so deleting
a command cannot discard an already-started note's note-off. Explicit module disable/removal or
transport cancellation still cancels the remaining suffix without replaying an in-flight send.

## Remaining C03 acceptance

MIDI still needs passive module scheduling with clock/MTC/recovery deadlines.
Output deadlines and catalog preparation/lifetimes are already worker-owned.
The existing 240 Hz module rule is still present; native worker ownership does not qualify it.
Keep those concrete gaps inside C03, preserving all commands and app-owned script surfaces.
General snapshot scaling and product-size throughput remain their owning plan tasks.

Tests use fake drivers and controlled setup/send/query/destructor gates, without connecting
physical endpoints. Module, codec and command preservation tests exercise the authored product
surfaces. The [status ledger](../progress/implementation-status.md) owns current evidence and
package completion; fake-provider tests do not qualify hardware or other platforms.

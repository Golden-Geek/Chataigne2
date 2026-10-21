# MIDI native and outgoing ownership

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
and their monotonic deadlines. This closes outgoing preparation and delayed storage/service;
input publication, feedback accounting, catalogs and passive module service remain open in C03.

## Native admission

| Resource                        | Limit                                                                                                                   |
| ------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| Live/cancelling native workers  | 32 shared input/output/discovery lifetimes, reserved before thread creation                                             |
| Selected port identifier        | 1024 retained bytes                                                                                                     |
| Native port inventory           | 64 retained ports per direction; 512-byte native names                                                                  |
| Discovery metadata              | 1024-byte variant IDs / 512-byte labels / 128-byte details                                                              |
| Discovery publication           | One catalog result / 256 KiB; bounded current, previous and pending copies (at most four results including the mailbox) |
| Callback ingress                | 256 messages / 2 MiB retained weight                                                                                    |
| MIDI message or outgoing packet | 64 KiB                                                                                                                  |
| Prepared input events           | 256 events / 4 MiB, including duplicate decoded SysEx storage                                                           |
| Worker input turn               | 32 raw messages, with cancellation-aware publication capacity waits                                                     |
| Actor input admission           | One batch of at most 32 events; no further drain until retained graph work clears                                       |
| Output commands                 | 256 requests / 2 MiB queued; captured command/script preparation reserves 1 MiB each                                    |
| Live output requests            | 2048 per output lifetime; global 4096 requests / 128 MiB including queued, preparing and scheduled work                 |
| One output batch                | One or two packets, each 64 KiB; complete validation precedes every first native effect                                 |
| Worker output turn              | One source preparation and at most 16 native effects                                                                    |
| Output events                   | 256 replies / 2 MiB                                                                                                     |
| Actor output drain              | 32 events per active or retiring handle                                                                                 |
| Output description / log bytes  | 512-byte description; log displays at most the first 256 payload bytes                                                  |

The native input callback timestamps arrival and copies only messages within the byte limit.
It never waits for downstream capacity. Full or oversized ingress is rejected before runtime
admission and counted explicitly; accepted messages remain ordered. Decode and 24-tick clock
analysis run on the input worker. BPM uses captured arrival times, so publication pressure does
not turn worker service timing into source timing. All bytes and decoded SysEx capacity count
toward input event admission. Undelivered callback storage and native shutdown remain worker-owned;
canonical payload disposal through actor values/callbacks remains part of the actor closure.

The input worker services its callback mailbox with a 10 ms background fallback, independently
of the actor. Presence checks run once per second on that worker. This is not an audio callback
or a product latency qualification.

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
selections instead of replacing whole parameters. Remaining catalog preparation/lifetime work
belongs with the actor closure below.

A source that retires before its first native effect cancels the whole request. Once a first effect
succeeds, the worker finishes that admitted batch even if its command node disappears, so deleting
a command cannot discard an already-started note's note-off. Explicit module disable/removal or
transport cancellation still cancels the remaining suffix without replaying an in-flight send.

## Remaining C03 acceptance

MIDI still needs input graph/edit and callback completion ownership, canonical input payload
lifetimes, feedback occurrence accounting, prepared catalog lifetime ownership, and passive
module scheduling with clock/MTC/recovery deadlines. Output deadlines are already worker-owned.
The existing 240 Hz module rule is still present; native worker ownership does not qualify it.
Keep those concrete gaps inside C03, preserving all commands and app-owned script surfaces.
General snapshot scaling and product-size throughput remain their owning plan tasks.

Tests use fake drivers and controlled setup/send/query/destructor gates, without connecting
physical endpoints. Module, codec and command preservation tests exercise the authored product
surfaces. The [status ledger](../progress/implementation-status.md) owns current evidence and
package completion; fake-provider tests do not qualify hardware or other platforms.

# MIDI native ownership

MIDI is app-owned under `apps/chataigne/src/module/modules/protocol/midi/midi_module`.
`midi_runtime` owns native Midir discovery, input/output creation, port resolution, sends,
presence checks and final cleanup. Midir and Midly remain the device and codec implementations.
Generic admission, bounded mailboxes, cancellation and lifetime reservation come from `golden_io`.

The module owns authored configuration, graph values, CC modes, clock/MTC state, commands,
script callbacks and feedback. Its focused `incoming`, `structure`, `timing`, `outgoing`,
`events` and `transport` modules keep those responsibilities separate from native ownership.
`helpers` holds shared value-tree policy and `script_values` holds script conversion.
This boundary closes native actor blocking; it does not complete C03's remaining MIDI actor work.

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
| Output commands                 | 256 packets / 2 MiB, plus one active native send                                                                        |
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

Output admission copies a bounded packet and description. Success means queued, not transmitted.
The native worker attempts it once and returns a success or error. Only successful native replies
produce outgoing traffic and logs. A failed send has an unconfirmed native outcome and is never
replayed. Admission closes immediately; each already-admitted suffix packet gets an explicit
not-sent outcome. Full reply storage backpressures native service instead of growing another queue.

## Lifetime and recovery

Spawning a worker does not report a successful connection. Connected state requires native
readiness for every selected direction. Setup failure, worker exit and native device loss have
explicit diagnostics. Readiness clears before failure publication or blocked cleanup, with a
worker notification independent of reply capacity; an unwind guard also clears readiness.

Ordinary output reconfiguration closes old admission, drains its accepted sends and receipts,
and waits for producer disconnection before opening a replacement. A retiring handle does not
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

## Remaining C03 acceptance

MIDI still needs bounded command/script/SysEx preparation, delayed-packet admission and service,
input graph/edit and callback completion ownership, feedback occurrence accounting, prepared
catalog lifetime ownership, and passive scheduling with clock/MTC/delayed-output deadlines.
The existing 240 Hz module rule is still present; native worker ownership does not qualify it.
Keep those concrete gaps inside C03, preserving all commands and app-owned script surfaces.
General snapshot scaling and product-size throughput remain their owning plan tasks.

Tests use fake drivers and controlled setup/send/query/destructor gates, without connecting
physical endpoints. Module, codec and command preservation tests exercise the authored product
surfaces. The [status ledger](../progress/implementation-status.md) owns current evidence and
package completion; fake-provider tests do not qualify hardware or other platforms.

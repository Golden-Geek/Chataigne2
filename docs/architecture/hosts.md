# Desktop Vs Browser Host

## Desktop Host

The default desktop host lives in `golden_core`.

- `crates/golden_core/hosts/desktop/src/lib.rs` is the public desktop-host entry point used through the `golden_core::app` facade.
- `crates/golden_core/hosts/desktop/src/desktop.rs` launches the built-in UI server and the Tauri window.
- By default the desktop runtime points Tauri at the built-in Rust host and serves the shipped frontend bundle from `/`; no separate Vite server is required.
- On Windows, debug builds keep their console output for `cargo run`, while release-style builds are windowed by default and `--show-output` opt-in attaches or creates a console when logs are needed.
- `--dev` swaps the desktop host over to the frontend dev server from `apps/chataigne/ui` so
  `cargo run -- --dev` uses the live Svelte/Vite stack instead of the bundled UI assets.
- `--no-frontend` keeps the Tauri window but disables bundled UI serving, which is useful when you want to start the external frontend yourself.
- `crates/golden_core/hosts/desktop/src/desktop_commands.rs` owns the Tauri window commands and native file-dialog commands used by the UI.
- `apps/chataigne/capabilities/default.json` controls the current Tauri remote permissions.
- Native file dialogs are desktop-only behavior and should remain outside pure engine or persistence layers.
- Apps may override CLI parsing or bootstrap by calling lower-level `golden_core::app` APIs, but they should not need app-shell host files by default.

The `golden_core` facade is the ready-to-launch full-host surface. It intentionally includes the
engine, persistence, HTTP/WebSocket transport, Tauri host, native dialogs, protocol, and scripting
runtime. Consumers that do not need that complete stack should depend on the focused package that
owns their required surface:

- `golden_engine` for host-independent node execution and application transactions.
- `golden_protocol` for wire DTOs without the engine, QuickJS, transport, or desktop host.
- `golden_script` for the QuickJS VM and explicit script-host traits without the engine or desktop.
- `golden_persistence` for project documents/codecs and file transactions without host dialogs.
- `golden_audio` for the independent audio engine and native audio adapters; it does not depend on
  Golden Core or Chataigne.

These are package boundaries, not feature aliases on the full facade. This keeps dependency intent
visible in each consumer manifest. Crate-external compile fixtures cover the engine transaction,
protocol, persistence, script runtime, transport-only headless host, and full default-host APIs.

Within `golden_engine::script`, `mod.rs` owns the script node's configuration, lifecycle, and
project adapter; `template.rs` owns template discovery and include expansion; `host.rs` adapts
engine events, snapshots, and effects to the public `golden_script` host traits. VM execution and
its safety limits remain in `golden_script`, not the engine adapter.

## Browser And Headless Host

The default built-in browser/headless path also starts from `golden_core`.

- `crates/golden_core/hosts/transport/src/lib.rs` is the public transport-host entry point used through the `golden_core::app` facade.
- `crates/golden_core/hosts/transport/src/ui_server/mod.rs` exposes the current HTTP and WebSocket runtime endpoints and serves any bundled frontend assets provided by the app shell.
- `--headless` runs the server without launching the Tauri window.
- Browser-triggered `Load From...` project imports are handled by the transport host, which currently stores uploaded project JSON files under `~/Documents/Chataigne` before loading them into the live engine.
- Browser-side `Open Remote` and `Save As` remain intentionally unwired until the browser file chooser workflow is designed.
- Apps can still supply custom bootstrap if they need it, but the reusable default transport server lives in `golden_core`.

## I/O Capacity And Overflow

The reusable I/O layer owns admission mechanics; each adapter supplies limits that match its
payloads. Ordered commands are never coalesced or silently discarded.

| Boundary                    |                        Item limit |                                     Retained-weight limit | Overflow policy                                                                                                              |                                                           Service turn |
| --------------------------- | --------------------------------: | --------------------------------------------------------: | ---------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------: |
| Generic pending events      |                             4,096 |                                       4 MiB caller weight | Return `Full` with ownership; count rejection                                                                                |                                                      Consumer supplied |
| Generic worker commands     |                               256 |                                       Fixed-size commands | Return `TrySendError::Full`                                                                                                  |                                                        Worker supplied |
| Authoritative control actor |                             1,024 |                              One typed operation per item | Return `ControlErrorKind::Overloaded`                                                                                        |                                   One operation, then recheck shutdown |
| Generation compiler         |           1 pending + 1 in flight |                     One immutable snapshot per generation | Replace pending; mark stale in flight; bounded completion queue                                                              |                                                         One generation |
| Lifecycle retirement        |                 Caller configured |                        One owned resource per active task | Reject before relinquishing ownership; return the resource                                                                   |                         One dedicated cleanup thread per admitted slot |
| UI TCP/HTTP connections     |                          16 total |                   16 MiB request including 32 KiB headers | Return HTTP 503 above connection capacity; reject oversized requests                                                         |                                    One request; 3 s read/write timeout |
| WebSocket hub commands      |                                64 |                1 MiB inbound frame; 256 intents per batch | Return a request-correlated overload error                                                                                   |                                              32 commands, then publish |
| WebSocket subscriptions     | 16 clients; 32 subscriptions each |                       Scope and interest per subscription | Close excess clients; reject excess subscription ids                                                                         |                                          Included in each publish turn |
| UI snapshot encoding        |               1 active + 8 queued |         One immutable capture each; 64 MiB encoded result | Return a request-correlated HTTP/WS overload error                                                                           | One capture; same-version whole-graph results reuse one cached payload |
| WebSocket client output     |                     64 per client | 4 MiB ordinary serialized weight plus one 64 MiB snapshot | Coalesce latest-value planes; replace overloaded subscription deltas with resync; disconnect if reliable recovery cannot fit |                                      Drained by the client socket loop |
| OSC output commands         |                               256 |                                  One UDP message per item | Reject synchronously with an overload error                                                                                  |                                                           256 commands |
| OSC input events            |                             2,048 |                             2 MiB decoded datagram weight | Reject new datagram event; count rejection                                                                                   |                                           256 datagrams / 1,024 events |
| Serial input events         |                             2,048 |                                      2 MiB received bytes | Reject new read event; count rejection                                                                                       |                                                           1,024 events |

Readiness is set only after successful admission and remains armed after a partial drain. Dynamic
payload adapters must call the weighted send API; fixed-size status events may use unit weight.
Compiler implementations receive a cooperative staleness token and must check it between expensive
materialization stages. Superseded pending snapshots are returned to the engine immediately for
retirement; stale in-flight results are reported but can never become the live generation.
Engine compiler requests capture a fixed set of copy-on-write parameter-value shard roots plus the
immutable scheduled-node root built by graph resolution. Dense parameter ordering, value cloning,
scheduled-kernel validation, and generation construction run on the compiler worker. A successful
request retains that one materialized layout for constant-time reuse during generation installation.
The transport host admits TCP connections before spawning request workers, so reconnect storms have
a fixed task ceiling. Its WebSocket hub never drains an unbounded producer backlog before
publication, and a `Received` control phase is emitted only after the corresponding command has
entered the hub mailbox. When a subscription delta overflows outbound capacity, the hub first
replaces that subscription's queued deltas with an explicit resync marker and pauses it until
recovery. Other subscriptions and control messages remain reliable. If that replacement cannot
fit, or reliable non-delta/snapshot delivery exhausts its capacity, the client is disconnected so
reconnect/snapshot can restore a coherent view. Only explicitly latest-wins values may be coalesced.
Snapshot requests clone only the fixed projection roots on the caller. One reusable transport
worker performs DTO materialization and JSON encoding, while the WebSocket hub continues accepting
control and replay work. Its queue and encoded size are explicit, and only the newest completed
whole-graph version remains cached.
`golden_io::RetirementPool` reserves capacity before ownership moves. Cleanup threads are therefore
limited even if native shutdown never returns, and admission failure gives the caller its original
resource back. Production project replacement permits two concurrent detached-engine retirements;
a third request is rejected before allocating a generation or preparing the candidate.

`golden_io::BoundedWorker` composes weighted command/reply channels with out-of-band cancellation.
The worker reserves a lifetime slot before starting and owns it until its resources finish dropping.
Cancelling or dropping the handle wakes command, retry, and reply-capacity waits without a join or
another cleanup thread. Completed replies wait for capacity on the worker; oversized replies return
ownership immediately. Worker exit remains visible to event-driven owners even without a reply.
The supplied pool therefore bounds active and retiring workers together. This differs from reserving
a retirement slot only after a live resource has already been removed from its owner.
`finish_admission` closes the producer while preserving admitted commands and replies; its owner
keeps draining until completion. A failed worker can instead close receiver admission and resolve
every already-accepted command before exiting. Neither path needs an extra stop command or a join.

`WorkerContext::drain_commands` exposes a bounded nonblocking turn for workers that also receive
unsolicited native input. Graceful closure drains accepted commands before disconnection;
explicit cancellation ends service without consuming the remaining queue.

`BoundedWorker::spawn_notified` publishes readiness after a reply enters its mailbox and after
the final sender exits, including an unexpected worker failure. `WorkerNotifier` carries only a
short, nonblocking wake callback; it never carries payloads or graph operations. Notifications run
outside the mailbox lock. `DelayedNotifier` shares one host timer with 1024 weak registrations,
coalesces each source's earliest deadline, and supports cancellation. Sound Card and UDP supply
their own engine-node wake adapters; Golden IO imports no app or engine-node types.

`PayloadPool` owns one background disposer and reserves item/retained-byte capacity before sharing
a prepared value. Its retained handle gives consumers read-only access while the pool keeps the
final strong owner. Destruction happens outside the pool lock, capacity stays reserved through
blocked cleanup, and dropping the host handle does not join. Destructor panic is contained so
later cleanup can continue. Consumers must not grow interior storage beyond the declared weight.
App stream data uses one shared pool capped at 256 payloads / 256 MiB. Message vectors and strings
are therefore finally disposed outside node callbacks, including after producer failure or node
removal. Graph-owned value copies and engine-owned event/replay payloads have separate lifetimes;
these admission bounds are not a whole-process RSS or event-history budget.

The app-owned UDP adapter owns socket generations in `stream/udp/lifecycle.rs`, using one active
or retiring generation per module and 16 global native lifetime slots. Bind/DNS/setup, packet
sends, parsing, script-argument preparation, and bounded log formatting stay on the worker.
Command admission is bounded by 256 items / 1 MiB; reply admission uses
64 items / 32 MiB, charging prepared values and callback allocations. Each native turn services at most 16 commands
and 16 incoming packets, and an owner drain extracts at most four events. Oversized UDP payloads
and host names fail admission synchronously. These bounds cover retained transport allocations;
shared stream preparation and graph application use the separate bounds below. A maximum raw
datagram and its callbacks fit the reply budget. A node drains one batch of at most four events
and publishes one prepared callback at a time with its own completion receipt. Failed publication
retries only an unpublished event, preserving FIFO and multiplicity; it never rebuilds byte arrays.

UDP nodes are passive. Reply/exit notifications and shared retry deadlines drive service without
scheduled updates or an idle compiled kernel. Settings are copied on the worker input boundary;
processing/grammar generations reject incomplete old frames, while logging-only changes preserve
the parser generation. Prepared results cannot be reinterpreted by a later UI setting. Processing
changes update the live socket without rebinding. Interface discovery runs on the worker at most
once per second, suppresses unchanged results, and retains existing choices if 256 entries / 64 KiB
would be exceeded. Destruction clears queued runtime input; undo creates a fresh input generation.

The app's `common/received_values` owns the shared HTTP/stream message model, retained-weight
accounting, and graph-application cursor. `common/streaming/admission.rs` owns immutable prepared
stream batches, their cached retained weight, and shared preparation/queue limits. Node queue
admission reads that metadata without walking or copying all values. It retains complete parse
batches in FIFO order, bounded by 65,536 messages / 16 MiB, including active batches waiting for
engine receipts. Rejected batches produce backend diagnostics.
One maximum IPv4 UDP datagram in raw mode fits this admission budget. Each service turn uses the
same 256 path/value/sibling work-unit and roughly 64 KiB copy targets as HTTP, with one admitted
larger scalar allowed a whole turn. Compatible existing values compare against the planned ordered
prefix, preserving sequences such as `0 → 1 → 0`; conflicting structural changes need a fresh
snapshot. Failed or discarded graph application abandons the batch suffix and reports failure
without replaying applied values or triggers. Later admitted batches can continue.

Raw preparation rejects input above 65,536 byte messages before expansion. JSON input is bounded
to 1 MiB before decoding, with a structural work/depth check before value conversion and path
expansion. Prepared paths support 64 levels / 4096 UTF-8 bytes; provenance is capped at 4096 bytes
without splitting UTF-8. Incremental preparation charges spare vector capacity, owned strings,
and payload storage, stopping at 16 MiB. A list's value bound derives from that storage budget,
so a valid 100,000-value list still uses the graph cursor instead of a smaller array-size limit.
Failed raw/JSON preparation rejects the whole frame before publication. Temporary JSON DOM and
conversion scratch allocations are additional to the retained-output budget; the input, nesting,
work, and value limits bound them separately. The 16 MiB number is not a total process-RSS claim.

Stream line parsing retains at most 1 MiB, scans incoming chunks without shifting their remaining
suffix, and rejects each oversized line once while discarding through its delimiter. Valid
surrounding frames survive. A changed grammar rejects a partial old line before interpreting new
input. Per-line preparation also enforces expansion/path admission and preserves other valid
frames in the same input turn. Line rejection is logged at the app-owned stream boundary.
Other stream modules still prepare input in node callbacks and retain their existing periodic
service. Their worker ownership, channel admission, and retirement remain open P03 work; UDP's
qualification does not qualify those paths or the wider module/generator audit.

Reconfiguration and disable close send admission while retaining accepted work and results.
The next generation waits for the old socket's release, including when rebinding the same port.
Only destination destruction cancels pending work. Bind failures explicitly fail already-admitted
sends, and worker/native failures use capped 250 ms-to-5 s reconnect backoff. An asynchronous Ready
event controls connected state. A disabled consumer retains its bounded completion backlog until
re-enabled or destroyed. Cancellation never joins setup, DNS, or cleanup; an uninterruptible
native call retains its global lifetime slot until it returns.

The app-owned HTTP adapter uses that boundary with 16 global worker lifetimes, 256 commands / 8 MiB
of retained request weight, and four replies / 32 MiB of retained event weight per worker. Bodies are
read up to a 1 MiB limit even without a declared content length. JSON expansion has value, depth,
and retained-weight limits; request JSON, headers, form fields, and file descriptors are bounded
before admission. Multipart file contents stream on the worker. Weight includes owned buffers,
parsed values, and prepared script arguments; it is an admission estimate, not a process-RSS limit.
One in-flight request/result may additionally be retained, and parsing/preparing it can temporarily
exceed the reply queue's retained-weight budget before rejection. Client creation, JSON parsing,
success/failure script-argument preparation, retry waits, requests, and client destruction stay on the worker.

HTTP module turns drain at most four events and retain at most that one FIFO batch. The shared
received-value cursor services at most 256 path/value/sibling work units and about 64 KiB of value/path/provenance
copies per turn. One larger scalar may consume a whole turn rather than being truncated or stranded;
it was already bounded at the worker boundary. Large arrays use the same continuation, not a smaller
input limit. Known missing paths become detached subtree batches; list tails are pruned with stable
next-sibling markers and bounded scans. Interleaved marker removal re-arms that scan. Surviving nodes
keep their identities and unindexed children remain untouched.

Success and failure callbacks stay in FIFO order, with at most one completion per turn. A response
waits for engine completion receipts before advancing to the next graph-planning turn. Rejection,
discard, or rollback reports `requestFailed` with the original request identity and attempt count;
an applied prefix may remain. It never reissues the endpoint request or replays earlier pulses.
Snapshot freshness alone cannot acknowledge a queued edit. Missing repeated trigger addresses and
ancestor/descendant replacements yield between conflicting units instead of collapsing into one
planned leaf. Creating a trigger address retains the first pulse until a fresh snapshot can
submit it as an Append value edit; existing trigger addresses may receive multiple appended
pulses in one turn. Only a bounded value prefix is cloned for planning. Indexed list elements retain
their existing description policy rather than duplicating request provenance. HTTP value paths
are limited to 256 bytes per segment / 4096 total bytes, and informational provenance is capped at
4096 UTF-8-safe bytes. Actor-side URL diagnostics also use a bounded prefix. These are backend
preparation/service contracts; no UI supplies graph mutation policy.

The HTTP owner retains a prepared callback event until its own publication receipt completes.
A discarded or rolled-back event can be resubmitted; a published event is never replayed because
a later request or subscriber failed. Retries share the event's immutable JSON payload instead
of copying the response body, and the app callback wrapper moves worker-prepared arguments into
that payload without reserializing them. Receipt waits, final acknowledgements, and callback-only
turns do not request whole-tree snapshots. Subscriber execution is a separate outcome from event
publication. The same four-event FIFO bound includes the callback waiting for acknowledgement.

Idle modules use pending/disconnected signals; failed startup/worker recovery uses capped backoff.
HTTP configuration handoff lives in the app's `http/lifecycle.rs`. Each module retains at most
three generations, including their bounded reply queues; all modules share the 16 native lifetime
slots. New generations wait for earlier accepted requests to resolve before dispatching their
own requests. Results drain from the oldest generation first, under the same four-event aggregate
service budget. Identical prepared configurations reuse admission. Full local/global capacity
blocks new admission and retains old owners while backoff waits for progress; no per-change cleanup
thread is created.

Request completion is distinct from native client destruction. A worker publishes completion
before cleanup, and the consumer observes that marker before its final queue drain, avoiding a
race with the last result. New requests/results can progress while old native cleanup retains its
reserved slot. Dropping a waiting generation wakes its predecessor wait. Request-worker panics
close admission and report failure for both the started request and unsent queued requests;
attempt counts and diagnostics distinguish possible remote effects from requests never sent.

Invalid settings and disabling close admission gracefully. Previously admitted commands retain
their owners; disabling defers callback consumption until re-enable, with ordinary reply
backpressure bounding the paused producer. Destruction cancels remaining delivery because the
destination itself is being removed. An already-issued blocking HTTP call may complete externally
and retain its lifetime slot until its configured timeout; cancellation does not imply remote
effects were rolled back. Full P03 module/lifecycle qualification remains in the implementation ledger.

Golden runtime activation builds the parameter-control index before the first timed tick. If an
active scheduled node needs a process-tree snapshot, activation also prepares one immutable,
one-use snapshot. The first tick reuses it only when no edits or inbox work changed the tree before
scheduled callbacks; otherwise the engine discards it and builds a fresh snapshot. This moves
initial indexing and snapshot construction out of the tick budget, but activation still performs
that synchronous work. It is not a claim that large-project startup or the first scheduled
callback meets a real-time deadline.
An edit before the first scheduled callback rejects the prepared snapshot and can still make that
first tick expensive; large-project edit and retirement behavior remains a separate qualification.

The engine's worker ingress uses reserved `NodeWaker` notifications; payloads and their byte
budgets stay with module-owned mailboxes. The Sound Card module consumes that wake path through
its passive worker-service hook, with configuration-dependent snapshot requests. It no
longer sends private wake events through an unbounded arbitrary-edit channel. One process-wide
host timer admits at most 1024 active/retiring Sound Card registrations. Each retains one earliest
deadline, so repeated retry scheduling does not accumulate jobs. Dropping the source cancels its
deadline; a full registration set rejects before worker startup. A later registration attempt can
recover after an owner releases its slot. A paired condition-variable lock wakes the sleeping timer
on deadline publication.
There is no full-queue fallback that polls the node each engine tick, and no per-source timer
thread. Startup failure does not permanently poison timer construction. This qualifies wake and
retry scheduling only; the Sound Card worker, event-drain, and native-retirement audit remains open.

## Current qualification boundaries

The table describes reusable primitives and the named migrated adapters, not every app module.
The October audit found blocking module stop/join paths, unbounded/unit-weight payload admission,
unrestricted engine external-edit drains, and lifecycle expansion loops. The arbitrary engine
channel has since been removed in favor of bounded worker notifications; the remaining module
and lifecycle gaps are still open. The single
[implementation plan](../plan/Chataigne2_Codex_Implementation_Plan.md) owns those P02/P03 repairs;
do not infer that all module I/O is already bounded from the generic channel API.

The HTTP/WebSocket UI host also lacks session/origin admission for privileged project/script
operations at the audited revision. Loopback defaults and Tauri capabilities are separate from
request trust. P05 defines a password-free LAN-compatible repair; the
[security reference](../reference/tauri-security.md) and [current status](../progress/implementation-status.md)
record the boundary and evidence.

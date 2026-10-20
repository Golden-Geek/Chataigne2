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
`command_admission_closed` observes producer closure even while accepted commands remain queued.
Duplex loops can stop admitting native input or reconnect/discovery work during graceful retirement
without discarding that queued command prefix.
`WorkerContext::try_publish_weighted` attempts one reply admission without waiting and returns
the original event on overload or disconnection. A duplex adapter can retain a bounded reply
continuation and keep servicing independent native work. `publish_weighted` remains the ordered,
cancellable capacity wait for sequential workers and final retirement. `event_metrics` exposes
the existing queue's item/weight/age/rejection diagnostics without consuming replies.

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
`try_reserve` admits capacity before a consumer materializes data. An unused reservation releases
its item/bytes on drop and keeps the disposer alive if the original pool handle disappears.
Sealing transfers ownership without copying, immediately releases unused bytes, and keeps used
capacity charged through final destruction. Oversized sealing returns the original producer data.
`try_retain_arc` also protects existing immutable Arcs and deduplicates their reservation within
one canonical pool. Ordinary Arc releases cannot notify it; while such entries exist, only the
disposer checks final ownership every 100 ms. An atomic unwrap claims final ownership so a
concurrent weak-reference upgrade cannot move destruction onto a consumer. Capacity stays
reserved through failed claims and cleanup.
App stream data uses one shared pool capped at 256 payloads / 256 MiB. Message vectors and strings
are therefore finally disposed outside node callbacks, including after producer failure or node
removal. Graph-owned value copies and engine-owned event/replay payloads have separate lifetimes;
these admission bounds are not a whole-process RSS or event-history budget.

`golden_script::ScriptMethodCall`, `ScriptPropertyWrite` and `ScriptCustomEvent` are runtime-only
ingress contracts shared by QuickJS, host bridges and engine edits. Method arguments, property
values, event payloads and staged logs share one canonical pool: **16384 allocations / 64 MiB**.
Methods admit 4096 arguments; each method/property/event admits at most 4 MiB retained storage,
including spare vector/string/reference capacity. Logs admit 64 KiB and names/topics 1024 bytes.
Rust producers prepare before enqueueing and recover rejected allocations. Event object containers
are rebuilt during preparation to exclude hidden spare map capacity without copying their strings.
Missing targets, unsupported operations, callback failure and unvisited edit/host-op suffixes
release shallow handles; the disposer keeps final ownership. `ProcessCtx::capture_script_args`
forwards the same argument lease. Property setters borrow the original value and validate before
creating an accepted domain-owned copy.

Custom-event routing and replay share a protected raw Arc, retaining its original lifetime charge
until every consumer releases it. Never register that Arc in an independent pool. Only the disposer
polls external owners, at most once per 100ms while idle. Small-event item headroom accommodates
the normal replay window and its deferred prefix compaction; byte exhaustion still rejects new
inputs explicitly until owners release capacity. These limits do not bound the entire process.

`PayloadReservation::share_retained` supplies a `SharedPayload` that can forward both raw Arc
routing and its original lease. The script bridge uses `CustomEvent::from_shared_payload`;
command capture calls `try_lease_payload` to reuse that canonical owner. Plain event JSON uses
one engine fallback pool capped at 16384 allocations / 64 MiB. Lifetime metadata is excluded
from serialization and logical event equality. Do not erase the owner before asynchronous
capture or register the same raw allocation in an independent pool.

QuickJS method, property and event JSON is limited to 1 MiB, 4097 aggregate nodes including the
root, and depth 32 before expanding a wide/deep tree. Malformed JSON fails explicitly. Disposal
capacity is reserved before typed conversion; method conversion also seals partial vectors on
failure. Unused capacity shrinks to measured retained bytes. Each callback queues at most 4096
host operations even when its configured host-call budget is larger. Callback failure discards
staged operations under the existing quarantine/reload policy; a host commit failure preserves
the already accepted prefix and discards its unvisited suffix.

JSON scratch parsing, VM string conversion and bounded-name cleanup remain on the invocation
thread. Accepted domain values, logger records and UI projections may have separate copied storage.
The input lifetime contract does not qualify whole-process RSS, replay projection cost or callback
deadlines.

The app-owned UDP adapter owns socket generations in `stream/udp/lifecycle.rs`, using one active
or retiring generation per module and 16 global native lifetime slots. Bind/DNS/setup, packet
sends, parsing, script-argument preparation, and bounded log formatting stay on the worker.
Command admission is bounded by 256 items / 1 MiB; each item owns one payload and a frozen
destination-plan ticket shared with the module's revision cache. A cold command also carries
a budgeted snapshot lease and native preparation cursor in that same envelope. Admission accepts
or rejects the whole command before preparation or send; an invalid or oversized later
destination cannot send its valid prefix. Allocated payload, destination slots, ticket storage,
and hostname capacity contribute to the weight. Pending plans reserve a snapshot-derived upper
bound capped below the command limit; ready plans charge their actual retained allocation.
One native cursor
retains at most one additional command outside the mailbox, bounded by another 1 MiB. Graceful
retirement completes that cursor and every queued command in FIFO order. Cancellation abandons
the destination; network failure is reported per destination without replaying prior sends.
Reply admission uses 64 items / 32 MiB, charging prepared values and callback allocations.
The native loop uses nonblocking reply admission with one pending result per producer in
`transport/replies.rs`: input (32 MiB), output outcome (64 KiB), and interface discovery
(64 KiB plus its fixed event envelope). These are additional to mailbox and command-cursor
bounds. The three pending entries share one FIFO, cache their weights, and retain the original
events on retries. A producer pauses until its prior result enters the mailbox. Input/discovery
pressure does not gate cold preparation or successful unlogged sends; output-outcome pressure
pauses its own destination cursor after the attempted send, so reply retries never resend it.
Graceful retirement finishes all admitted sends and then drains every pending result before
producer disconnection. Cancellation abandons the pending results and unsent suffix without a join.
Oversized reply details become one explicit bounded error; they never trigger a replay. Interface
discovery bounds error text before caching as well as bounding successful choices.
Each native turn services at most 16 destinations (across command boundaries), or yields cold
preparation after 64 traversal steps, then services at most 16 incoming packets. An owner drain
extracts at most four events. Oversized UDP payloads fail synchronously; cold plan validation
errors arrive asynchronously before any output is sent. These bounds cover retained transport allocations;
shared stream preparation and graph application use the separate bounds below. A maximum raw
datagram and its callbacks fit the reply budget. A node drains one batch of at most four events
and publishes one prepared callback at a time with its own completion receipt. Failed publication
retries only an unpublished event, preserving FIFO and multiplicity; it never rebuilds byte arrays.
Outgoing log prefixes are prepared on the native worker after successful sends and use the same
bounded event drain. The app checks Golden Engine's subtree revision at each script/custom-event
send, including calls immediately after an edit before callbacks run. Unchanged destinations share
one immutable prepared plan; commands retain its original interpretation across later configuration
changes. Cached-plan replacement and mailbox rejection release only a retained handle; the shared
payload disposer owns final target-string cleanup. Disable/destroy releases the module cache.
Closed or retiring transport admission rejects before plan preparation. Generic sender/worker
admission hints observe native closure independently of reply draining and queue capacity; the
actual send remains authoritative because closure can race with preparation. UDP releases its
cache on closure/disconnection while retaining it under ordinary queue pressure.
Cold preparation uses constant-space folder links and validates borrowed hostnames before copying;
it never recurses or allocates an intermediate output-ID list. Native preparation, cancellation,
and failed-builder cleanup stay on the worker. Pending and successful tickets can be reused only
after their owning command was admitted; rejected admission never installs an ownerless ticket.
Transient preparation failures permit a later retry; invalid configurations remain rejected until
their subtree revision changes. The applied view is captured once, with no per-turn snapshot rebuild.
Golden Engine caches conservative snapshot allocation weights and leases actual shared storage
through one canonical 384-allocation / 1 GiB pool. Ordinary snapshot clones and overlays retain
the reservation until they release the shared data; final destruction stays off the actor.
These are allocation reservations, not a whole-process RSS bound. Default output creation only
adds an output when its manager is empty; existing user folder structure is preserved without a scan.

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
The TCP client uses a bounded native worker with 16 global lifetime slots, 256 commands / 1 MiB,
one additional 1 MiB write cursor, and 64 replies / 32 MiB. The owner drains at most four events.
DNS resolution, up to eight 500 ms connection attempts, capped reconnect backoff, socket health,
writes, and outgoing log preparation run on that worker. Stop cancels without joining; an
uninterruptible setup call retains its slot. Reconfiguration closes admission and retains the
old owner until accepted sends/results finish before opening a replacement endpoint.

Each native write turn submits at most 64 KiB, retaining its exact offset across partial writes,
`WouldBlock`, and interruption. A five-second deadline from the first native write attempt closes a stalled
connection. Failure reports its written prefix; no prefix or suffix is replayed after reconnect.
Admission captures the live connection generation, and the worker rejects queued commands from
an old generation even if a new socket is already connected.
Admission also compares the endpoint against the applied snapshot before moving the payload;
a send immediately after an endpoint edit is rejected while reconfiguration is pending.
Native service bounds command work to 16 steps and input to four 8 KiB reads per turn.
Input, output-outcome, and status continuations
share the app's generic stream reply FIFO, with per-source limits of 32 MiB / 64 KiB / 64 KiB.
Input pressure cannot gate successful unlogged output. Outgoing logs are bounded and follow
complete native writes. Connection status is a bounded latest-state read model.

TCP client is passive: reply/exit notifications and bounded retry deadlines drive its owner,
including explicit disabled-node completion service during retirement. Parsing, callback preparation
and grammar/connection generations live at the native boundary. The six stream adapters use passive
receipts and retained input/output ownership described above. The wider module/generator audit
remains open P03 work.

Reconfiguration and disable close send admission while retaining accepted work and results.
The next generation waits for the old socket's release, including when rebinding the same port.
Only destination destruction cancels pending work. Bind failures explicitly fail already-admitted
sends, and worker/native failures use capped 250 ms-to-5 s reconnect backoff. An asynchronous Ready
event controls connected state. Disabled consumers normally retain their bounded completion backlog
until re-enabled or destroyed; owners that opt into disabled-node completion service can finish
accepted work while remaining disabled. Cancellation never joins setup, DNS, or cleanup; an uninterruptible
native call retains its global lifetime slot until it returns.

The app-owned HTTP adapter uses that boundary with 16 global worker lifetimes, 256 commands / 8 MiB
of retained request weight, and four replies / 32 MiB of retained event weight per worker. Bodies are
read up to a 1 MiB limit even without a declared content length. JSON expansion has value, depth,
and retained-weight limits; request JSON, headers, form fields, and file descriptors are bounded
before endpoint dispatch. Multipart file contents stream on the worker. Weight includes owned buffers,
parsed values, and prepared script arguments; it is an admission estimate, not a process-RSS limit.
One in-flight request/result may additionally be retained, and parsing/preparing it can temporarily
exceed the reply queue's retained-weight budget before rejection. Client creation, JSON parsing,
success/failure script-argument preparation, retry waits, requests, and client destruction stay on the worker.

HTTP native input is a typed script-argument lease or a captured command invocation, sharing
the generation's existing FIFO and cancellation. Commands retain their immutable snapshot and
bounded override JSON (256 KiB / 4096 nodes / depth 16); only target selection is decoded on
the actor. HTTP and stream commands share this app-owned capture boundary. Command source
parameters admit 4 MiB total before conversion, including spare capacity. JSON command bodies
are checked through a non-materializing serde pass (16384 nodes / depth 64) before construction;
headers and form fields stop at 256 entries. Worker materialization rechecks overridden source
weight and converted request bounds. File contents continue to stream directly on the worker.

Queue acceptance precedes content conversion. Failed conversion reports an ordered, bounded
warning with zero attempts and sends nothing; later requests remain connected and may progress.
Applied configuration and source/target eligibility are checked before native admission. Once
accepted, a request retains that generation and its frozen input across disable/reconfigure or
source removal; explicit module destruction may cancel it. Native queue rejection releases only
leases on the actor. Snapshot, override and argument lifetimes keep their existing canonical
budgets, while materialized request copies and conversion scratch belong to the native worker.
Command capture also holds source/target wake identities: retirement fences pending admission
even if a node ID is later restored. Conversion never reapplies that source check to an already
admitted native input.

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
thread. Startup failure does not permanently poison timer construction.

Sound Card facades hold `AudioControl` and observation readers. A prestarted native owner keeps
the `AudioEngine`, receives its events and performs all blocking shutdown/join/final engine cleanup.
Sixteen global native lifetimes are reserved before engine construction and held through cleanup.
Stopping a facade requests bounded control shutdown and signals cancellation; dropping it neither
joins nor creates a cleanup thread. Producer exit is announced before cleanup can block; queued
event prefixes drain before native-failure recovery. A separate completion lifetime resolves only
after actual cleanup; the same startup worker waits for that completion before constructing
a replacement. Destroy/restore starts a fresh node lifetime with the existing recovery policy.

Eight `BoundedWorker` startup lifetimes own initialization and result publication. Their mailboxes
admit eight bounded requests and two results. One fixed 10ms coalescing window and one eight-item
drain select the latest admitted request without a producer extending the debounce forever.
Startup-result service drains at most two per actor turn. Queue overload retains the worker;
disconnection and native failure use the module's existing capped retry deadlines. Error strings
are limited to 4096 UTF-8-safe bytes and driver identifiers to 128 bytes at startup admission.

Sound Card's event retained-byte accounting, budgeted drain, reliable occurrence delivery, remaining
admission recovery and configuration-preparation bounds remain P03 work. The underlying audio
event stream currently uses `try_send` with a dropped-event counter; wake/native-ownership evidence
does not qualify that overload policy, whole-process memory or actor deadlines.

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

# Timed generators

Signals and Metronomes belong to `apps/chataigne/src/module/modules/generators`.
Their modules own authored configuration, generated value nodes, warnings and script
surfaces. Their workers own elapsed-time accounting, waveform/tempo state and output
preparation. `golden_io` owns the reusable bounded queues, deadline waits, notifications,
cancellation and lifetime permits; it contains no generator policy.

## Admission and service

The app policy lives in `module/common/generators.rs`. These are explicit overload limits,
not a claim that every maximum-size configuration meets its requested sampling rate.

| Resource                   | Bound                                                           |
| -------------------------- | --------------------------------------------------------------- |
| Items per module           | 4096                                                            |
| Label/declaration text     | 1024 bytes each                                                 |
| Aggregate curve structure  | 16384 keys; source preflight also counts non-key curve children |
| One configuration          | 32 MiB retained weight                                          |
| Queued commands            | 16 items and 32 MiB retained weight                             |
| Queued publications        | 8 batches and 1 MiB retained weight                             |
| Publication/actor drain    | At most 64 rows in one batch per wake callback                  |
| Live or cancelling workers | 64 across both generator families                               |

Borrowed snapshot metadata is checked before configuration construction, including curve
script bytes before cloning. Runtime admission repeats the checks and includes retained
vector/string capacity. Curve weight conservatively charges 1024 bytes per key for public
keys and fixed-size compiled segment storage, plus twice each script string's capacity.
The worker retains one active configuration, one in-flight command and one bounded computed
event in addition to its bounded mailboxes. Configuration replacement and final worker state
disposal occur on the worker.

Queue pressure rejects new commands explicitly. Configuration is desired state: the module
keeps the current worker and retries the latest authored state after 250 ms. A full queue never
causes a replacement worker or loss of its accepted command prefix. Reset and manual-tick
script calls return overload/disconnection errors to their caller. Invalid authored input stops
processing and publishes a node warning; a valid edit can start it again.

## Time and occurrences

Each ordinary command settles elapsed time using the previous configuration before changing
state. Frequency/tempo edits preserve phase or time since the previous tick. Metronome catch-up
per item is limited to 128 ticks per worker turn; a pending command waits for the old tempo's
complete due prefix across turns. Cancellation interrupts this process.

The worker splits publications into bounded batches and waits for mailbox capacity with
cancellation support. Backpressure slows sampling instead of silently losing cycle/tick counts.
The next service accounts for the elapsed interval. Each accepted manual tick produces its own
ordered publication; natural tick/cycle callbacks preserve their existing aggregate count fields.
Metronome parameter triggers retain their existing coalescing behavior. The count-bearing script
callback, rather than repeated parameter triggers, reports the natural catch-up count.

## Lifetime and recovery

Both modules are passive engine nodes. Worker publications and final disconnection notify the
reserved node lifetime; pending batches re-arm it. Configuration edits request a wake, and retry
uses the shared deadline notifier. There is no periodic engine-side generator polling.

A worker reserves a lifetime permit before thread creation and retains it through final cleanup.
Stop, disable, destroy and owner drop only cancel; they never join or spawn a retirement helper.
A blocked worker keeps its permit, so repeated replacement cannot bypass the lifetime bound.
Published replies drain before an unexpected exit is handled once, warned and retried after
250 ms. Removal closes the old node wake source; undo registers a fresh source and worker,
without consuming the retired worker's mailbox.

Deterministic clock fixtures, bounded-admission/backpressure fixtures and engine lifecycle tests
live in each generator's adjacent `tests/` directories. They use no hardware or desktop hooks.
Product-scale timing remains the separate P16 qualification gate.

# System module ownership

Chataigne owns OS nodes and commands under `apps/chataigne/src/module/modules/system/os`.
The module projects authored state and observations; its `os_runtime/` worker owns metrics,
command preparation, DNS/UDP, process creation and native cleanup. `golden_io` provides the
public worker, retained-payload, lifetime and notification contracts. OS policy stays in the app layer.

App Control has its own watched-application/folder model under the adjacent `app_control/` tree.
Its native runtime owns process/window operations and recursive folder observations. The module
owns authored watch configuration, detached value trees, command catalogs and guarded publication.
The [closure audit](../progress/p03-closure-audit.md) records remaining C06 qualification.

## Capture and dispatch

Construction and declaration-only materialization perform no native discovery or worker startup.
Engine-owned lifecycle callbacks register a passive wake; its service starts the native provider.
Static build information is available immediately. Native host architecture/version and metrics
arrive from the worker. Sysinfo refreshes the current process, rather than constructing a complete
host process inventory. CPU/network inventory bounds are 4096 processors / 256 interfaces.

Command nodes retain their canonical snapshot and execute overrides. Scripts share Core-owned
argument arrays. Bounded, inbox-only attachments put both forms through the same stabilization
boundary, ordered by their original engine timestamps. There is no JSON command relay or full
snapshot overlay. Native preparation borrows consumed fields and honors the last matching override.

Each module admits 64 source lifetimes across capture, the internal inbox, native work, completed
outcomes and queued publication. Its permits follow the retained source, including disposal.
The native mailbox admits 64 records / 4 MiB. Global captures and outcomes share 512 records /
16 MiB; each record charges 24 KiB, with an additional three times execute-override storage for
capture scratch. Metrics have a separate 512-record / 16 MiB pool, charged at 24 KiB per record.
Core snapshot, argument, event and callback allocations retain their independent canonical quotas.
These are application ownership bounds, not RSS claims.

Scripts accept at most three arguments / 64 KiB of retained input. Preparation bounds MAC text
at 64 bytes, host text at 253 bytes and numeric text at 32 bytes before copying/parsing. Native
architecture/version fields are bounded at 4096 bytes each; diagnostics are capped at 1024 bytes.

## Native lifetime and outcomes

Sixteen global slots include opening, active and retiring providers. Stop invalidates the actor
fence and cancels without joining. A second native fence and stopped notification retire before
the provider's destructor, even on panic. Blocked setup, native calls or cleanup retain their
original slot and payload ownership. They cannot cause a replacement-thread cascade.

Started host-control processes stay owned through exit and reaping on the worker. A started
process with an error or uncertain result does not trigger another candidate command. Local
process completion and UDP submission do not acknowledge a host shutdown or a remote wake.
Commands are never automatically replayed. Unstarted cancellation reports **not sent**;
interrupted execution reports an **unconfirmed** outcome.

Command failures preserve the metrics provider. Provider/setup failures use an absolute recovery
deadline with 250 ms to 8 s backoff, reset after a healthy sample. Failed disposer startup remains
retryable; successful initialization keeps one shared quota owner. A full native mailbox retains the same captured source behind
a separate absolute 250 ms deadline; unrelated wakes do not retry it early. Refused admission
disarms the unpublished receipt and cannot produce a cancellation/readiness feedback loop.
Metrics pressure withholds a latest-state sample and reports a warning without reconnecting.

The native worker services at most eight commands before checking its one-second metrics deadline.
An uninterruptible native call holds that provider and can delay its next sample.
The actor services at most eight captured inputs, sixteen completed outcomes, four statuses and
one latest metrics record per wake. It has no periodic polling callback.

## Publication

Native preparation builds canonical callbacks. Callback-admission failure remains separate from
the known native result and never makes an already performed action retryable. Completed records
remain owned when an actor wake lacks the snapshot needed for publication.

Metrics publication fences both runtime lifetimes and the module/value target identities before
any values or constraints change. The original quota follows the queued edit batch through
application or rejection. Completed command callbacks describe past effects: their destination
uses the current module identity guard, rather than requiring that the completed native session
still exist. Removing a command before native dispatch prevents execution; removing it afterward
does not erase a known physical outcome.

Tests use controlled providers. Test builds disable the native host-control executor, so an
incorrect fixture cannot perform shutdown, reboot, logout or Wake-on-LAN. Physical control and
platform qualification remain separate; the [execution ledger](../progress/implementation-status.md)
owns acceptance evidence and the remaining App Control work.

## App Control

`app_control/app_control_runtime/native/` owns sysinfo refresh, directory traversal, process
creation/termination, window enumeration/actions and child reaping. Construction is inert;
engine readiness registers a passive wake. The native worker samples active watches every 500 ms,
servicing at most eight commands between sample checks. An empty configuration receives one
initial observation and then remains idle. Native actions are disabled in test builds.

Configuration admits 64 combined watch targets, with labels up to 128 bytes and paths up to
4096 bytes. Derived filename labels truncate at a UTF-8 boundary. Value folders are authored as
detached trees. A later notified turn binds their materialized identities. Command catalogs walk
at most 64 DFS steps per turn, prioritizing the command tester; they do not scan every descendant
in one callback. Superseded catalog mutations skip themselves and current jobs update live scalar
choices/selection in one drain, preserving identity and a later authored selection. The actor
services at most eight captured inputs, sixteen completed outcomes, four native events and eight
watch rows per turn.

Sixteen global native slots include blocked opening and retiring providers. Stop cancels without
joining; the native fence and stopped notification retire before provider cleanup. Launched child
handles remain owned and are reaped on the native worker. Cleanup can wait for surviving children;
it retains that provider's original slot and never kills an application merely because its module
was removed. Process/CPU inventories reject above 16384/4096 entries. Window enumeration rejects
above 4096 callbacks; an action batch rejects above 64 processes or windows before acting.

Folder collection admits 1024 entries / 512 KiB of retained entry data across all watch roots,
4096 bytes per relative path and depth 32. Directory symlinks are not followed. Collection stages
all baselines and commits them only after every root succeeds. Report backpressure retains a
prepared report and withholds further sampling, preserving accepted change order.

Canonical command snapshots and execute overrides, shared script arguments, and authored Running
events use the same typed local inbox boundary before dispatch. Each module admits 64 source
lifetimes through native execution, outcome publication and disposal. The native command mailbox
admits 64 records / 4 MiB. Global source/outcome storage admits 512 records / 32 MiB, charging
64 KiB plus three times execute-override storage per source; configuration and reports have
separate 64-record / 64 MiB and 32-record / 64 MiB pools, charging 1 MiB and 2 MiB respectively.
Core snapshots, arguments and callbacks retain their own canonical quotas.

Observation edits carry native/configuration lifetime guards and current watch/value identities.
Running observations carry the module's source ID, preventing feedback from becoming another
launch/kill request. Accepted folder changes append distinct triggers and carry callbacks prepared
on the worker. Completed command callbacks describe past effects and use current module identity;
retiring the native session does not erase a completed outcome. Commands are never replayed.
Startup or unexpected worker failure schedules 250 ms to 8 s recovery, reset by a healthy report.
Native-mailbox pressure retains the same captured source behind an absolute 250 ms retry deadline.

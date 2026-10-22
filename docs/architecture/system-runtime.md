# System module ownership

Chataigne owns OS nodes and commands under `apps/chataigne/src/module/modules/system/os`.
The module projects authored state and observations; its `os_runtime/` worker owns metrics,
command preparation, DNS/UDP, process creation and native cleanup. `golden_io` provides the
public worker, retained-payload, lifetime and notification contracts. OS policy stays in the app layer.

App Control has its own watched-application/folder model under the adjacent `app_control/` tree.
Its remaining C06 ownership work is recorded in the
[closure audit](../progress/p03-closure-audit.md).

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

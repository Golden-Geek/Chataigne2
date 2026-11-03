# Reference media

MapGyver owns Reference authoring, mirrored output identity and dependency semantics under
`apps/mapgyver/src/media/reference`. Golden provides typed Parameters, history, script delivery
and the GPU operations. Reference media have their own `mapgyver.media.reference.v1` document
node: Target, disableable Resolution and Sampling. They contain no authored source channels,
source settings, transport or decoder.

A mirrored output is `ChannelReference { media: reference_id, channel: target_channel_id }`.
Its channel UUID comes from the terminal target's real channel node; its media UUID identifies
the Reference. Backend inspection resolves current target order/labels on demand. Add, remove,
rename and reorder require no mirror subtree edits, extra node labels or callback fan-out.
Unconfigured/missing targets expose no outputs; existing demanded identities become missing
and recover when the target returns. Channel settings are edited at the target, while a
Reference's own settings remain ordinary typed Parameters.

`InspectChannels { query: { media, start, count } }` returns at most 128 channel snapshots,
the current total and an authored membership status (Ready, Unconfigured, MissingTarget or
MissingMedia). It does not report decoder readiness. Reference chains distinguish an unset
target from a deleted target. Only the requested page projects channel definitions; counting
walks the terminal target's ordered channel siblings without allocating the full mirrored set.
The same query supports physical media and never changes history or document identity.
MapGyver registers it through Golden's immutable `ProjectLifecycle::query_app_operation` hook,
including typed Inspect Channels commands. UI, scripts and exclusive headless callers use the
same generated DTO and admitted result, without a document-wide mutation checkpoint.

The Reference Inspector's Channels section requests 32 outputs per page. It reflects current
target labels/order and navigates to a real target channel's generic Inspector for editing;
the mirrored output itself remains virtual. No add/remove controls imply local channel
ownership. The reader permits one active request plus one latest replacement and fences replies
by session, project, history and selection. Disconnect, active editing and disposal hide old
pages; removing the final item on a later page returns to the last valid page.
This bounds the Reference reader, not every existing workbench provider: the global
Composition catalog refresh remains a named M06 cleanup requirement in the status ledger.

The read resolver memoizes Reference chains within each immutable query. Catalog choice pages
iterate target channels and retain only the requested bounded page. Capture follows demanded
Reference outputs as dependencies under its existing count/guard limits, retaining Reference,
target and membership lifetime guards. No decoder or input subscription is copied.

The rendering DAG orders target channels before Reference outputs. Automatic References reuse
the prepared target texture directly and add no render pass. Enabled Reference Resolution
adds one sampling pass per demanded Reference output, shared by its consumers. Screen,
preview and composition routing consume the prepared target mapping, including aliases.

Cycle checks normalize aliases to actual target channels, retaining channel-specific edges
through Composition and Matrix. Bare Reference chains and mixed feedback reject before a
target parameter commits. The generic parameter hook reads proposed values through Golden's
borrowed validation context. Project validation memoizes chains and does not expand every
Reference × target-channel combination into document edges.

`CreateReference { target, label }` returns `ReferenceCreated { media }`; it invents no default
channel identity. `CreateMedia` with kind `Reference` uses the same creation boundary.
`ConfigureReference { media, target }` edits Target. Shared media Resolution/Sampling intents
also accept References. Mirrored channel definitions use `ChannelSource::Reference`; consumers
must treat those outputs as derived. Standard manager creation, folders, selection and the
generic Inspector expose the real Reference node.

The manager's named `createReference`, `configureReference` and `inspectChannels` methods use
the same intents, committed callbacks and embedded source examples. Typed command kinds use real
Reference-target Parameters. Composition/Matrix drop commands use command document
schema **`mapgyver.media.command.v4`**; unsupported older records reject explicitly. Existing
source media remain schema v5; Reference uses its separate v1 schema. No migration is added.

## Functional acceptance

**M04/7 accepted: 2026-11-03.** The [requirement-by-requirement evidence](../progress/evidence/mapgyver-reference-native-2026-11-03.json)
links channel synchronization, missing/recovered targets, mixed cycle rejection, source/decoder
ownership, resampling, default/Alt subtree semantics and Parameter/API/history/project parity
to their owning tests. Reference core, paged presentation and
[Composition/Matrix-owned drops](mapgyver-owned-media.md) are verified through typed callers
and all four bundled Reference workflows.

The complete production native fixture passes Solid Color, Picture (alpha 128) and Shared
Texture on Intel Graphics and NVIDIA RTX 5080: **six hardware/source cases, 136.91 seconds**.
Automatic aliases reuse the producer target; forced 3×2 Resolution adds a shared-source pass.
Three native outputs pass all pixels within 2/255 through nested Composition/Matrix, project
reload, authored target deletion/Undo, independent copying, playing-copy removal and final-use
retirement. The fixture waits for copied-input readiness and services the production source
notification path. Earlier fixture failures remain in the evidence; all owned processes exit.

The [unchanged authored backend fixture](../progress/evidence/mapgyver-reference-scale-2026-11-03.json)
passes at 10k References/54,111 actual nodes and 512 producer variants: read/edit p95
**0.102/22.6 ms**, 10,726-byte pages and 16 demanded capture records. Golden's immutable app
queries and shared graph-event payloads preserve history/rollback and wire data. Library
coverage is 317 MapGyver and 672 Golden engine tests, with strict lint and generated-contract
checks; native prerequisites compiled before execution.

This closes Reference's functional gate. Whole-app Q04 authored-count scaling, UI latency,
memory, cadence and output budgets remain under M09 with their M06/M07 dependencies. The native
fixture uses tiny output probes; broader source lifecycle/cross-adapter qualification, genuine
device removal and physical display coverage retain their owning gates. M04/8 Media Preview
is the next product deliverable in the [status ledger](../progress/mapgyver-status.md).

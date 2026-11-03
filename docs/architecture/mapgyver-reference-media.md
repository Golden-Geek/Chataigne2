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

Reference core, paged output presentation, [Composition/Matrix-owned Reference/copy drops](mapgyver-owned-media.md)
and missing/unconfigured source-drop parity are software/bundled-verified. Full mixed/end-to-end
scale and native sharing/resampling/recovery remain open. The existing native fixture now includes
all three source kinds on both hardware adapters, nested Composition/Matrix, missing-target Undo,
reload and copied-source final-use retirement; it compiles but has not run.

The [authored scale fixture and two observed results](../progress/evidence/mapgyver-reference-scale-2026-11-03.json)
cover up to 10k actual References and 512 producer variants. Pages and demanded capture stay
bounded, but no passing isolated 50 ms receipt result exists. Golden checkpoints now retain their
existing copy-on-write parameter values; document-wide transaction work remains outside the
Reference resolver. Next move pure app reads behind an immutable public Golden query contract.
The [status ledger](../progress/mapgyver-status.md) keeps the full gate open; this is not Q04 acceptance.

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

The manager's named `createReference` and `configureReference` methods use the same intents,
committed callbacks and embedded source examples. Corresponding typed command kinds use real
Reference-target Parameters. Adding that command Parameter advances the command document
schema to **`mapgyver.media.command.v2`**; unsupported v1 records reject explicitly. Existing
source media remain schema v5; Reference uses its separate v1 schema. No migration is added.

This is the first M04/7 product slice. Full acceptance still requires the complete mirrored
output presentation/selection and stress coverage, default-drop/Alt-copy child-media behavior,
complete mixed Matrix/Reference parity and native sharing/resampling/recovery checks. The
[status ledger](../progress/mapgyver-status.md) keeps the full gate open.

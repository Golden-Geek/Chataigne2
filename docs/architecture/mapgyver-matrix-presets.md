# Matrix configuration presets

MapGyver owns configuration capture/recall under `apps/mapgyver/src/matrix`; Golden owns
typed child nodes, references, metadata, atomic edits/history and the generic Inspector.
M15/4 is being delivered in slices. Column muting is implemented and functionally verified;
typed preset storage, capture/recall and contextual/command/script APIs are still pending.

- Presets store stable Layer/Column references, relative order and authored enabled metadata.
  They store no Cell contents, playback position, transition state or duplicate enable flags.
- Recall orders captured members within their current parent's captured-member slots.
  Groups, new members and other children keep their slots. Reparented members stay in their
  current parent; recall never reconstructs old hierarchy or moves Cells independently.
- Missing/deleted members are skipped. Added members remain untouched. Empty recalls are no-ops.
  Updating a preset replaces its captured membership with the current configuration.
- Recall is one prevalidated history operation. Reject references to another Matrix and invalid
  entries before mutation; Undo/Redo restores exact order and metadata without touching media.
- Enabled state is ordinary node metadata. Disabling a Column suppresses its rendered Cell
  contributions, including outgoing retained sources, without changing authored Cell identity,
  playback requests, transport position or application-demand policy. Re-enable resumes the
  current logical mix. A disabled Column cannot receive its own control pulses, as today.
- Column Groups apply inherited enablement to their Columns. Runtime projection caches this
  state on authoring changes; frame capture uses bounded constant-time lookups, not tree walks.
  Each retained clip binding remembers its last observed Column visibility independently of
  its pinned Cell source. Replacement/deletion cannot bypass muting or redirect outgoing pixels.
- Preset capture/recall does not synthesize Play/Stop. Existing media activation, application
  control and fade policies remain authoritative during configuration changes.

Typed preset entries will be real reference/order/enabled Parameters in child nodes, edited
through the ordinary Inspector. Backend intents resolve defaults and mutation semantics for
UI, transport, scripts and commands alike. Capture builds one detached subtree; recall plans
the complete edit set before applying it. This note is the execution contract, not evidence
that the pending preset feature has been implemented or accepted.

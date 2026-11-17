# Matrix configuration presets

MapGyver owns configuration capture/recall under `apps/mapgyver/src/matrix`; Golden owns
typed child nodes, references, metadata, atomic edits/history and the generic Inspector.
M15/4 is being delivered in slices. Column muting is functionally verified. Typed preset
storage and capture/update/recall now live in `matrix/presets`; command conveniences,
member-management presentation and complete gate qualification remain pending.

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

Preset entries are real reference/order/enabled Parameters. An intrinsic collection lives
inside the Media Parameters folder, so it does not become a channel or Matrix axis child.
The shared Media declaration creates one passive collection per Media; operations admit
Matrix owners only. This keeps persistence/copy ownership in the ordinary declared subtree.
The Matrix configurations icon opens contextual capture/recall/update/inspect/delete actions;
the generic Inspector owns preset names. Member-list presentation is a remaining slice.

Backend intents resolve defaults and mutation semantics for UI, transport and the existing
generic script request API. Capture builds one detached subtree; update inserts its replacement
entry forest in one batch; recall plans the complete edit set before applying it. Explicit
capture admits up to 128 presets, bounding the names returned with each Matrix viewport.
Command-node/script convenience surfaces and complete limit/admission qualification remain
required before M15/4 acceptance. This note defines behavior; the status ledger owns evidence.

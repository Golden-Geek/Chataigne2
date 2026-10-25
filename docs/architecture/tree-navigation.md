# Tree presentation and keyboard navigation

Golden UI owns the app-agnostic tree presentation shared by the project outliner, manager panels
and node picker. App wrappers provide their node predicates, child boundaries, row supplements
and activation policy. Chataigne's module, Formula library, property and processor managers use
the shared manager; the processor manager no longer maintains a separate recursive renderer.

`collectVisibleOutlinerRows` projects expanded and filtered paths, including hidden-row hoisting.
`createOutlinerProjection` retains visited-node lookups per panel and reconciles published
`VersionedNodeMap` changes. This avoids repeated persistent-trie traversal for unchanged siblings.
It releases its cache when its owning graph/manager/picker disappears. Ordinary mutable maps
are read directly rather than cached across calls.
Openness edits reproject changed branches and retain unchanged row objects. Backend metadata
acknowledgements that do not change visibility retain the complete row array. Structural edits,
search and policy changes use a full projection. Child/visibility policies are pure node predicates;
a caller supplies a new predicate when its policy changes.

`TreeRows` owns roving focus, accessible treeitem positions and keyboard intents over that logical
projection. `VirtualRows` owns row geometry, bounded mounting and retention of the focused DOM row.
Navigation never depends on the currently mounted subset. Up/Down and Home/End move focus;
Right expands or enters a branch; Left collapses or returns to the nearest visible parent.
Enter/Space activate the focused selectable node using the caller's selection policy. Focus does
not select automatically. Search's Down operation enters the current roving row. Embedded controls
keep their own key handling.

An explicit collapse takes precedence over automatic ancestor visibility. A new external outliner
selection opens its ancestors once; it cannot continually undo a later manual collapse. Manager
and picker expansion continue through their existing backend metadata intents and rejection paths.
Outliner openness remains panel-owned persisted presentation.

Filtering or collapse preserves the focused identity when visible, otherwise falls back to its
nearest visible ancestor or neighboring row. Empty projections focus their tree container without
stealing focus from search. Pending reveal/focus is fenced by navigation generation, session,
snapshot generation, owner identity and node UUID. New focus owners win over pending work.

The [P11 keyboard evidence](../progress/p11-tree-keyboard.md) distinguishes shared browser
qualification from remaining app-consumer and constrained-picker coverage. Native physical-input
usability belongs to P17.

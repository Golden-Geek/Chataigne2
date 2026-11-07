# Manager creation and disclosure

Golden owns manager creation feedback and filtered tree presentation. Apps register
their admitted hierarchy and use the same public Add/context menus. Creation remains
an engine operation available without a mounted manager or UI.

`CreateUserItem` returns `UiAck.created_node` only after its atomic edit commits.
The Rust-owned receipt contains the exact runtime address and stable UUID of the
created root. Catalog aliases and concrete node types may differ; callers never
scan sibling additions or poll for a guessed type. Protocol version 0.15.0 and its
TypeScript declarations are generated together from `golden_protocol`.

Golden's menu selection helper sends the receipt to its existing pending-selection
store. An acknowledgement may precede the graph delta; selection waits for the
exact projected identity without polling. Add buttons, single-item creation and
nested context menus share it.
Manager menus select every successfully created root; catalog selection hints
remain available to specialized receipt-only editor callers.
A changed session, document generation, explicit selection or recycled identity
rejects late selection feedback. Failure does not select. Receipt-only editor,
script and headless callers retain their own selection policy.

The shared outliner projection determines disclosure from admitted visible children,
including descendants hoisted through hidden structural nodes. The same projection
drives arrows, keyboard navigation and accessibility state. Raw Parameter children
cannot create an empty expander. Ordinary unfiltered branches retain bounded closed
traversal and the existing panel-owned lookup cache.

MapGyver registers media/folders, Matrix Layers/Groups/Columns/Column Groups/Cells
and owned media as rows. Its hidden channel nodes bridge the real Matrix hierarchy;
Parameter folders and Composition placement metadata stop traversal. Composition
children are their actual owned media. No parallel authored tree is retained in UI
state, and specialized editors remain responsible for domain operations.

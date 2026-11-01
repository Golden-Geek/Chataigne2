# MapGyver parameter ownership

Golden Core Parameter nodes own every authored setting. The app owns node schemas, domain
validation and specialized canvas/grid operations. Golden owns parameter value types, controls,
references, generic Inspector, events, history and persistence. DTO definitions are validated
operation inputs or read/runtime projections; they are never a second authored document.

Use Params DSL declarations with meaningful labels, types, constraints and enum/reference/file
metadata. Editable lists use child nodes containing parameters. Names, identities, hierarchy and
ordering use Golden's normal node metadata/topology rather than duplicate settings. Editor zoom,
selection and native observations remain presentation/runtime state.

Ordinary properties render through the default Inspector children. Custom inspector content is
limited to contextual operations and specialized presentation. A canvas gesture, script method or
domain command writes the same real parameters as a generic widget. Do not encode a definition
or collection as one JSON/string parameter, or maintain parallel UI-side values.

## Conversion inventory

The [implementation plan](../plan/MapGyver_Implementation_Plan.md#parameter-node-ownership-correction--2026-11-01)
and [current ledger](../progress/mapgyver-status.md) track acceptance. This inventory is the full
required scope; a converted owner does not complete the refactor.

| Owner | Required parameter fields | State |
| --- | --- | --- |
| Composition instance | Source media/channel references; position, scale, rotation, pivot; opacity, blend, enabled | Verified: generic Inspector, history/rejection, complete Parameter copy and project roundtrip |
| Media/channel | Media/source/output kind; automatic/fixed size, extent/filter; solid RGBA; Picture asset reference; Spout selector and maximum texture descriptor; Composition/Matrix canvas and transition settings | Pending |
| Asset | Typed locator kind/path and authored preparation settings | Pending |
| Screen | Enabled; target kind, display identity, rectangle origin/extent, Spout name/extent | Pending |
| Surface | Enabled, source references, crop, four corner positions, opacity and calibration | Pending |
| Matrix row/group | Opacity, blend and enabled; existing node hierarchy/order | Pending |
| Matrix column/cell | Transition override/curve/duration; application-association child nodes with reference/pre/post; column/source references on cells | Pending |
| Application | Executable/directory, argument children, environment mode/entry children, window mode, readiness/sender/evidence/timeout, stop deadlines, restart/backoff | Pending |
| Commands and inspector registrations | Audit remaining input declarations; remove ordinary property forms and opaque authored codecs | Pending |

## Validation and history

Parameter leaf constraints validate types/ranges/enums/references. Golden's
`Node::validate_parameter_change` checks ancestor domain invariants before committing a value,
history or events. Its borrowed graph view avoids building a process-tree snapshot. Domain
owners enforce geometry and reachable-source cycle rules here so raw parameter edits receive the
same guarantees as operation adapters. Structural operations remain explicit backend intents.

Composition instances currently expose nine parameters beneath their declared Parameters folder.
Their definition projects Golden's synchronized typed handles. The ordinary property form and
opaque instance codec are removed; unsupported old opaque instance records fail explicitly.
Backend operations emit SetParam edits with independent command history; explicit gesture groups
retain one undo unit. Copy uses Golden's complete subtree capture/import boundary in one forest,
preserving values, defaults, user-editable constraints and control subtrees with independent
identities. Source references remain shared. Producer-owned constraints rebuild from declarations
on project load, following Golden's standard policy. Instances use the default Inspector without
an app-specific inspector component. Existing reference scenes remain historical evidence.

The additional real nodes require an explicit, bounded MapGyver materialization policy:
4,194,304 generated-work units with Golden's unchanged 64-pass limit. This supports the existing
100k-instance admission with declared parameter children. The same policy is applied before
new-project, sparse baseline, load/recovery and subtree-import expansion. Golden's shared default
remains unchanged. This structural guard is separate from all retained renderer, memory, frame
and recovery acceptance budgets.

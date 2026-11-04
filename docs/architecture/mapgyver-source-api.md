# MapGyver source automation

MapGyver owns source methods and script snippets under `apps/mapgyver/src/media`.
Golden owns the script runtime, method dispatch, bounded app-operation queue, history and
callback delivery. Named methods translate arguments to the existing Rust-owned `MediaEdit`
contract; they do not edit internal Parameters directly or invent alternate source policy.

Create a script with **Add Media Script** in the Media Manager Inspector, or the
`CreateScript { label }` media intent. This explicit operation installs the embedded app
template as an ordinary Golden Script child and commits one history step. Scripts stay out
of the manager's object outliner and creation menus. Its Inspector Scripts section mounts at
most 32 scripts per page; selecting a script opens Golden's standard code editor. Inline
templates have no update callback and remain passive after loading.
Save edited source with the standard editor's Ctrl+S action, then use **Reload Runtime** to
execute changed initialization code.

`local` refers to Media Manager from these scripts. Each method starts with a nonempty request
id. UUID arguments come from committed results, not numeric session node IDs. Empty optional
labels use backend defaults; empty asset/sender strings clear that channel's source setting.
`Reference` creation returns `ReferenceCreated { media }`, because an unconfigured Reference
has no output channel. Its [target/output contract](mapgyver-reference-media.md) is distinct
from physical source-channel authoring. Command records now use schema v5; older schemas reject clearly.

`inspectChannel` is an immutable read: `Channel.snapshot` is null when the owner or channel
has been removed, allowing queued Inspector reads to retire safely. Existing invalid node
types or ownership still reject. Neither reads nor their rejections add history entries.

| Method                     | Arguments after request id                                                      |
| -------------------------- | ------------------------------------------------------------------------------- |
| `createMedia`              | kind (`SolidColor`, `Picture`, `SharedTexture`, `Composition`, `Matrix`, `Reference`), label |
| `createReference`          | target media UUID or empty string, label                                      |
| `configureReference`       | Reference media UUID, target media UUID or empty string                       |
| `inspectChannels`          | media UUID, nonnegative integer start, count in 1–128                          |
| `dropMatrixMedia`          | Matrix media, layer, column and source node UUIDs; copy boolean |
| `dropCompositionMedia`     | Composition media/channel UUIDs, source node UUID, replaced media UUID or empty string, copy boolean, canvas X/Y |
| `addChannel`               | media UUID, label; backend rejects derived outputs                              |
| `configureMediaResolution` | media UUID, enabled boolean, width, height                                      |
| `configureMediaFitMode`    | Picture, Shared Texture or Reference media UUID; `Fit`, `Fill` or `Stretch`       |
| `configureSolidColor`      | media UUID, channel UUID, straight linear R, G, B, A                            |
| `configurePicture`         | media UUID, channel UUID, asset UUID or empty string                            |
| `configureSharedTexture`   | media UUID, channel UUID, exact sender or empty string                          |
| `inspectChannel`           | media UUID, channel UUID                                                        |
| `importPicture`            | path, absolute boolean; otherwise a validated project-relative locator          |
| `executeCommand`           | configured command UUID                                                         |
| `inspectMedia`             | no further arguments                                                            |
| `editMedia`                | JSON serialization of the complete typed `MediaEdit` intent                     |

Enabled Resolution requires whole pixels in 1–16384 per axis. Disabling it retains authored
pixel values and projects source-native/procedural sizing. Width/height arguments are still
finite numbers when disabled. Resolution and Fit Mode apply to all channels; named source
methods configure only the selected variant. Import queues detached asset/media creation;
file access and decoding remain on workers.
Fit Mode applies only at forced Resolution; Auto retains its choice and preserves intrinsic
source dimensions. The intent and named script method edit the same typed media Parameter.
Solid Color and authored Composition/Matrix canvases reject this operation without mutation.
Sampling is fixed to Linear; the former sampling intent/method/command is removed rather than
silently ignored. The typed `ConfigureFitMode` command edits the same Fit Mode Parameter.

`onAppOperationResult(reply)` runs after the operation commits or rejects. `request_id`
correlates the call; `status` is `applied` or `rejected`. Applied replies contain the same
`value` returned to UI/headless callers; rejection contains `message` and no value. Invalid
method argument shapes fail before queuing. Inspections and rejections add no history.
Undo/Redo replays authored state rather than requests; disposal/project replacement fences
pending operation owners. Callbacks may queue the next explicit operation.

The embedded `script_templates/source_api.js` and `callbacks.js` document runnable examples.
No repository files are read when an installed app creates the template. The generic
`editMedia` surface retains folders, asset management, ordering/removal and specialized
Composition/Matrix intents; see [the Rust contract](../../apps/mapgyver/src/media/protocol.rs).

The Media Manager Inspector's **Commands** section creates app-owned
`mapgyver.media.command.v4` children through `CreateCommand`. `InspectCommandCatalog` returns
the Rust-owned catalog; the generator exports each command's visible Parameter fields.
The section mounts at most 32 commands per page. Commands stay out of object creation menus
and the manager outliner. Their configuration uses the ordinary Golden Inspector and real
typed Parameters, including references, enums, file paths, resolution and color.

The 40 command kinds cover app control, source/variant and Reference creation, Composition and Matrix Reference/copy drops, settings, assets, folders, ordering,
removal, bounded inspection and script creation. `ExecuteCommand { command }`, the Execute
Trigger, `local.executeCommand(requestId, commandUuid)` and the command node's
`execute(requestId)` method share existing `MediaEdit` behavior and history. The command node's
Trigger/method captures configuration before entering the bounded operation queue; commands have
no update polling. Disable/removal invalidates pending command-owned requests. Reload,
copy and Undo/Redo restore authored configuration without firing commands. Results use the
same committed callback envelope and media result event; rejected executions also set a
command warning. The revised M04/1 contract is
[accepted with source/test and bundled workflow evidence](../progress/mapgyver-m04-source-api.md).
Native variant Resolution/sharing is
[accepted on both hardware adapters](../progress/mapgyver-m04-variants.md). Full native
lifecycle/resource parity, Reference and Media Preview retain separate open gates in the
[status ledger](../progress/mapgyver-status.md).

## App-control commands and scripts

`applications/scripting.rs` owns eight named methods registered by the Medias script node:
`setApplicationFile`, `configureApplication`, `inspectApplication`, `inspectApplicationRuntime`,
`inspectApplicationCallbacks`, `launchMatrixColumn`, `launchMatrixCell` and `unloadMatrix`.
Every call takes a request id first and projects to the same typed application/Matrix intents
used by other callers. The owner id is a real Column or Cell UUID; there is no standalone
Application authoring API. `configureApplication` accepts a bounded transient JSON call argument,
never an authored JSON/String Parameter. Argument validation happens before queuing.

Seven **App control** command kinds expose file selection, Column/Cell launch, Matrix unload
and the three inspections through real File, NodeReference and enum Parameters. Rust generates
their TypeScript contract and visible fields; the ordinary Golden Inspector edits them.
`SetFile` changes only the executable through the existing atomic configuration boundary,
preserving arguments, environment and every other setting. Empty file values clear it.
Editing, project load and Undo/Redo never fire a command or replay OS side effects.

The embedded `applications/script_template.js` supplies request examples and
`onApplicationLifecycle(event)`. Project/sequence and runtime/launch identity describe the
ordered lifecycle of a shared runtime owner; runtime inspection maps it to current consumers.
These event-driven callbacks use the existing project-fenced delivery, with no idle polling.
[The API milestone evidence](../progress/mapgyver-m08-control-api.md) covers software and bundled
command UI behavior. The complete native picker, target/access, recovery and shutdown acceptance
remains M08/4 in the status ledger.

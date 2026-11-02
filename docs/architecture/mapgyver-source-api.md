# MapGyver source scripting

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

| Method                     | Arguments after request id                                                      |
| -------------------------- | ------------------------------------------------------------------------------- |
| `createMedia`              | kind (`SolidColor`, `Picture`, `SharedTexture`, `Composition`, `Matrix`), label |
| `addChannel`               | media UUID, label; backend rejects derived outputs                              |
| `configureMediaResolution` | media UUID, enabled boolean, width, height                                      |
| `configureMediaSampling`   | media UUID, `Linear` or `Nearest`                                               |
| `configureSolidColor`      | media UUID, channel UUID, straight linear R, G, B, A                            |
| `configurePicture`         | media UUID, channel UUID, asset UUID or empty string                            |
| `configureSharedTexture`   | media UUID, channel UUID, exact sender or empty string                          |
| `inspectChannel`           | media UUID, channel UUID                                                        |
| `importPicture`            | path, absolute boolean; otherwise a validated project-relative locator          |
| `inspectMedia`             | no further arguments                                                            |
| `editMedia`                | JSON serialization of the complete typed `MediaEdit` intent                     |

Enabled Resolution requires whole pixels in 1–16384 per axis. Disabling it retains authored
pixel values and projects source-native/procedural sizing. Width/height arguments are still
finite numbers when disabled. Sampling and Resolution apply to all channels; named source
methods configure only the selected variant. Import queues detached asset/media creation;
file access and decoding remain on workers.

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
Typed command-node authoring remains an open M04/1 requirement in the
[status ledger](../progress/mapgyver-status.md).

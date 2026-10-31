// Media Manager functions. Authored edits use the Rust MediaEdit contract and one atomic history step.
// local.editMedia(requestId, JSON.stringify(mediaEdit))
// local.inspectMedia(requestId)
// A command node exposes execute(), using its authored Request and Request Id parameters.
// Request ids correlate callbacks; choose distinct ids for concurrent calls.
//
// Examples (defaults and identities are allocated by the backend):
// local.editMedia("create", JSON.stringify({
//   kind: "CreateMedia", media_kind: "SolidColor", label: null
// }));
// local.inspectMedia("catalog");
// Use the returned channel reference for ConfigureChannel, RenameChannel, ReorderChannel,
// RemoveChannel or InspectChannel. ImportPicture, asset operations and media operations use
// this same entrypoint; definitions come from the catalog rather than UI/script defaults.
// Composition media uses media_kind: "Composition". Add ordered instances through:
// local.editMedia("instances", JSON.stringify({
//   kind: "Composition", reference: compositionChannelReference,
//   edit: { kind: "AddInstances", sources: [sourceChannelReference] }
// }));
// Inspect uses edit: { kind: "Inspect" }; DuplicateInstances, ConfigureInstance, RenameInstance,
// ReorderInstance and RemoveInstance use the returned stable instance identity.
// EditInstanceGeometry uses { mode: "Translate" | "Rotate" | "Scale", origin, start, point }.
// Copy origin from the inspected instance transform; start/point are canvas-texel pointer samples.
// The backend changes only that geometry field and returns InstanceChanged with committed state.
// Matrix media uses media_kind: "Matrix". Its Final, Layer and Group channels are stable exports.
// Shared app definitions use {kind:"Applications", edit:{kind:"Create",label:null}}.
// Configure/Rename/Remove/Duplicate/Inspect take application: applicationId; InspectCatalog takes
// search/start/count (1..128). These authored operations never launch a process.
// Matrix ConfigureColumn preserves transition plus applications: [{application: applicationId, pre:1, post:1}].
// Distances are clamped column neighbors, not time; copies keep shared app IDs. Missing refs survive Undo.
// InspectRuntime returns the live project/launch identities and explicit lifecycle phases through
// the same onAppOperationResult callback. elapsed_ms and next_action_ms share one monotonic origin.
// An explicitly started backend lifecycle owner supplies demand/readiness; authoring never runs it.
// local.editMedia("layers", JSON.stringify({ kind: "Matrix", media: matrixMediaId,
//   edit: { kind: "AddLayers", parent: null, count: 4 } }));
// Matrix Inspect returns ordered items/columns and sparse cells. AddColumns does not create cells.
// For large documents, InspectViewport accepts viewport: { row_start, row_count, column_start,
// column_count }; responses contain only that window (maximum 4096 addresses, 128 rows, 64 columns).
// InspectChoices uses query: { scope: "All" | "MatrixFinal", search, start, count } for bounded
// source/media selection (maximum 128 choices). These reads create no history or playback launch.
// Matrix InspectTargets uses query: { scope: { kind: "Groups" | "Columns" } or
// { kind: "Items", parent: groupIdOrNull }, search, start, count } for paged move destinations.
// SetCells accepts assignments: [{ layer, column, source: channelReferenceOrNull }].
// Configure/ConfigureItem/ConfigureColumn, copy, move/reorder, rename and removal use the same route.
// Authored cells reference channels without owning them; feedback is rejected across all media.
// local.editMedia("launch", JSON.stringify({ kind: "Matrix", media: matrixMediaId,
//   edit: { kind: "LaunchColumn", column: columnId, mode: "Cut" } }));
// InspectPlayback reads committed desired state, independently of selection or renderer installation.
// Launches are ephemeral and do not add authored history. Use mode: "Configured" for the column
// override or Matrix default fade. Interrupted fades preserve current weights and outgoing demand.

function init() {
  // Subscribe once. No update callback or periodic polling is needed.
  local.listen({ level: 1 });
}

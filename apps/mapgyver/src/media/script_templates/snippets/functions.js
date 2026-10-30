// Media Manager functions. All edits use the Rust MediaEdit contract and one atomic history step.
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

function init() {
  // Subscribe once. No update callback or periodic polling is needed.
  local.listen({ level: 1 });
}

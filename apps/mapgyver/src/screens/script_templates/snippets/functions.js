// Screen Manager methods share the Rust ScreenEdit contract and atomic Undo boundary.
// local.editScreens(requestId, JSON.stringify(screenEdit))
// local.inspectScreens(requestId)
// A Screen Operation command also exposes execute() for its authored request.
// Use distinct request ids for concurrent operations.
//
// local.editScreens("create", JSON.stringify({ kind: "CreateScreen", label: null }));
// local.inspectScreens("catalog");
// Use returned Screen/Surface identities for configuration, rename, reorder and removal.
// A Screen starts disabled with no target. Its initial Surface covers the complete source
// and Screen. Channel references come from the Media Manager; never invent identities.
// Calibration changes preserve the authored source. Desktop coordinates are physical pixels;
// crop and quad coordinates are normalized source and Screen fractions respectively.
// DropSource accepts a Media or Channel node UUID. Use surface:null to create a default
// Surface, or an existing Surface UUID to replace only its source. A Media resolves its
// first channel in authored order; a Channel resolves itself, for UI and script callers alike.
// local.editScreens("drop", JSON.stringify({kind:"DropSource", screen:screenId, surface:null, source:nodeUuid}));

function init() {
	local.listen({ level: 1 });
}

// MapGyver media API. Request ids correlate asynchronous committed/rejected callbacks.
// Empty labels use backend defaults. IDs are the UUID strings returned in callbacks.
// Shared Resolution and Sampling belong to media; source settings belong to channels.
script.setApiVersion(1);

function init() {
	local.listen({ level: 1 });
	// local.createMedia("create", "SolidColor", "");
	// local.importPicture("picture", "images/example.png", false); // false: project-relative
}

function sourceExamples(mediaId, channelId, assetId) {
	// local.editMedia("command", JSON.stringify({kind: "CreateCommand", command_kind: "ConfigureResolution", label: null}));
	// Configure the command's typed Parameters in the Inspector, then:
	// local.executeCommand("run-command", commandId); // or command.execute("run-command")
	// Uncomment the operations you need. Each call is its own atomic history step.
	// local.dropMatrixMedia("cell", matrixId, layerId, columnId, mediaId, false); // true copies the full media
	// local.dropCompositionMedia("drop", compositionMediaId, compositionChannelId, mediaId, "", false, 0, 0); // true copies the full media
	// local.createReference("reference", mediaId, ""); // ReferenceCreated returns media UUID
	// local.configureReference("retarget", referenceMediaId, mediaId); // empty target clears
	// local.addChannel("variant", mediaId, "");
	// local.configureMediaResolution("size", mediaId, true, 1920, 1080);
	// local.configureMediaResolution("native-size", mediaId, false, 1920, 1080);
	// local.configureMediaSampling("sampling", mediaId, "Linear"); // or "Nearest"
	// local.configureSolidColor("color", mediaId, channelId, 1, 0, 0, 1); // straight linear RGBA
	// local.configurePicture("asset", mediaId, channelId, assetId); // "" clears the asset
	// local.configureSharedTexture("sender", mediaId, channelId, "Exact sender"); // "" clears
	// local.inspectChannel("channel", mediaId, channelId);
	// local.inspectChannels("channels", mediaId, 0, 32); // bounded page; also mirrors References
	// local.inspectMedia("catalog");
	// local.editMedia("rename", JSON.stringify({kind: "RenameMedia", media: mediaId, label: "Source"}));
	// editMedia accepts the complete Rust-owned MediaEdit contract, including folders,
	// assets, channel reorder/removal, Composition and Matrix operations.
}

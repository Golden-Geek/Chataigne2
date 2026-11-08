// MapGyver media API. Request ids correlate asynchronous committed/rejected callbacks.
// Empty labels use backend defaults. IDs are the UUID strings returned in callbacks.
// Shared Resolution and Fit Mode belong to media; source settings belong to channels.
script.setApiVersion(1);

function init() {
	local.listen({ level: 1 });
	// local.createMedia("create", "SolidColor", "");
	// local.importPicture("picture", "images/example.png", false); // false: project-relative
}

function sourceExamples(mediaId, channelId, assetId) {
	// local.createMedia("video", "Video", "Clip"); // one transport, one File per channel
	// local.configureVideoFile("file", mediaId, channelId, "clips/example.mov");
	// local.playVideo("play", mediaId); local.pauseVideo("pause", mediaId); local.stopVideo("stop", mediaId);
	// local.seekVideo("seek", mediaId, 1250.5); // milliseconds; generic Inspector edits the same Parameter
	// local.setVideoRate("rate", mediaId, 0.5); local.setVideoLoop("loop", mediaId, true);
	// local.inspectVideoFrame("frame", mediaId, channelId); // null while timestamp preparation is pending
	// local.inspectVideoSource("source", mediaId, channelId); // timing/decoded readiness, failure kind and retry state
	// local.stepVideo("next", mediaId, channelId, "Next"); // commits a prepared exact timestamp; no FPS estimate
	// local.inspectVideo("state", mediaId); // readiness/actual state differs from authored intent
	// local.editMedia("command", JSON.stringify({kind: "CreateCommand", command_kind: "ConfigureResolution", label: null}));
	// Configure the command's typed Parameters in the Inspector, then:
	// local.executeCommand("run-command", commandId); // or command.execute("run-command")
	// Uncomment the operations you need. Each call is its own atomic history step.
	// local.dropMatrixMedia("cell", matrixId, layerId, columnId, mediaId, false); // true copies the full media
	// local.transferMatrixCell("transfer", matrixId, cellId, layerId, columnId, "Drop"); // Drop swaps/moves; Move/Copy/Reference require empty destination
	// local.dropCompositionMedia("drop", compositionMediaId, compositionChannelId, mediaId, "", false, 0, 0); // true copies the full media
	// local.createCompositionMedia("create-child", compositionMediaId, compositionChannelId, "SolidColor", "", 320, 180);
	// local.createReference("reference", mediaId, ""); // ReferenceCreated returns media UUID
	// local.selectReferenceChannel("output", referenceMediaId, channelId); // empty channelId follows the first available output
	// local.configureReference("retarget", referenceMediaId, mediaId); // empty target clears
	// local.addChannel("variant", mediaId, "");
	// local.configureMediaResolution("size", mediaId, true, 1920, 1080);
	// local.configureMediaResolution("native-size", mediaId, false, 1920, 1080);
	// local.configureMediaFitMode("fit", mediaId, "Fit"); // Picture/Shared Texture/Reference: Fit, Fill, Stretch
	// local.configureMediaSenderSelection("mode", mediaId, "Auto"); // Shared Texture: Manual or Auto; carries each exact sender
	// local.configureSolidColor("color", mediaId, channelId, 1, 0, 0, 1); // straight linear RGBA
	// local.configurePicture("picture", mediaId, channelId, "images/source.png"); // "" clears the File
	// local.configurePictureFile("file", mediaId, channelId, "images/source.png"); // edits this Picture channel's own File; "" clears it
	// local.inspectPictureFile("file-binding", mediaId, channelId); // bounded live File Parameter address
	// local.reloadPicture("reload", mediaId, channelId); // retry this File without authored history
	// local.configureSharedTexture("sender", mediaId, channelId, "Exact sender"); // "" clears
	// local.inspectChannel("channel", mediaId, channelId);
	// local.inspectChannels("channels", mediaId, 0, 32); // bounded page; also mirrors References
	// local.inspectMedia("catalog");
	// local.editMedia("rename", JSON.stringify({kind: "RenameMedia", media: mediaId, label: "Source"}));
	// editMedia accepts the complete Rust-owned MediaEdit contract, including folders,
	// assets, channel reorder/removal, Composition and Matrix operations.
	// local.editMedia("axis", JSON.stringify({kind: "Matrix", media: matrixId, edit: {kind: "CreateAxisItem", item: "Column", context: selectedNodeUuid}})); // backend resolves selected hierarchy; null uses root
}

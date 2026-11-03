// App control belongs to a Column or Cell. ownerId is that document object's UUID.
// The backend preserves all other settings when selecting/clearing only the App file.
function applicationExamples(ownerId, matrixId, layerId, columnId) {
	// local.setApplicationFile("app-file", ownerId, "C:\\Apps\\Example.exe");
	// local.setApplicationFile("clear-app", ownerId, "");
	// local.inspectApplication("app-settings", ownerId);
	// To edit all settings, modify the returned reply.value.snapshot.definition, then:
	// local.configureApplication("app-settings", ownerId, JSON.stringify(definition));
	// This JSON is a bounded call argument, never an authored String Parameter.
	// local.launchMatrixColumn("column", matrixId, columnId, "Configured"); // or "Cut"
	// local.launchMatrixCell("cell", matrixId, layerId, columnId, "Configured");
	// local.unloadMatrix("unload", matrixId);
	// local.inspectApplicationRuntime("runtime");
	// local.inspectApplicationCallbacks("callbacks");
}

function onApplicationLifecycle(event) {
	// Event-driven, project/launch-fenced delivery. Shared consumers have one runtime owner.
	// event.project and event.sequence identify ordered events within this project lifetime.
	// event.status.runtime/launch/phase describe the owned process; never adopt a PID by name.
	// local.inspectApplicationRuntime("runtime"); // maps runtime owners to current consumers
	// if (event.status.phase === "Failed") log("App failed", event.status.message);
}

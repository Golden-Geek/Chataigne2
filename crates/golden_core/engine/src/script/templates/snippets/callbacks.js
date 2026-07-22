let utilitiesFolder = null;

function init() {
  utilitiesFolder = script.addNode("folder", "Utilities");
  log(gainParam, utilitiesFolder);

  // `local` is the local node proxy. Use it to mutate the live tree.
  // Example:
  // if (local && local.depth === undefined) {
  //   local.addParameter("depth", 0.25);
  // }
}

function update(delta) {
  void delta;
  // Remove this callback when your script is event-driven only.
  //
  // Runtime tree examples:
  // if (!local) return;
  // local.listen({ level: 1 });
  // listen(root, { level: 2 });
  // local.enabled = true;
  // local.name = "Renamed local";
  // local.addFolder("Utilities");
  // local.addParameter("depth", 0.5);
  // local.depth = 0.75; // writes child parameter value
  // script.addParameter("feedback", { type: "float", default: 0.1 }); // local mutation in callbacks
}

function event(event) {
  if (event.kind === "paramChanged") {
    const payload = event.payload ?? {};
    void payload;
    log("paramChanged from node", event.origin);
  } else if (event.kind === "custom") {
    log("custom event received");
  }
}

function paramChanged(param, oldValue) {
  if (!gainParam || !param) {
    return;
  }
  if (param == gainParam || param.is(gainParam)) {
    log("gain changed", oldValue, "->", param.value);
  }
}

function destroy() {
  log("script destroyed");
}

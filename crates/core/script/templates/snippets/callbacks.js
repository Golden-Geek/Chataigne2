function init() {
  log(`script initialized (${String(GC_TEMPLATE)})`);
}

function update(delta) {
  void delta;
  // Remove this callback when your script is event-driven only.
}

function event(event) {
  if (event.kind === "paramChanged") {
    const payload = event.payload ?? {};
    void payload;
    log(`paramChanged from node ${String(event.origin)}`);
  } else if (event.kind === "custom") {
    log("custom event received");
  }
}

function destroy() {
  log("script destroyed");
}

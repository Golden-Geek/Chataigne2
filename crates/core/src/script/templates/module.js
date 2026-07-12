{{include:snippets/header.js}}

script.setApiVersion(1);
{{include:snippets/subscriptions/host.js}}

function init() {
  log("module-scoped script initialized");
}

// Uncomment if you need periodic processing:
// script.setUpdateRateHz(60);
// function update(delta) {
//   void delta;
// }

function event(event) {
  if (event.kind === "paramChanged") {
    log("module script noticed param change");
  }
}

on_init: function(ctx) {
  ctx.log("info", `script initialized (${String(GC_TEMPLATE)})`);
},

on_update: function(ctx, delta) {
  void ctx;
  void delta;
  // Remove this callback when your script is event-driven only.
},

on_event: function(ctx, event) {
  if (event.kind === "paramChanged") {
    const payload = event.payload ?? {};
    void payload;
    ctx.log("info", `paramChanged from node ${String(event.origin)}`);
  } else if (event.kind === "custom") {
    ctx.log("info", "custom event received");
  }
},

on_destroy: function(ctx) {
  ctx.log("info", "script destroyed");
},

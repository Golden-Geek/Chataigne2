{{include:snippets/header.js}}

return {
  api_version: 1,
  subscriptions: [
    {{include:snippets/subscriptions/host.js}}
  ],
  on_init: function(ctx) {
    ctx.log("info", "module-scoped script initialized");
  },

  // Uncomment if you need periodic processing:
  // update_rate_hz: 60,
  // on_update: function(ctx, delta) {
  // },

  on_event: function(ctx, event) {
    if (event.kind === "paramChanged") {
      ctx.log("info", "module script noticed param change");
    }
  },
};

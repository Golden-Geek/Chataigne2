// Default script template for Golden Core script nodes.
// Use include directives to compose reusable snippets.
{{include:snippets/header.js}}

return {
  api_version: 1,
  // Used only when on_update exists.
  {{include:snippets/update_rate.js}}
  parameters: {
    {{include:snippets/parameters/basic_gain.js}}
  },
  subscriptions: [
    {{include:snippets/subscriptions/host.js}}
  ],
  exports: {
    {{include:snippets/exports/ping.js}}
  },
  {{include:snippets/callbacks.js}}
};

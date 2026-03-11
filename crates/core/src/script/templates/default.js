// Default script template for Golden Core script nodes.
// Use include directives to compose reusable snippets.
{{include:snippets/header.js}}

script.setApiVersion(1);
{{include:snippets/update_rate.js}}
{{include:snippets/parameters/basic_gain.js}}
{{include:snippets/subscriptions/host.js}}
{{include:snippets/exports/ping.js}}

{{include:snippets/callbacks.js}}

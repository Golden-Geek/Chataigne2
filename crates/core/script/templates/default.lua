-- Default script template for Golden Core script nodes.
-- Use include directives to compose reusable snippets.
{{include:snippets/header.lua}}

return {
  api_version = 1,
  -- Used only when on_update exists.
  {{include:snippets/update_rate.lua}}
  parameters = {
    {{include:snippets/parameters/basic_gain.lua}}
  },
  subscriptions = {
    {{include:snippets/subscriptions/host.lua}}
  },
  exports = {
    {{include:snippets/exports/ping.lua}}
  },
  {{include:snippets/callbacks.lua}}
}

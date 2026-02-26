{{include:snippets/header.lua}}

return {
  api_version = 1,
  subscriptions = {
    {{include:snippets/subscriptions/host.lua}}
  },
  on_init = function(ctx)
    ctx.log("info", "module-scoped script initialized")
  end,

  -- Uncomment if you need periodic processing:
  -- update_rate_hz = 60,
  -- on_update = function(ctx, delta)
  -- end,

  on_event = function(ctx, event)
    if event.kind == "paramChanged" then
      ctx.log("info", "module script noticed param change")
    end
  end,
}

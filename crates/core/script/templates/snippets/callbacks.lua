on_init = function(ctx)
  ctx.log("info", "script initialized (" .. tostring(GC_TEMPLATE) .. ")")
end,

on_update = function(ctx, delta)
  local _ = ctx
  local _ = delta
  -- Remove this callback when your script is event-driven only.
end,

on_event = function(ctx, event)
  if event.kind == "paramChanged" then
    local payload = event.payload or {}
    local _ = payload
    ctx.log("info", "paramChanged from node " .. tostring(event.origin))
  elseif event.kind == "custom" then
    ctx.log("info", "custom event received")
  end
end,

on_destroy = function(ctx)
  ctx.log("info", "script destroyed")
end,

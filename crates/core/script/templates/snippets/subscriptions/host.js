// Listen from the local host node.
// level = 0 -> host only
// level = 1 -> host + direct children (default)
// level = 2 -> include grandchildren (useful when params are under folders)
if (local) {
  local.listen({ level: 2 });
}

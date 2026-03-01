// @host listens from the host node.
// max_depth = 0 -> host only
// max_depth = 1 -> host + direct children
// max_depth = 2 -> include grandchildren (useful when params are under folders)
{ node: "@host", max_depth: 2 },

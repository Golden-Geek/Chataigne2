// Golden Core script template.
//
// Global host helpers:
//   log(...args)        -> info level
//   success(...args)
//   warn(...args)
//   error(...args)
//   emit(topic, payloadObject)
//
// Script configuration methods:
//   script.setApiVersion(number)
//   script.setUpdateRateHz(number | null)
//   script.addParameter(name, specObject)   -> node handle (or deferred handle)
//   script.addNode(type, label, defaultValue?)
//   script.addFolder(label)
//   script.removeParameter(name)
//
// Runtime listener helpers:
//   listen(node, { level: 1 })  -> generic helper for any node/parameter
//   unlisten(node)
//   clearListeners()
//
// Tree access helpers:
//   tree.root(), root     -> engine root node proxy
//   tree.host(), local    -> script parent node proxy (host container)
//
// Node proxy features:
//   (implemented by node/parameter Rust script descriptors, not JS-side stubs)
//   local.childName            -> child node proxy or parameter value
//   local.someParam = value    -> sets child parameter value (if child is parameter)
//   local.name = "New Name"    -> metadata property write
//   local.enabled = false      -> metadata property write
//   local.getProperties()      -> plain object with node metadata/script properties
//   local.getChildren()        -> array of child node proxies
//   local.getChild(index|key)  -> child node proxy by index or by decl/name key
//   local.addFolder(label)
//   local.addParameter(name, defaultValueOrSpecObject)
//   local.removeParameter(name)
//   local.addNode(type, label, defaultValue?)
//   local.removeNode()
//   local.listen({ level: 1 })
//   local.unlisten()
//   String(local)              -> human readable summary for logs
//   local.is(otherNode)        -> id-based identity check
//
// Note:
//   script.addParameter/removeParameter called inside callbacks (init/update/event/destroy)
//   also apply live mutations on the Script node itself.
//
// Optional hook functions:
//   init(), update(delta), event(event), paramChanged(param, oldValue, event), destroy()
//
// Event object passed to event()/paramChanged():
//   event.kind   : "paramChanged", "childAdded", "childRemoved", "metaChanged", "custom", ...
//   event.origin : origin node id (or null)
//   event.payload: raw event payload object
const GC_TEMPLATE = "default";

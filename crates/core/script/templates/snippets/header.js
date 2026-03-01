// Golden Core script template.
//
// Global host helpers:
//   log(message)        -> info level
//   success(message)
//   warn(message)
//   error(message)
//   emit(topic, payloadObject)
//
// Script configuration methods:
//   script.setApiVersion(number)
//   script.setUpdateRateHz(number | null)
//   script.listen(nodeSelector, maxDepth)
//   script.unlisten(nodeSelector, maxDepth)
//   script.addParameter(name, specObject)
//   script.removeParameter(name)
//
// Optional hook functions:
//   init(), update(delta), event(event), paramChanged(event), destroy()
//
// Event object passed to event()/paramChanged():
//   event.kind   : "paramChanged", "childAdded", "childRemoved", "metaChanged", "custom", ...
//   event.origin : origin node id (or null)
//   event.payload: raw event payload object
const GC_TEMPLATE = "default";

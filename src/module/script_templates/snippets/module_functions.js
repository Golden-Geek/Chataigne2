/*
 * Module host quick reference:
 *   local                      -> this module node
 *   local.connection           -> Connection folder proxy
 *   local.parameters           -> Parameters folder proxy
 *   local.values               -> Values folder proxy
 *
 * Generic node functions available on local and child proxies:
 *   local.getChild(indexOrKey)
 *   local.getChildren()
 *   local.getProperties()
 *   local.setParam(key, value)
 *   local.addFolder(label)
 *   local.addParameter(name, defaultValueOrSpec)
 *   local.removeParameter(name)
 *   local.addNode(type, label, defaultValueOrSpec?)
 *   local.removeNode()
 *   local.listen({ level: 2 })
 *   local.unlisten()
 */


/*
 * MQTT module functions (call them on local):
 *   local.publish(topic, payload = "", qos = "at_most_once", retain = false)
 *   local.publishText(topic, text = "", qos = "at_most_once", retain = false)
 *   local.publishJson(topic, value, qos = "at_most_once", retain = false)
 *
 * QoS can be "at_most_once", "at_least_once", "exactly_once", or 0/1/2.
 */


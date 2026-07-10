/*
 * OS module functions (call them on local):
 *   local.shutdown()
 *   local.reboot()
 *   local.logout()
 *   local.wakeOnLan(macAddress, broadcastHost = "255.255.255.255", port = 9)
 *
 * `wakeOnLan` sends a standard UDP magic packet to the selected broadcast target.
 */
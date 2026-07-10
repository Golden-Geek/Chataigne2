/*
 * Stream module functions (call them on local):
 *   local.sendText(text, lineEnding = "none")
 *   local.sendString(text, lineEnding = "none")
 *   local.sendBytes(...bytes)
 *   local.sendData(...bytes)
 *   local.sendHex(hex)
 *   local.sendHexString(hex)
 *
 * lineEnding can be "none", "nl", "cr", or "crlf". Byte payloads accept
 * separate byte numbers or a byte-list string like "0x01 2 3".
 */


/*
 * Art-Net and sACN DMX functions (call them on local):
 *   local.setChannel(channel, value)
 *   local.sendFrame("[0, 127, 255]")
 *   local.blackout()
 *
 * Channels are one-based (1..512), levels are 0..255, and sendFrame accepts a
 * JSON array containing at most 512 channel values.
 */

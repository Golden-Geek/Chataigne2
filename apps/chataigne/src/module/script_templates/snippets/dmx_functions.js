/*
 * Art-Net and sACN DMX functions (call them on local):
 *   local.setChannel(channel, value)
 *   local.setChannels(channel, { Bytes: [0, 127, 255] })
 *   local.sendFrame({ Bytes: [0, 127, 255] })
 *   local.sendFrame("[0, 127, 255]")
 *   local.blackout()
 *
 * Channels are one-based (1..512), levels are 0..255. setChannels updates a
 * contiguous range atomically in one frame; the range must fit in the universe.
 * sendFrame accepts up to 512 bytes, or an explicitly supplied JSON channel string.
 * Use the tagged Bytes form to distinguish a byte array from vector/color values.
 */

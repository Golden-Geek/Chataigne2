/*
 * MIDI module functions (call them on local):
 *   local.sendNoteOn(channel, note, velocity = 127)
 *   local.sendNoteOff(channel, note, velocity = 0)
 *   local.sendFullNote(channel, note, velocity = 127, durationMs = 100, offVelocity = 0)
 *   local.sendCC(channel, controller, value)
 *   local.sendControlChange(channel, controller, value)
 *   local.sendProgramChange(channel, program)
 *   local.sendPitchBend(channel, value)
 *   local.sendChannelPressure(channel, pressure)
 *   local.sendPolyPressure(channel, note, pressure)
 *   local.sendSysEx(...bytes)
 *   local.sendSysex(...bytes)
 *   local.sendRawBytes(...bytes)
 *
 * Channels are 1-16. Notes, velocities, controllers, programs, and pressure values
 * are clamped to 0-127. Pitch bend is clamped to 0-16383, centered at 8192.
 * Byte payloads accept separate byte numbers or a byte-list string like
 * "0xF0 0x7D 0x01 0xF7".
 */


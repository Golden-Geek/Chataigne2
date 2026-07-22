/*
 * Buttplug module functions (call them on local):
 *   local.startScanning()
 *   local.stopScanning()
 *   local.stopAllDevices()
 *   local.stopAll()
 *   local.stopDevice(device = "selected")
 *   local.setOutput(output, value, device = "selected", durationMs = 1000)
 *   local.vibrate(value, device = "selected")
 *   local.rotate(value, device = "selected")
 *   local.oscillate(value, device = "selected")
 *   local.position(value, device = "selected")
 *   local.positionWithDuration(value, durationMs = 1000, device = "selected")
 *
 * Device can be "selected", "all", a Buttplug device index, or a device name.
 * Output values are normalized 0.0-1.0 and are clamped by Parameters / Max Output.
 */

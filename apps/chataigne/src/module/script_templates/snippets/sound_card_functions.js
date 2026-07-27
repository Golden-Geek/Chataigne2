/*
 * Sound Card module functions (call them on local):
 *   local.playFile(path, playbackId)
 *   local.stopFile(playbackId)
 *   local.stopAllFiles()
 *   local.setMasterVolume(volumeDb)
 *   local.setChannelVolume(channel, volumeDb)
 *
 * `playbackId` is a non-empty lane identifier. Playing a new file on the same
 * lane replaces its loading or active voice without affecting other lanes.
 * `channel` accepts an output-channel node handle or that node's stable UUID.
 * Volumes are expressed in dB from -120 through +24.
 */

/*
 * Sound Card module functions (call them on local):
 *   local.playFile(path, playbackId)
 *   local.playFile(path, playbackId, startOffsetSeconds, forceRestart)
 *   local.stopFile(playbackId)
 *   local.stopAllFiles()
 *   local.setMasterVolume(volumeDb)
 *   local.setChannelVolume(channel, volumeDb)
 *
 * `playbackId` is a non-empty lane identifier. Playing a new file on the same
 * lane replaces its loading or active voice without affecting other lanes.
 * `startOffsetSeconds` is optional and defaults to 0. `forceRestart` is optional
 * and defaults to true; when false, an occupied playback lane is left unchanged.
 * `channel` accepts an output-channel node handle or that node's stable UUID.
 * Volumes are expressed in dB from -120 through +24.
 */

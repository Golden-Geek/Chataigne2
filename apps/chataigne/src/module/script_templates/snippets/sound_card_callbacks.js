/*
 * Sound Card module callbacks:
 *   playbackStarted(playbackId, path, info)
 *   playbackFinished(playbackId, info)
 *   playbackStopped(playbackId, reason, info)
 *   playbackFailed(playbackId, path, error)
 *   audioDeviceStatusChanged(direction, status)
 *   audioBackendStatusChanged(backend, status)
 *
 * Playback callbacks are transient lifecycle notifications: they are delivered
 * once by the live engine and are never replayed after reconnect or resync.
 */

function playbackStarted(playbackId, path, info) {
  void playbackId;
  void path;
  void info;
}

function playbackFinished(playbackId, info) {
  void playbackId;
  void info;
}

function playbackStopped(playbackId, reason, info) {
  void playbackId;
  void reason;
  void info;
}

function playbackFailed(playbackId, path, error) {
  void playbackId;
  void path;
  void error;
}

function audioDeviceStatusChanged(direction, status) {
  void direction;
  void status;
}

function audioBackendStatusChanged(backend, status) {
  void backend;
  void status;
}

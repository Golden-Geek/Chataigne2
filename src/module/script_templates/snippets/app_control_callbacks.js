/*
 * App Control module callbacks:
 *   watchFolderChanged(folder, change)
 *   appControlCommandRequested(command, details)
 *   appControlCommandFailed(command, error)
 *
 * `folder` is the watched-folder item node handle.
 * `change` includes path, exists, entryCount, created, modified, removed, and timestampMs.
 */

function watchFolderChanged(folder, change) {
  void folder;
  void change;
}

function appControlCommandRequested(command, details) {
  void command;
  void details;
}

function appControlCommandFailed(command, error) {
  void command;
  void error;
}
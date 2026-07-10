/*
 * OS module callbacks:
 *   systemStatsUpdated(stats)
 *   systemCommandRequested(command, details)
 *   systemCommandFailed(command, error)
 *
 * `stats` groups the current host info, CPU, memory, network, and uptime values.
 */

function systemStatsUpdated(stats) {
  void stats;
}

function systemCommandRequested(command, details) {
  void command;
  void details;
}

function systemCommandFailed(command, error) {
  void command;
  void error;
}
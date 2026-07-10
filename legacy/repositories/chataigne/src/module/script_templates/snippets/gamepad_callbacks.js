/*
 * Gamepad module callbacks:
 *   gamepadConnected(gamepad)
 *   gamepadDisconnected(gamepad)
 *   gamepadAxisChanged(axis, value, rawValue, gamepad)
 *   gamepadButtonPressed(button, value, gamepad)
 *   gamepadButtonReleased(button, value, gamepad)
 *   gamepadButtonChanged(button, value, pressed, gamepad)
 *
 * Axis and button names match the Gamepad module Values tree declaration ids.
 * Axis values are processed by the per-axis Dead Zone and Offset parameters.
 */

function gamepadConnected(gamepad) {
  void gamepad;
}

function gamepadDisconnected(gamepad) {
  void gamepad;
}

function gamepadAxisChanged(axis, value, rawValue, gamepad) {
  void axis;
  void value;
  void rawValue;
  void gamepad;
}

function gamepadButtonPressed(button, value, gamepad) {
  void button;
  void value;
  void gamepad;
}

function gamepadButtonReleased(button, value, gamepad) {
  void button;
  void value;
  void gamepad;
}

function gamepadButtonChanged(button, value, pressed, gamepad) {
  void button;
  void value;
  void pressed;
  void gamepad;
}

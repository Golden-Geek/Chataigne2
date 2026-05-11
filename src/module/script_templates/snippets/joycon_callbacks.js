/*
 * Joy-Con module callbacks:
 *   joyConConnected(side, joyCon)
 *   joyConDisconnected(side, joyCon)
 *   joyConButtonPressed(side, button, joyCon)
 *   joyConButtonReleased(side, button, joyCon)
 *   joyConButtonChanged(side, button, pressed, joyCon)
 *   joyConStickChanged(side, stick, joyCon)
 *   joyConMotionChanged(side, motion, joyCon)
 *
 * `side` is "left" or "right".
 * Button names match the Values tree leaf ids within each controller's Buttons folder.
 * Stick values are post-dead-zone values.
 * Motion callbacks only fire when Motion Data is enabled.
 */

function joyConConnected(side, joyCon) {
  void side;
  void joyCon;
}

function joyConDisconnected(side, joyCon) {
  void side;
  void joyCon;
}

function joyConButtonPressed(side, button, joyCon) {
  void side;
  void button;
  void joyCon;
}

function joyConButtonReleased(side, button, joyCon) {
  void side;
  void button;
  void joyCon;
}

function joyConButtonChanged(side, button, pressed, joyCon) {
  void side;
  void button;
  void pressed;
  void joyCon;
}

function joyConStickChanged(side, stick, joyCon) {
  void side;
  void stick;
  void joyCon;
}

function joyConMotionChanged(side, motion, joyCon) {
  void side;
  void motion;
  void joyCon;
}
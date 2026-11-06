import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from window_capture import validate_foreground


class NativeWindows:
    foreground = 20
    dialog_pid = 30
    dialog_class = "#32770"

    def GetForegroundWindow(self):
        return self.foreground

    def GetWindowThreadProcessId(self, hwnd, target):
        target._obj.value = self.dialog_pid
        return 1

    def GetClassNameW(self, hwnd, target, capacity):
        target.value = self.dialog_class
        return len(target.value)


class DialogCaptureTests(unittest.TestCase):
    def test_default_capture_never_accepts_an_auxiliary_foreground(self):
        with self.assertRaisesRegex(AssertionError, "foreground"):
            validate_foreground(NativeWindows(), 10, 30)

    def test_exact_owned_dialog_is_admitted(self):
        validate_foreground(NativeWindows(), 10, 30, 20)

    def test_different_process_is_rejected(self):
        windows = NativeWindows()
        windows.dialog_pid = 99
        with self.assertRaisesRegex(AssertionError, "fixture PID"):
            validate_foreground(windows, 10, 30, 20)

    def test_other_window_class_and_lost_foreground_are_rejected(self):
        windows = NativeWindows()
        windows.dialog_class = "OtherWindow"
        with self.assertRaisesRegex(AssertionError, "native dialog"):
            validate_foreground(windows, 10, 30, 20)
        windows.dialog_class = "#32770"
        windows.foreground = 40
        with self.assertRaisesRegex(AssertionError, "foreground"):
            validate_foreground(windows, 10, 30, 20)


if __name__ == "__main__":
    unittest.main()

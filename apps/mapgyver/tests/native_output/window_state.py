"""Read-only HWND/physical-client/pixel observations for one owned output fixture PID."""
import ctypes
from ctypes import wintypes
import argparse
import json

from PIL import ImageGrab


def snapshot(pid, main=False, output_title=None, require_output_owner=False):
    user = ctypes.windll.user32
    user.SetThreadDpiAwarenessContext.argtypes = [ctypes.c_void_p]
    user.SetThreadDpiAwarenessContext(ctypes.c_void_p(-4))
    user.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
    user.GetClassNameW.argtypes = [wintypes.HWND, wintypes.LPWSTR, ctypes.c_int]
    user.GetWindowTextW.argtypes = [wintypes.HWND, wintypes.LPWSTR, ctypes.c_int]
    user.GetClientRect.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.RECT)]
    user.IsWindowVisible.argtypes = [wintypes.HWND]
    user.IsIconic.argtypes = [wintypes.HWND]
    user.ClientToScreen.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.POINT)]
    user.WindowFromPoint.argtypes = [wintypes.POINT]
    user.WindowFromPoint.restype = wintypes.HWND
    result = []
    callback_type = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)

    def visit(hwnd, _):
        owner = wintypes.DWORD()
        user.GetWindowThreadProcessId(hwnd, ctypes.byref(owner))
        if owner.value != pid:
            return True
        name = ctypes.create_unicode_buffer(256)
        user.GetClassNameW(hwnd, name, len(name))
        title = ctypes.create_unicode_buffer(512)
        user.GetWindowTextW(hwnd, title, len(title))
        output = title.value == output_title if output_title else name.value == "GoldenNativeOutputWindow"
        if (not main and not output) or (main and (output or not title.value.startswith("MapGyver"))):
            return True
        rect = wintypes.RECT()
        origin = wintypes.POINT()
        assert user.GetClientRect(hwnd, ctypes.byref(rect))
        assert user.ClientToScreen(hwnd, ctypes.byref(origin))
        width, height = rect.right - rect.left, rect.bottom - rect.top
        x, y = origin.x + width // 2, origin.y + height // 2
        visible = bool(user.IsWindowVisible(hwnd))
        minimized = bool(user.IsIconic(hwnd))
        center = wintypes.POINT(x, y)
        sample_owner = user.WindowFromPoint(center) if require_output_owner else None
        # Actual composited desktop pixels, including native output absent in WebView captures.
        image = None
        if not main and (not require_output_owner or (visible and not minimized and sample_owner == hwnd)):
            image = ImageGrab.grab(
                bbox=(x - 2, y - 2, x + 3, y + 3),
                all_screens=True,
                include_layered_windows=require_output_owner,
            )
            if require_output_owner:
                sample_owner = user.WindowFromPoint(center)
                if sample_owner != hwnd:
                    image = None
        window = {
            "hwnd": hwnd,
            "title": title.value,
            "bounds": {"left": origin.x, "top": origin.y,
                       "extent": {"width": width, "height": height}},
            "visible": visible,
            "minimized": minimized,
            "center_rgb": sorted(set(image.convert("RGB").get_flattened_data())) if image else None,
        }
        if require_output_owner:
            window["sample_owner_hwnd"] = sample_owner
        result.append(window)
        return True

    callback = callback_type(visit)
    user.EnumWindows.argtypes = [callback_type, wintypes.LPARAM]
    assert user.EnumWindows(callback, 0)
    return result


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("pid", type=int)
    parser.add_argument("--main", action="store_true")
    parser.add_argument("--title")
    parser.add_argument("--require-output-owner", action="store_true")
    args = parser.parse_args()
    print(json.dumps(snapshot(args.pid, args.main, args.title, args.require_output_owner)))

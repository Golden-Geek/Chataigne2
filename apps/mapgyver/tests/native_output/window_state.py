"""Read-only HWND/physical-client/pixel observations for one owned output fixture PID."""
import ctypes
from ctypes import wintypes
import json
import sys

from PIL import ImageGrab


def snapshot(pid):
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
    result = []
    callback_type = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)

    def visit(hwnd, _):
        owner = wintypes.DWORD()
        user.GetWindowThreadProcessId(hwnd, ctypes.byref(owner))
        if owner.value != pid:
            return True
        name = ctypes.create_unicode_buffer(256)
        user.GetClassNameW(hwnd, name, len(name))
        if name.value != "GoldenNativeOutputWindow":
            return True
        title = ctypes.create_unicode_buffer(512)
        user.GetWindowTextW(hwnd, title, len(title))
        rect = wintypes.RECT()
        origin = wintypes.POINT()
        assert user.GetClientRect(hwnd, ctypes.byref(rect))
        assert user.ClientToScreen(hwnd, ctypes.byref(origin))
        width, height = rect.right - rect.left, rect.bottom - rect.top
        x, y = origin.x + width // 2, origin.y + height // 2
        # Actual composited desktop pixels, including native output absent in WebView captures.
        image = ImageGrab.grab(bbox=(x - 2, y - 2, x + 3, y + 3), all_screens=True)
        result.append({
            "hwnd": hwnd,
            "title": title.value,
            "bounds": {"left": origin.x, "top": origin.y,
                       "extent": {"width": width, "height": height}},
            "visible": bool(user.IsWindowVisible(hwnd)),
            "minimized": bool(user.IsIconic(hwnd)),
            "center_rgb": sorted(set(image.convert("RGB").get_flattened_data())),
        })
        return True

    callback = callback_type(visit)
    user.EnumWindows.argtypes = [callback_type, wintypes.LPARAM]
    assert user.EnumWindows(callback, 0)
    return result


if __name__ == "__main__":
    print(json.dumps(snapshot(int(sys.argv[1]))))

"""Sample the owned native output on the fixture's 24-bit TrueColor X11 display."""

import ctypes as c
import json
import sys

x11 = c.CDLL("libX11.so.6")


def bind(name, result, *arguments):
    function = getattr(x11, name)
    function.restype = result
    function.argtypes = arguments
    return function


open_display = bind("XOpenDisplay", c.c_void_p, c.c_char_p)
close_display = bind("XCloseDisplay", c.c_int, c.c_void_p)
get_image = bind(
    "XGetImage",
    c.c_void_p,
    c.c_void_p,
    c.c_ulong,
    c.c_int,
    c.c_int,
    c.c_uint,
    c.c_uint,
    c.c_ulong,
    c.c_int,
)
get_pixel = bind("XGetPixel", c.c_ulong, c.c_void_p, c.c_int, c.c_int)
destroy_image = bind("XDestroyImage", c.c_int, c.c_void_p)

display = open_display(None)
assert display, "Could not open the fixture X11 display"
try:
    window, width, height = (int(value, 0) for value in sys.argv[1:])
    image = get_image(display, window, width // 2 - 4, height // 2 - 4, 8, 8, -1, 2)
    assert image, "Could not sample the fixture X11 window"
    try:
        pixels = [get_pixel(image, x, y) for y in range(8) for x in range(8)]
        print(json.dumps([[(pixel >> shift) & 255 for shift in (16, 8, 0)] for pixel in pixels]))
    finally:
        destroy_image(image)
finally:
    close_display(display)

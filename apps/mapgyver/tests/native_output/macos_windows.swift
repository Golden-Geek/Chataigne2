import CoreGraphics
import Foundation

if CommandLine.arguments == [CommandLine.arguments[0], "--primary-display"] {
    let bounds = CGDisplayBounds(CGMainDisplayID())
    let rectangle = ["left": bounds.origin.x, "top": bounds.origin.y,
                     "width": bounds.width, "height": bounds.height]
    let data = try JSONSerialization.data(withJSONObject: rectangle)
    print(String(decoding: data, as: UTF8.self))
    exit(0)
}

guard CommandLine.arguments.count == 2, let pid = Int(CommandLine.arguments[1]), pid > 0 else {
    fputs("Specify the PID of the owned MapGyver fixture\n", stderr)
    exit(2)
}

guard let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID)
        as? [[String: Any]] else {
    fputs("The fixture is outside a WindowServer session\n", stderr)
    exit(1)
}

let owned = windows.compactMap { window -> [String: Any]? in
    guard (window[kCGWindowOwnerPID as String] as? NSNumber)?.intValue == pid,
          let identity = window[kCGWindowNumber as String] as? NSNumber,
          let layer = window[kCGWindowLayer as String] as? NSNumber,
          let bounds = window[kCGWindowBounds as String] as? [String: NSNumber],
          let left = bounds["X"], let top = bounds["Y"],
          let width = bounds["Width"], let height = bounds["Height"] else {
        return nil
    }
    return ["id": identity, "layer": layer, "left": left, "top": top, "width": width, "height": height]
}

let data = try JSONSerialization.data(withJSONObject: owned, options: [.sortedKeys])
print(String(decoding: data, as: UTF8.self))

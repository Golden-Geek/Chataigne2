import CoreGraphics
import Foundation
import ImageIO

if CommandLine.arguments.count == 3, CommandLine.arguments[1] == "--sample-center" {
    let url = URL(fileURLWithPath: CommandLine.arguments[2])
    guard let source = CGImageSourceCreateWithURL(url as CFURL, nil),
          let image = CGImageSourceCreateImageAtIndex(source, 0, nil),
          image.width >= 8, image.height >= 8,
          let center = image.cropping(to: CGRect(x: image.width / 2 - 4, y: image.height / 2 - 4,
                                                 width: 8, height: 8)),
          let colorSpace = CGColorSpace(name: CGColorSpace.sRGB) else {
        fputs("Could not read the output window's center pixels\n", stderr)
        exit(1)
    }
    var rgba = [UInt8](repeating: 0, count: 4)
    let sampled = rgba.withUnsafeMutableBytes { bytes -> Bool in
        guard let context = CGContext(data: bytes.baseAddress, width: 1, height: 1,
                                      bitsPerComponent: 8, bytesPerRow: 4, space: colorSpace,
                                      bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return false }
        context.interpolationQuality = .high
        context.draw(center, in: CGRect(x: 0, y: 0, width: 1, height: 1))
        return true
    }
    guard sampled, rgba[3] >= 250 else {
        fputs("The captured output window has no opaque center pixel\n", stderr)
        exit(1)
    }
    let data = try JSONSerialization.data(withJSONObject: rgba.prefix(3).map(Int.init))
    print(String(decoding: data, as: UTF8.self))
    exit(0)
}

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

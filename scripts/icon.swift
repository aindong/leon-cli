// icon.swift OUT.iconset: draw leon's app icon at every size iconutil wants.
// A dark tile holding three amber panes (one tall, two stacked), the tall one with a prompt
// chevron and cursor. Each size is drawn from vectors; 16 and 32 px skip the chevron so the panes
// stay crisp.
import AppKit

let out = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "Leon.iconset"
try FileManager.default.createDirectory(atPath: out, withIntermediateDirectories: true)

func rgb(_ hex: UInt32) -> NSColor {
    NSColor(srgbRed: CGFloat(hex >> 16 & 0xff) / 255, green: CGFloat(hex >> 8 & 0xff) / 255,
            blue: CGFloat(hex & 0xff) / 255, alpha: 1)
}

func rounded(_ r: NSRect, _ radius: CGFloat) -> NSBezierPath {
    NSBezierPath(roundedRect: r, xRadius: radius, yRadius: radius)
}

/// Draws on a 1024-unit canvas, origin bottom left.
func draw(small: Bool) {
    // the tile, with Apple's icon-grid margin
    let tile = NSRect(x: 100, y: 100, width: 824, height: 824)
    NSGradient(starting: rgb(0x262B3B), ending: rgb(0x10121A))!.draw(in: rounded(tile, 185), angle: -90)
    rgb(0x3A4057).setStroke()
    let edge = rounded(tile.insetBy(dx: 4, dy: 4), 181)
    edge.lineWidth = 8
    edge.stroke()

    // panes: wider gaps when small so they survive the downscale
    let gap: CGFloat = small ? 56 : 40
    let inner = tile.insetBy(dx: small ? 120 : 112, dy: small ? 120 : 112)
    let leftW = (inner.width - gap) * 0.55
    let rightW = inner.width - gap - leftW
    let halfH = (inner.height - gap) / 2
    let radius: CGFloat = small ? 36 : 44
    let left = NSRect(x: inner.minX, y: inner.minY, width: leftW, height: inner.height)
    let rightX = inner.minX + leftW + gap
    let top = NSRect(x: rightX, y: inner.minY + halfH + gap, width: rightW, height: halfH)
    let bottom = NSRect(x: rightX, y: inner.minY, width: rightW, height: halfH)
    rgb(0xF6B93B).setFill()
    rounded(left, radius).fill()
    rgb(0xB9832A).setFill()
    rounded(top, radius).fill()
    rounded(bottom, radius).fill()

    if !small {
        // a prompt in the tall pane: ">" and a cursor bar
        rgb(0x141720).setStroke()
        let chevron = NSBezierPath()
        let cx = left.minX + 84, cy = left.maxY - 150
        chevron.move(to: NSPoint(x: cx, y: cy + 64))
        chevron.line(to: NSPoint(x: cx + 76, y: cy))
        chevron.line(to: NSPoint(x: cx, y: cy - 64))
        chevron.lineWidth = 40
        chevron.lineCapStyle = .round
        chevron.lineJoinStyle = .round
        chevron.stroke()
        rgb(0x141720).setFill()
        rounded(NSRect(x: cx + 112, y: cy - 66, width: 72, height: 34), 12).fill()
    }
}

let sizes: [(String, Int)] = [
    ("16x16", 16), ("16x16@2x", 32), ("32x32", 32), ("32x32@2x", 64), ("128x128", 128),
    ("128x128@2x", 256), ("256x256", 256), ("256x256@2x", 512), ("512x512", 512), ("512x512@2x", 1024),
]
for (name, px) in sizes {
    let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: px, pixelsHigh: px, bitsPerSample: 8,
                               samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
                               colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
    let scale = CGFloat(px) / 1024
    let t = NSAffineTransform()
    t.scale(by: scale)
    t.concat()
    draw(small: px <= 32)
    NSGraphicsContext.restoreGraphicsState()
    try rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: "\(out)/icon_\(name).png"))
}

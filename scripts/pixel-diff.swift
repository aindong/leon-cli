// pixel-diff <before-dir> <after-dir> [name:x,y,w,h ...]: compare every PNG in before-dir with the
// one of the same name in after-dir, pixel by pixel. Boxes (in pixels) name where a picture may
// differ (an animation caught at another frame). Prints each picture's differing pixels and their
// bounding box (PIXEL_DIFF_TILES=1 lists them as 32-pixel tiles); exits 1 when any differ outside the allowed boxes, 2 on a missing or unreadable file.
import AppKit

let args = CommandLine.arguments
guard args.count >= 3 else { print("usage: pixel-diff <before-dir> <after-dir> [name:x,y,w,h ...]"); exit(2) }
var allowed: [String: [CGRect]] = [:]
for a in args.dropFirst(3) {
    let parts = a.split(separator: ":", maxSplits: 1)
    let n = parts.count == 2 ? parts[1].split(separator: ",").compactMap { Double($0) } : []
    guard n.count == 4 else { print("bad box \(a): use name:x,y,w,h"); exit(2) }
    allowed[String(parts[0]), default: []].append(CGRect(x: n[0], y: n[1], width: n[2], height: n[3]))
}

func pixels(_ path: String) -> (Int, Int, [UInt8])? {
    guard let img = NSImage(contentsOfFile: path),
          let cg = img.cgImage(forProposedRect: nil, context: nil, hints: nil) else { return nil }
    let (w, h) = (cg.width, cg.height)
    var buf = [UInt8](repeating: 0, count: w * h * 4)
    guard let ctx = CGContext(data: &buf, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                              space: CGColorSpaceCreateDeviceRGB(),
                              bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return nil }
    ctx.draw(cg, in: CGRect(x: 0, y: 0, width: w, height: h))
    return (w, h, buf)
}

let names = (try? FileManager.default.contentsOfDirectory(atPath: args[1]))?.filter { $0.hasSuffix(".png") }.sorted() ?? []
guard !names.isEmpty else { print("no PNGs in \(args[1])"); exit(2) }
var failed = false
for name in names {
    guard let (w, h, a) = pixels("\(args[1])/\(name)"), let (w2, h2, b) = pixels("\(args[2])/\(name)") else {
        print("\(name): missing or unreadable in one folder"); exit(2)
    }
    guard w == w2 && h == h2 else { print("\(name): size \(w)x\(h) became \(w2)x\(h2)"); failed = true; continue }
    let boxes = allowed[String(name.dropLast(4))] ?? []
    var (n, outside) = (0, 0)
    var (x0, y0, x1, y1) = (w, h, -1, -1)
    var tiles = Set<String>()
    for y in 0..<h {
        for x in 0..<w {
            let i = (y * w + x) * 4
            if a[i] == b[i] && a[i + 1] == b[i + 1] && a[i + 2] == b[i + 2] { continue }
            n += 1
            (x0, y0, x1, y1) = (min(x0, x), min(y0, y), max(x1, x), max(y1, y))
            tiles.insert("\(x / 32 * 32),\(y / 32 * 32)")
            // the context draws bottom-up: row 0 of the buffer is the picture's top
            if !boxes.contains(where: { $0.contains(CGPoint(x: x, y: y)) }) { outside += 1 }
        }
    }
    let box = n == 0 ? "" : " in x \(x0)...\(x1), y \(y0)...\(y1)"
    print("\(name): \(n) pixels differ\(box)\(outside > 0 && outside < n ? ", \(outside) outside the allowed boxes" : "")")
    if outside > 0 { failed = true }
    // PIXEL_DIFF_TILES=1: where they are, as 32-pixel tiles (x,y of each tile's corner)
    if ProcessInfo.processInfo.environment["PIXEL_DIFF_TILES"] != nil && n > 0 { print("  tiles: \(tiles.sorted().joined(separator: " "))") }
}
exit(failed ? 1 : 0)

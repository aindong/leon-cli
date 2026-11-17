// deskshot: leon's translucent window as the screen draws it, over a neutral desktop of our own, so
// a picture shows what is behind the window without showing anything of this machine.
//   deskshot backdrop              cover the main screen with a plain gradient window (until killed)
//   deskshot PID BACKDROP out [PAD] capture PID's frontmost window, PAD points around it, from the
//                                  screen; only when every window in front of the backdrop there is
//                                  PID's and the backdrop (pid BACKDROP) covers it all (else exit 1)
// Build it with swiftc and run the binary: the backdrop's windows must be owned by the pid you have.
import AppKit

let args = CommandLine.arguments

if args.count == 2 && args[1] == "backdrop" {
    // top to bottom from slate blue to muted violet, one flat colour a row (a dithered gradient
    // costs megabytes of PNG)
    final class Desk: NSView {
        override func draw(_ dirty: NSRect) {
            let (top, bottom): ([CGFloat], [CGFloat]) = ([0.18, 0.30, 0.40], [0.42, 0.32, 0.48])
            let h = Int(bounds.height)
            for y in 0..<h {
                let t = CGFloat(y) / CGFloat(max(h - 1, 1))
                let c = (0..<3).map { bottom[$0] + (top[$0] - bottom[$0]) * t }
                NSColor(srgbRed: c[0], green: c[1], blue: c[2], alpha: 1).setFill()
                NSRect(x: 0, y: CGFloat(y), width: bounds.width, height: 1).fill()
            }
        }
    }
    let app = NSApplication.shared
    app.setActivationPolicy(.accessory)
    let frame = NSScreen.screens[0].frame
    let window = NSWindow(contentRect: frame, styleMask: .borderless, backing: .buffered, defer: false)
    window.contentView = Desk(frame: frame)
    window.orderFrontRegardless()
    app.run()
}

guard args.count == 4 || args.count == 5, let pid = Int(args[1]), let backdrop = Int(args[2]),
      let pad = Double(args.count == 5 ? args[4] : "0")
else { print("usage: deskshot backdrop | deskshot PID BACKDROP_PID out.png [PAD]"); exit(2) }
let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as! [[String: Any]] // front to back
let owner = { (w: [String: Any]) in w[kCGWindowOwnerPID as String] as? Int }
let bounds = { (w: [String: Any]) in CGRect(dictionaryRepresentation: w[kCGWindowBounds as String] as! CFDictionary)! }
guard let win = list.first(where: { owner($0) == pid && ($0[kCGWindowLayer as String] as? Int) == 0 })
else { print("deskshot: no window for pid \(pid)"); exit(1) }
let rect = bounds(win).insetBy(dx: -pad, dy: -pad).integral
guard let back = list.firstIndex(where: { owner($0) == backdrop && bounds($0).contains(rect) })
else { print("deskshot: the backdrop doesn't cover \(rect)"); exit(1) }
// the Dock keeps a clear window over the whole screen and draws only the Dock in it, outside the
// visible frame; a shot inside that frame (menu bar and Dock excluded) can't show the Dock
let screen = NSScreen.screens[0], v = screen.visibleFrame
let visible = CGRect(x: v.minX, y: screen.frame.height - v.maxY, width: v.width, height: v.height) // top-left origin
guard visible.contains(rect) else { print("deskshot: \(rect) isn't inside the screen's visible frame"); exit(1) }
let dock = { (w: [String: Any]) in (w[kCGWindowOwnerName as String] as? String) == "Dock" }
for w in list[..<back] where bounds(w).intersects(rect) && owner(w) != pid && !dock(w) {
    print("deskshot: another program's window is over the shot (pid \(owner(w) ?? -1)); nothing captured")
    exit(1)
}
let r = "\(Int(rect.minX)),\(Int(rect.minY)),\(Int(rect.width)),\(Int(rect.height))"
let p = Process()
p.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
p.arguments = ["-x", "-R\(r)", args[3]]
try! p.run()
p.waitUntilExit()
exit(p.terminationStatus)

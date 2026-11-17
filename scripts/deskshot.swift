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
let owner = { (w: [String: Any]) in w[kCGWindowOwnerPID as String] as? Int }
let bounds = { (w: [String: Any]) in CGRect(dictionaryRepresentation: w[kCGWindowBounds as String] as! CFDictionary)! }
let screen = NSScreen.screens[0], v = screen.visibleFrame
let visible = CGRect(x: v.minX, y: screen.frame.height - v.maxY, width: v.width, height: v.height) // top-left origin
// the Dock keeps a clear window over the whole screen and draws the Dock in it only outside the
// visible frame; an auto-hiding Dock can slide into that frame, so then nothing is exempt
let autohide = CFPreferencesCopyAppValue("autohide" as CFString, "com.apple.dock" as CFString) as? Bool ?? false
let clearDock = { (w: [String: Any]) in
    !autohide && (w[kCGWindowOwnerName as String] as? String) == "Dock" && bounds(w) == CGRect(origin: .zero, size: screen.frame.size)
}

/// The rect to capture, when only PID's windows are in front of the backdrop there; else nil, said why.
func check() -> CGRect? {
    let no = { (why: String) -> CGRect? in print("deskshot: \(why); nothing captured"); return nil }
    let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as! [[String: Any]] // front to back
    guard let at = list.firstIndex(where: { owner($0) == pid && ($0[kCGWindowLayer as String] as? Int) == 0 })
    else { return no("no window for pid \(pid)") }
    let rect = bounds(list[at]).insetBy(dx: -pad, dy: -pad).integral
    guard visible.contains(rect) else { return no("\(rect) isn't inside the screen's visible frame") }
    guard let back = list.firstIndex(where: { owner($0) == backdrop && bounds($0).contains(rect) })
    else { return no("the backdrop doesn't cover \(rect)") }
    guard at < back else { return no("leon's window is behind the backdrop") }
    if let w = list[..<back].first(where: { bounds($0).intersects(rect) && owner($0) != pid && !clearDock($0) }) {
        return no("another program's window is over the shot (pid \(owner(w) ?? -1))")
    }
    return rect
}

guard let rect = check() else { exit(1) }
let r = "\(Int(rect.minX)),\(Int(rect.minY)),\(Int(rect.width)),\(Int(rect.height))"
let p = Process()
p.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
p.arguments = ["-x", "-R\(r)", args[3]]
try! p.run()
p.waitUntilExit()
// a window that came up while screencapture ran (a notification banner) fails the check again
guard check() == rect else {
    try? FileManager.default.removeItem(atPath: args[3])
    print("deskshot: the screen changed during the capture; nothing kept")
    exit(1)
}
exit(p.terminationStatus)

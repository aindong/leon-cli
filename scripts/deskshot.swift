// deskshot: leon's translucent window as the screen draws it, over a neutral desktop of our own, so
// a picture shows what is behind the window without showing anything of this machine.
//   deskshot backdrop              cover the main screen with a plain gradient window (until killed)
//   deskshot PID BACKDROP out [PAD] capture PID's frontmost window, PAD points around it, from the
//                                  screen; only when every window in front of the backdrop there is
//                                  PID's and the backdrop (pid BACKDROP) covers it all (else exit 1)
//   deskshot window PCT            print "x y w h": PCT % of the main screen's visible frame, centred,
//                                  in points from the top left (for leon's app.json)
//   deskshot record PID BACKDROP out.mov [PAD]
//                                  the same rect as a 30 fps video, no cursor, until SIGTERM or
//                                  SIGINT; the check runs on every frame, and a take any frame of
//                                  which fails it is deleted (exit 1)
// Build it with swiftc and run the binary: the backdrop's windows must be owned by the pid you have.
import AppKit
import AVFoundation
import ScreenCaptureKit

let record = CommandLine.arguments.count > 1 && CommandLine.arguments[1] == "record"
let args = record ? Array(CommandLine.arguments.dropFirst()) : CommandLine.arguments

if args.count == 3 && args[1] == "window", let pct = Double(args[2]) {
    let s = NSScreen.screens[0], v = s.visibleFrame
    let (w, h) = ((v.width * pct / 100).rounded(), (v.height * pct / 100).rounded())
    let top = s.frame.height - v.maxY // the visible frame's top, from the top left
    print(Int(v.minX + (v.width - w) / 2), Int(top + (v.height - h) / 2), Int(w), Int(h))
    exit(0)
}

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
else { print("usage: deskshot backdrop | deskshot [record] PID BACKDROP_PID out.png|out.mov [PAD]"); exit(2) }
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
        return no("another program's window is over the shot (pid \(owner(w) ?? -1), \(w[kCGWindowOwnerName as String] ?? "?") \(w[kCGWindowName as String] ?? "") \(bounds(w)))")
    }
    return rect
}

guard let rect = check() else { exit(1) }
// record: the screen in `rect`, as a still captures it but 30 times a second, until SIGTERM or SIGINT
if record {
    let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
    guard let display = content.displays.first(where: { $0.displayID == CGMainDisplayID() })
    else { print("deskshot: no main display"); exit(1) }
    let filter = SCContentFilter(display: display, excludingWindows: [])
    let config = SCStreamConfiguration()
    let scale = Double(filter.pointPixelScale)
    config.sourceRect = rect
    config.width = Int(rect.width * scale)
    config.height = Int(rect.height * scale)
    config.minimumFrameInterval = CMTime(value: 1, timescale: 30)
    config.showsCursor = false
    config.queueDepth = 6
    let url = URL(fileURLWithPath: args[3])
    let out = try Recorder(url, rect, config.width, config.height, check)
    let queue = DispatchQueue(label: "deskshot.record")
    let stream = SCStream(filter: filter, configuration: config, delegate: nil)
    try stream.addStreamOutput(out, type: .screen, sampleHandlerQueue: queue)
    try await stream.startCapture()
    print("deskshot: recording \(rect)")
    var sources: [DispatchSourceSignal] = []
    for sig in [SIGTERM, SIGINT] {
        signal(sig, SIG_IGN)
        let s = DispatchSource.makeSignalSource(signal: sig, queue: queue)
        s.setEventHandler {
            Task {
                try? await stream.stopCapture()
                out.writer.endSession(atSourceTime: CMClockGetTime(CMClockGetHostTimeClock())) // the last frame lasts until now
                out.input.markAsFinished()
                await out.writer.finishWriting()
                if out.bad || !out.started || out.writer.status != .completed {
                    try? FileManager.default.removeItem(at: url)
                    print(out.bad ? "deskshot: another window came over the recording; nothing kept" : "deskshot: nothing recorded")
                    exit(1)
                }
                exit(0)
            }
        }
        s.resume()
        sources.append(s)
    }
    while true { try await Task.sleep(nanoseconds: 1_000_000_000) }
}

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

/// Writes the frames that changed; marks the take bad when a frame's screen fails `check()`.
final class Recorder: NSObject, SCStreamOutput {
    let writer: AVAssetWriter
    let input: AVAssetWriterInput
    let rect: CGRect
    let check: () -> CGRect?
    var started = false
    var bad = false
    init(_ url: URL, _ rect: CGRect, _ w: Int, _ h: Int, _ check: @escaping () -> CGRect?) throws {
        self.rect = rect
        self.check = check
        writer = try AVAssetWriter(outputURL: url, fileType: .mov)
        input = AVAssetWriterInput(mediaType: .video, outputSettings: [
            AVVideoCodecKey: AVVideoCodecType.h264, AVVideoWidthKey: w, AVVideoHeightKey: h,
            AVVideoCompressionPropertiesKey: [AVVideoAverageBitRateKey: 40_000_000],
        ])
        input.expectsMediaDataInRealTime = true
        writer.add(input)
        writer.startWriting()
    }
    func stream(_ stream: SCStream, didOutputSampleBuffer buffer: CMSampleBuffer, of type: SCStreamOutputType) {
        // an idle frame (nothing changed) carries no image
        guard type == .screen, buffer.isValid,
              let info = CMSampleBufferGetSampleAttachmentsArray(buffer, createIfNecessary: false) as? [[SCStreamFrameInfo: Any]],
              let raw = info.first?[.status] as? Int, SCFrameStatus(rawValue: raw) == .complete
        else { return }
        if check() != rect { bad = true }
        if bad { return }
        if !started { writer.startSession(atSourceTime: buffer.presentationTimeStamp); started = true }
        if input.isReadyForMoreMediaData { input.append(buffer) }
    }
}


// winframes: watch one app's window from outside, through the window server, the same way for
// any app.
//
//   winframes PID keys N [GAP_MS]   post N synthetic "x" keys to PID; per key, the time from posting
//                                   it to the first frame whose echo bar has that key's colour
//   winframes PID watch SECONDS [UNTIL] [floor=PID] [save=DIR]  every changed frame of the window, and how many
//                                   sentinel bars it shows (runs of truecolor 255,0,128); stops 0.5 s after
//                                   the first frame showing UNTIL bars; floor=PID also lists the frames of a
//                                   `floor animate` window (what the capture delivered meanwhile); save=DIR
//                                   writes each new frame there as DISPLAYTIME.png (a motion's frames)
//   winframes PID hide|unhide        hide the app as ⌘H does, or show it again
//   winframes floor [W H] [animate] a window of this tool's own that draws the echo bar the moment
//                                   a key arrives (run it, then `keys` its pid: the floor); with
//                                   `animate` it also changes every display frame, to measure
//                                   what the capture itself can deliver
//
// The echo bar (scripts/perf/echo.py) is red before the first key and flips red/green per key.
// A frame arrives (ScreenCaptureKit, status .complete) when the window was composited anew; the
// time used is its displayTime, when the window server composited it (before scan-out). Every
// time printed is the boot clock in nanoseconds (CLOCK_UPTIME_RAW), like LEON_FRAME_LOG's.
// Build: swiftc -O -o target/perf/winframes scripts/perf/winframes.swift
import AppKit
import CoreImage
import CoreMedia
import ScreenCaptureKit

setvbuf(stdout, nil, _IOLBF, 0)
let args = CommandLine.arguments
func fail(_ msg: String) -> Never {
  FileHandle.standardError.write("winframes: \(msg)\n".data(using: .utf8)!)
  exit(1)
}
if args.count == 3, ["hide", "unhide"].contains(args[2]), let p = pid_t(args[1]), let target = NSRunningApplication(processIdentifier: p) {
  _ = args[2] == "hide" ? target.hide() : target.unhide()  // what ⌘H does, from outside
  exit(0)
}
guard args.count >= 2, args[1] == "floor" || (args.count >= 4 && ["keys", "watch"].contains(args[2])) else {
  fail("usage: winframes PID keys N [GAP_MS] | PID watch SECONDS | floor [W H] [animate]")
}

var tb = mach_timebase_info_data_t()
mach_timebase_info(&tb)
func ns(_ mach: UInt64) -> UInt64 { mach * UInt64(tb.numer) / UInt64(tb.denom) }
func now() -> UInt64 { ns(mach_absolute_time()) }

enum Shade { case red, green, sentinel, other }
func shade(_ r: Int, _ g: Int, _ b: Int) -> Shade {
  // the sentinel, also as a pane faded toward a dark background draws it
  if r > 140 && g < 90 && b > 60 && b < 190 && r > b + 40 { return .sentinel }
  if r > 150 && g < 100 && b < 70 { return .red }
  if g > 150 && r < 110 && b < 130 { return .green }
  return .other
}

let app = NSApplication.shared
let debug = ProcessInfo.processInfo.environment["WINFRAMES_DEBUG"] == "1"

// MARK: floor

final class FloorView: NSView {
  var n = 0
  var tick = 0
  override var acceptsFirstResponder: Bool { true }
  override func keyDown(with event: NSEvent) { n += 1; display() }
  override func draw(_ r: NSRect) {
    NSColor(white: tick % 2 == 0 ? 0.1 : 0.12, alpha: 1).setFill()
    bounds.fill()
    (n % 2 == 0 ? NSColor(srgbRed: 1, green: 0, blue: 0, alpha: 1) : NSColor(srgbRed: 0, green: 1, blue: 0, alpha: 1)).setFill()
    NSRect(x: 4, y: bounds.height - 24, width: 100, height: 18).fill()
  }
}

if args[1] == "floor" {
  let w = Double(args.count > 3 ? args[2] : "1000") ?? 1000, h = Double(args.count > 3 ? args[3] : "680") ?? 680
  app.setActivationPolicy(.regular)
  let win = NSWindow(contentRect: NSRect(x: 120, y: 120, width: w, height: h), styleMask: [.titled], backing: .buffered, defer: false)
  let v = FloorView(frame: win.contentRect(forFrameRect: win.frame))
  win.contentView = v
  win.title = "winframes floor"
  win.makeKeyAndOrderFront(nil)
  win.makeFirstResponder(v)
  app.activate(ignoringOtherApps: true)
  if args.contains("animate") {
    Timer.scheduledTimer(withTimeInterval: 1.0 / Double(NSScreen.main?.maximumFramesPerSecond ?? 60), repeats: true) { _ in
      v.tick += 1; v.display()
    }
  }
  print("floor \(getpid())")
  app.run()
}

// MARK: watching

struct Seen { let at: UInt64; let bar: Shade; let sentinels: Int; let dirty: Bool }

final class Watcher: NSObject, SCStreamOutput {
  let lock = NSLock()
  var seen: [Seen] = []
  var probe: (Int, Int)?  // the echo bar's centre, in buffer pixels, once found
  var wantSentinel = false
  var saveDir: String?
  let images = CIContext()
  var kept: [(UInt64, CGImage)] = []  // save=DIR's frames, written once the watch is over
  var lastHash: UInt64 = 0
  func stream(_ s: SCStream, didOutputSampleBuffer sb: CMSampleBuffer, of type: SCStreamOutputType) {
    guard type == .screen,
      let info = (CMSampleBufferGetSampleAttachmentsArray(sb, createIfNecessary: false) as? [[SCStreamFrameInfo: Any]])?.first,
      let raw = info[.status] as? Int, SCFrameStatus(rawValue: raw) == .complete,
      let shown = info[.displayTime] as? UInt64,
      let px = CMSampleBufferGetImageBuffer(sb)
    else { return }
    CVPixelBufferLockBaseAddress(px, .readOnly)
    defer { CVPixelBufferUnlockBaseAddress(px, .readOnly) }
    let w = CVPixelBufferGetWidth(px), h = CVPixelBufferGetHeight(px), row = CVPixelBufferGetBytesPerRow(px)
    let base = CVPixelBufferGetBaseAddress(px)!.assumingMemoryBound(to: UInt8.self)
    func at(_ x: Int, _ y: Int) -> Shade {
      let p = base + y * row + x * 4  // BGRA
      return shade(Int(p[2]), Int(p[1]), Int(p[0]))
    }
    lock.lock(); defer { lock.unlock() }
    if probe == nil {
      // the echo bar: the first row (top down) with a long run of red; its run's middle
      outer: for y in stride(from: 0, to: h, by: 2) {
        var run = 0
        for x in 0..<w {
          run = at(x, y) == .red ? run + 1 : 0
          if run >= 40 { probe = (x - 20, y + 3); break outer }
        }
      }
    }
    // sentinel bars: runs of 150 or more, one per place (a bar is several pixel rows tall)
    var sentinels = 0
    if wantSentinel {
      var bars: [(x: Int, y: Int)] = []
      for y in stride(from: 0, to: h, by: 3) {
        var run = 0
        for x in 0..<w {
          run = at(x, y) == .sentinel ? run + 1 : 0
          if run == 150, !bars.contains(where: { abs($0.x - (x - 150)) < 40 && y - $0.y < 12 }) {
            bars.append((x - 150, y))
          } else if run == 150, let i = bars.lastIndex(where: { abs($0.x - (x - 150)) < 40 }) {
            bars[i].y = y  // the same bar, a row further down
          }
        }
      }
      sentinels = bars.count
    }
    let bar = probe.map { at($0.0, min($0.1, h - 1)) } ?? .other
    if debug, let xy = ProcessInfo.processInfo.environment["WINFRAMES_AT"]?.split(separator: ",").compactMap({ Int($0) }), xy.count == 2 {
      let p = base + xy[1] * row + xy[0] * 4
      FileHandle.standardError.write("at \(xy[0]),\(xy[1]) rgb \(p[2]),\(p[1]),\(p[0]) \(shade(Int(p[2]), Int(p[1]), Int(p[0])))\n".data(using: .utf8)!)
    }
    if debug, let (x, y) = probe {
      let p = base + y * row + x * 4
      FileHandle.standardError.write("frame \(w)x\(h) probe \(x),\(y) rgb \(p[2]),\(p[1]),\(p[0]) \(bar)\n".data(using: .utf8)!)
    }
    // the capture repeats a window's frame at the display rate, dirty rects and all: a frame is new
    // only when its pixels differ from the one before
    var hash: UInt64 = 1469598103934665603
    for y in 0..<h {
      let words = UnsafeRawPointer(base + y * row).assumingMemoryBound(to: UInt64.self)
      for i in 0..<(w * 4 / 8) { hash = (hash ^ words[i]) &* 1099511628211 }
    }
    let dirty = hash != lastHash
    lastHash = hash
    if dirty, saveDir != nil, let cg = images.createCGImage(CIImage(cvPixelBuffer: px), from: CGRect(x: 0, y: 0, width: w, height: h)) {
      kept.append((ns(shown), cg))
    }
    seen.append(Seen(at: ns(shown), bar: bar, sentinels: sentinels, dirty: dirty))
  }
  func snapshot() -> [Seen] { lock.lock(); defer { lock.unlock() }; return seen }
  /// Write save=DIR's frames as DISPLAYTIME.png, off the capture queue; a failed write says so.
  func writeKept() {
    guard let dir = saveDir else { return }
    lock.lock(); let frames = kept; lock.unlock()
    for (at, cg) in frames {
      let url = URL(fileURLWithPath: dir).appendingPathComponent("\(at).png")
      guard let png = NSBitmapImageRep(cgImage: cg).representation(using: .png, properties: [:]) else { continue }
      do { try png.write(to: url) } catch { FileHandle.standardError.write("winframes: \(url.path): \(error)\n".data(using: .utf8)!) }
    }
  }
  var found: Bool { lock.lock(); defer { lock.unlock() }; return probe != nil }
}

guard let pid = pid_t(args[1]) else { fail("bad pid \(args[1])") }
app.setActivationPolicy(.prohibited)

/// The largest on-screen window of `pid` (it can take a moment to appear after launch).
func window(of pid: pid_t) async -> SCWindow {
  for _ in 0..<1500 {
    let content = try? await SCShareableContent.excludingDesktopWindows(true, onScreenWindowsOnly: true)
    if let w = content?.windows
      .filter({ $0.owningApplication?.processID == pid && $0.windowLayer == 0 && $0.frame.width > 100 })
      .max(by: { $0.frame.width * $0.frame.height < $1.frame.width * $1.frame.height })
    {
      return w
    }
    try? await Task.sleep(nanoseconds: 10_000_000)  // start-up times the window's appearance
  }
  fail("no on-screen window for pid \(pid)")
}

let fps = NSScreen.main?.maximumFramesPerSecond ?? 60

func capture(_ window: SCWindow, into watcher: Watcher) async -> SCStream {
  let config = SCStreamConfiguration()
  config.width = Int(window.frame.width)  // points, not pixels: a smaller copy is cheaper to capture
  config.height = Int(window.frame.height)
  config.pixelFormat = kCVPixelFormatType_32BGRA
  config.colorSpaceName = CGColorSpace.sRGB
  config.minimumFrameInterval = CMTime(value: 1, timescale: CMTimeScale(fps))
  config.queueDepth = 8
  config.showsCursor = false
  let stream = SCStream(filter: SCContentFilter(desktopIndependentWindow: window), configuration: config, delegate: nil)
  do {
    try stream.addStreamOutput(watcher, type: .screen, sampleHandlerQueue: DispatchQueue(label: "frames", qos: .userInteractive))
    try await stream.startCapture()
  } catch { fail("capture: \(error)") }
  return stream
}

func run() async {
  // two captures starting at once in two processes can wedge the capture service: one process
  // watches both windows, set up one after the other, and gives up if that takes 20 s
  let watchdog = DispatchWorkItem { fail("the capture did not start in 20 s") }
  DispatchQueue.main.asyncAfter(deadline: .now() + 20, execute: watchdog)
  let win = await window(of: pid)
  print("window \(now()) \(Int(win.frame.width))x\(Int(win.frame.height))")
  print("period \(1_000_000_000 / fps)")
  let watcher = Watcher()
  watcher.wantSentinel = args[2] == "watch"
  watcher.saveDir = args.first(where: { $0.hasPrefix("save=") }).map { String($0.dropFirst(5)) }
  if let dir = watcher.saveDir {
    do { try FileManager.default.createDirectory(atPath: dir, withIntermediateDirectories: true) } catch { fail("save=\(dir): \(error)") }
  }
  let stream = await capture(win, into: watcher)
  // watch ... floor=PID: also the floor's window, for the capture's own frame rate
  let floor = Watcher()
  var floorStream: SCStream?
  if let f = args.first(where: { $0.hasPrefix("floor=") }), let fp = pid_t(f.dropFirst(6)) {
    floorStream = await capture(await window(of: fp), into: floor)
  }
  watchdog.cancel()

  if args[2] == "watch" {
    // until SECONDS have passed, or 0.5 s after a frame showed UNTIL sentinels
    let secs = Double(args[3]) ?? 10
    let until = args.count > 4 ? Int(args[4]) ?? 0 : 0  // "floor=…" parses as 0
    let start = now()
    print("start \(start)")
    // SIGTERM ends the watch early, with the frames so far printed
    signal(SIGTERM, SIG_IGN)
    let term = DispatchSource.makeSignalSource(signal: SIGTERM, queue: .main)
    let stopped = Watcher()  // a lock and a flag
    term.setEventHandler { stopped.lock.lock(); stopped.wantSentinel = true; stopped.lock.unlock() }
    term.resume()
    func stopping() -> Bool { stopped.lock.lock(); defer { stopped.lock.unlock() }; return stopped.wantSentinel }
    while now() < start + UInt64(secs * 1e9) && !stopping() {
      if until > 0, let first = watcher.snapshot().first(where: { $0.at > start && $0.sentinels >= until }),
        now() > first.at + 500_000_000
      { break }
      try? await Task.sleep(nanoseconds: 50_000_000)
    }
    for f in watcher.snapshot() where f.at > start { print("frame \(f.at) \(f.sentinels) \(f.dirty ? 1 : 0)") }
    watcher.writeKept()
    for f in floor.snapshot() where f.at > start { print("floor \(f.at) \(f.dirty ? 1 : 0)") }
    try? await floorStream?.stopCapture()
  } else {
    // a frame arrives only on a change: nudge the window once if the bar is not seen yet
    for _ in 0..<30 where !watcher.found { try? await Task.sleep(nanoseconds: 100_000_000) }
    guard watcher.found else { fail("no echo bar in the window: is scripts/perf/echo.py the focused pane?") }
    // keys go to the key window of the active app: bring the target (a process the caller started) forward
    NSRunningApplication(processIdentifier: pid)?.activate()
    try? await Task.sleep(nanoseconds: 1_200_000_000)  // and past the re-presenting that follows
    let n = max(1, Int(args[3]) ?? 100)
    let gap = max(0, Double(args.count > 4 ? args[4] : "50") ?? 50)
    let src = CGEventSource(stateID: .privateState)
    var missed = 0
    for i in 1...n {
      guard let down = CGEvent(keyboardEventSource: src, virtualKey: 7, keyDown: true),
        let up = CGEvent(keyboardEventSource: src, virtualKey: 7, keyDown: false)
      else { fail("can't make a key event") }
      // the colour after the one shown now (so one lost key doesn't throw every later one off)
      let want: Shade = watcher.snapshot().last?.bar == .green ? .red : .green
      let posted = now()
      down.postToPid(pid)
      up.postToPid(pid)
      var shown: UInt64?
      for _ in 0..<1000 {  // up to 1 s
        shown = watcher.snapshot().first { $0.at > posted && $0.bar == want }?.at
        if shown != nil { break }
        try? await Task.sleep(nanoseconds: 1_000_000)
      }
      if let shown {
        print("key \(i) \(posted) \(shown)")
      } else {
        missed += 1
        print("missed \(i) \(posted)")
        // a late echo may still land: let it, so the next key isn't credited with it (the next
        // key's colour is worked out from the bar as it is then)
        try? await Task.sleep(nanoseconds: 1_000_000_000)
      }
      // a random gap, so keys don't lock onto one phase of the display's refresh
      try? await Task.sleep(nanoseconds: UInt64((gap + (gap > 0 ? Double.random(in: 0..<gap) : 0)) * 1e6))
    }
    if missed == n { fail("no key changed the echo bar: is the window focused?") }
  }
  try? await stream.stopCapture()
  exit(0)
}

Task { await run() }
app.run()

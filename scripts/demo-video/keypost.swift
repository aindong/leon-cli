// keypost: post keys to the focused app (the calling process needs Accessibility).
//   keypost combo cmd+shift+\ | type "some text" | key return|esc|down|up|left|right|tab|space|backspace
//   keypost park: move the pointer to the middle of the right edge (a bottom edge shows the Dock)
//   keypost click X Y: click at X,Y points and park again at once, so no frame shows the pointer
//   keypost wiggle SECS: keep the parked pointer stepping on the right edge for SECS (see below)
//   keypost move X Y: the pointer to X,Y points and left there (a hover); rclick X Y: a right click
//   keypost scroll X Y DY: scroll DY points (negative: down the page) at X,Y, then park
// Real key codes on a US layout: GPUI's query fields ignore a key event's Unicode string.
import CoreGraphics
import Foundation

// index = virtual key code (kVK_ANSI_*); " " marks codes that aren't printable keys
let plain = Array("asdfhgzxcv bqweryt123465=97-80]ou[ip lj'k;\\,/nm.")
let named: [String: CGKeyCode] = [
  "return": 36, "tab": 48, "space": 49, "backspace": 51, "esc": 53, "left": 123, "right": 124,
  "down": 125, "up": 126, "`": 50,
]
let shifted: [Character: Character] = [
  "~": "`", "_": "-", "|": "\\", ":": ";", "\"": "'", "<": ",", ">": ".", "?": "/", "+": "=",
  "!": "1", "@": "2", "#": "3", "$": "4", "%": "5", "^": "6", "&": "7", "*": "8", "(": "9", ")": "0",
  "{": "[", "}": "]",
]
let src = CGEventSource(stateID: .hidSystemState)

func park() {
  let b = CGDisplayBounds(CGMainDisplayID())
  CGWarpMouseCursorPosition(CGPoint(x: b.maxX - 1, y: b.midY))
}

func post(_ code: CGKeyCode, _ flags: CGEventFlags = []) {
  for down in [true, false] {
    let e = CGEvent(keyboardEventSource: src, virtualKey: code, keyDown: down)!
    e.flags = flags
    e.post(tap: .cghidEventTap)
    usleep(12_000)
  }
}

func code(_ name: String) -> CGKeyCode? {
  if let c = named[name] { return c }
  if name.count == 1, let i = plain.firstIndex(of: Character(name)), plain[i] != " " { return CGKeyCode(i) }
  return nil
}

let a = CommandLine.arguments
switch (a.count, a.count > 1 ? a[1] : "") {
case (3, "combo"):
  var flags: CGEventFlags = []
  // split on the last "+" only, so "cmd+shift+=" and "cmd+shift++" both work
  var parts = a[2].components(separatedBy: "+")
  var key = parts.removeLast()
  if key.isEmpty { key = "+"; parts.removeLast() }
  for m in parts {
    switch m {
    case "cmd": flags.insert(.maskCommand)
    case "shift": flags.insert(.maskShift)
    case "ctrl": flags.insert(.maskControl)
    case "alt": flags.insert(.maskAlternate)
    default: print("keypost: unknown modifier \(m)"); exit(2)
    }
  }
  if key.count == 1, let base = shifted[Character(key)] { key = String(base); flags.insert(.maskShift) }
  guard let c = code(key) else { print("keypost: no key \(key)"); exit(2) }
  post(c, flags)
case (3, "key"):
  guard let c = code(a[2]) else { print("keypost: no key \(a[2])"); exit(2) }
  post(c)
case (3, "type"):
  for ch in a[2] {
    var c = ch
    var flags: CGEventFlags = []
    if let base = shifted[ch] { c = base; flags = .maskShift }
    if ch.isUppercase { c = Character(ch.lowercased()); flags = .maskShift }
    guard let k = code(c == " " ? "space" : String(c)) else { print("keypost: no key for \(ch)"); exit(2) }
    post(k, flags)
    usleep(55_000)  // a typing pace that reads on video
  }
case (2, "park"):
  park()
case (4, "click"):
  guard let x = Double(a[2]), let y = Double(a[3]) else { print("keypost: click X Y"); exit(2) }
  let at = CGPoint(x: x, y: y)
  for type in [CGEventType.leftMouseDown, .leftMouseUp] {
    CGEvent(mouseEventSource: src, mouseType: type, mouseCursorPosition: at, mouseButton: .left)!.post(tap: .cghidEventTap)
    usleep(8_000)
  }
  park()
case (4, "move"), (4, "rclick"):
  guard let x = Double(a[2]), let y = Double(a[3]) else { print("keypost: \(a[1]) X Y"); exit(2) }
  let at = CGPoint(x: x, y: y)
  CGWarpMouseCursorPosition(at)
  let kinds: [CGEventType] = a[1] == "move" ? [.mouseMoved] : [.mouseMoved, .rightMouseDown, .rightMouseUp]
  for type in kinds {
    CGEvent(mouseEventSource: src, mouseType: type, mouseCursorPosition: at, mouseButton: .right)!.post(tap: .cghidEventTap)
    usleep(8_000)
  }
case (5, "scroll"):
  guard let x = Double(a[2]), let y = Double(a[3]), let dy = Int32(a[4]) else { print("keypost: scroll X Y DY"); exit(2) }
  // a real move first: the app scrolls what is under the pointer it last saw
  CGWarpMouseCursorPosition(CGPoint(x: x, y: y))
  CGEvent(mouseEventSource: src, mouseType: .mouseMoved, mouseCursorPosition: CGPoint(x: x, y: y), mouseButton: .left)!
    .post(tap: .cghidEventTap)
  usleep(50_000)
  // in steps, as a trackpad sends them: one big jump can be read as a fling
  let step: Int32 = dy < 0 ? -20 : 20
  // each at X,Y itself: during a take `keypost wiggle` moves the pointer to the edge meanwhile
  for _ in 0..<(abs(dy) / 20) {
    let e = CGEvent(scrollWheelEvent2Source: src, units: .pixel, wheelCount: 1, wheel1: step, wheel2: 0, wheel3: 0)!
    e.location = CGPoint(x: x, y: y)
    e.post(tap: .cghidEventTap)
    usleep(16_000)
  }
  park()
case (3, "wiggle"):
  // the parked pointer steps 4 pt up and down the right edge (in the strip the render crops off)
  // for SECS: a frame every 0.2 s, so the recorder never sees a still screen after a pane closes
  guard let secs = Double(a[2]) else { print("keypost: wiggle SECS"); exit(2) }
  let b = CGDisplayBounds(CGMainDisplayID())
  for i in 0..<Int(secs * 5) {
    CGWarpMouseCursorPosition(CGPoint(x: b.maxX - 1, y: b.midY + CGFloat(i % 2 * 4)))
    usleep(200_000)
  }
  park()
default:
  print("usage: keypost combo KEYS | key NAME | type TEXT | park | click X Y | move X Y | rclick X Y | scroll X Y DY | wiggle SECS"); exit(2)
}

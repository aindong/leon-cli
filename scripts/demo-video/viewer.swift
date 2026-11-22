// viewer: the web demo's "browser on your laptop", a plain window that shows one page.
//   viewer URL X Y W H: a window at X,Y (points, top left) of W x H, an address line over the page
// A browser of its own, so the demo shows no other product's window or logo. The address line is
// the page's real URL, read only. It loads URL as any browser would: leon's Host, Origin and token
// checks see an ordinary page (docs/architecture/web.md). `run-demo.sh push` builds it.
import AppKit
import WebKit

final class Delegate: NSObject, NSApplicationDelegate, WKNavigationDelegate {
  let url: URL, x: Double, y: Double, w: Double, h: Double
  var window: NSWindow!
  let bar = NSTextField(labelWithString: "")

  init(_ url: URL, _ x: Double, _ y: Double, _ w: Double, _ h: Double) {
    (self.url, self.x, self.y, self.w, self.h) = (url, x, y, w, h)
  }

  func applicationDidFinishLaunching(_ note: Notification) {
    let screen = NSScreen.main!.frame
    let frame = NSRect(x: x, y: screen.height - y - h, width: w, height: h)
    window = NSWindow(contentRect: frame, styleMask: [.titled, .closable, .resizable], backing: .buffered, defer: false)
    window.title = "leon"
    window.appearance = NSAppearance(named: .darkAqua)
    let content = window.contentView!
    let barH = 30.0
    bar.frame = NSRect(x: 12, y: h - barH + 7, width: w - 24, height: 18)
    bar.autoresizingMask = [.width, .minYMargin]
    bar.font = .systemFont(ofSize: 13)
    bar.textColor = .secondaryLabelColor
    bar.stringValue = url.absoluteString
    let web = WKWebView(frame: NSRect(x: 0, y: 0, width: w, height: h - barH))
    web.autoresizingMask = [.width, .height]
    web.navigationDelegate = self
    content.addSubview(bar)
    content.addSubview(web)
    web.load(URLRequest(url: url))
    window.makeKeyAndOrderFront(nil)
    window.makeFirstResponder(web)
    NSApp.activate(ignoringOtherApps: true)
  }

  func webView(_ webView: WKWebView, didCommit navigation: WKNavigation!) {
    bar.stringValue = webView.url?.absoluteString ?? bar.stringValue
  }

  func applicationShouldTerminateAfterLastWindowClosed(_ app: NSApplication) -> Bool { true }
}

let a = CommandLine.arguments
guard a.count == 6, let url = URL(string: a[1]), let x = Double(a[2]), let y = Double(a[3]),
  let w = Double(a[4]), let h = Double(a[5])
else {
  print("usage: viewer URL X Y W H")
  exit(2)
}
let app = NSApplication.shared
app.setActivationPolicy(.regular)
let delegate = Delegate(url, x, y, w, h)
app.delegate = delegate
app.run()

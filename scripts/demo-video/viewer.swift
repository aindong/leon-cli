// viewer: the web demo's "browser on your other machine", a plain window with an address bar.
//   viewer URL X Y W H PROXY [blank]: a window at X,Y (points, top left) of W x H showing URL;
//   blank: the address bar starts empty and focused, and Return loads what was typed or pasted
// A browser of its own, so the demo shows no other product's window or logo. The VM can't resolve
// the made-up MagicDNS name, so every request goes through kit/bin/proxy.py on 127.0.0.1:PROXY,
// which passes it on unchanged to leon on this machine: leon's Host, Origin and token checks see
// the page by its MagicDNS name (docs/architecture/web.md). `run-demo.sh push` builds it.
import AppKit
import Network
import WebKit

final class Delegate: NSObject, NSApplicationDelegate, NSTextFieldDelegate {
  let url: URL, frame: NSRect, proxy: UInt16, blank: Bool
  var window: NSWindow!
  var web: WKWebView!
  let bar = NSTextField(string: "")

  init(_ url: URL, _ frame: NSRect, _ proxy: UInt16, _ blank: Bool) {
    (self.url, self.frame, self.proxy, self.blank) = (url, frame, proxy, blank)
  }

  func applicationDidFinishLaunching(_ note: Notification) {
    menus()
    let screen = NSScreen.main!.frame
    let (w, h, barH) = (frame.width, frame.height, 44.0)
    let rect = NSRect(x: frame.minX, y: screen.height - frame.minY - h, width: w, height: h)
    window = NSWindow(contentRect: rect, styleMask: [.titled, .closable, .resizable], backing: .buffered, defer: false)
    window.title = "leon"
    window.appearance = NSAppearance(named: .darkAqua)
    let content = window.contentView!
    bar.frame = NSRect(x: 12, y: h - barH + 8, width: w - 24, height: 28)
    bar.autoresizingMask = [.width, .minYMargin]
    bar.bezelStyle = .roundedBezel
    bar.font = .systemFont(ofSize: 14)
    bar.placeholderString = "Address"
    bar.delegate = self
    let config = WKWebViewConfiguration()
    config.websiteDataStore = .nonPersistent()
    if #available(macOS 14.0, *) {
      let at = NWEndpoint.hostPort(host: "127.0.0.1", port: NWEndpoint.Port(rawValue: proxy)!)
      config.websiteDataStore.proxyConfigurations = [ProxyConfiguration(httpCONNECTProxy: at)]
    }
    web = WKWebView(frame: NSRect(x: 0, y: 0, width: w, height: h - barH), configuration: config)
    web.autoresizingMask = [.width, .height]
    // 688 pt is 764 CSS px: the page's desktop layout starts above 700 (phone.js)
    web.pageZoom = 0.9
    web.setValue(false, forKey: "drawsBackground") // dark while the page loads, not a white flash
    content.addSubview(bar)
    content.addSubview(web)
    window.makeKeyAndOrderFront(nil)
    if blank {
      window.makeFirstResponder(bar)
    } else {
      load(url)
    }
    NSApp.activate(ignoringOtherApps: true)
  }

  func load(_ u: URL) {
    bar.stringValue = u.absoluteString
    web.load(URLRequest(url: u))
    window.makeFirstResponder(web)
  }

  // Return in the address bar opens what is in it
  func control(_ control: NSControl, textView: NSTextView, doCommandBy sel: Selector) -> Bool {
    guard sel == #selector(NSResponder.insertNewline(_:)) else { return false }
    let text = bar.stringValue.trimmingCharacters(in: .whitespaces)
    if let u = URL(string: text.contains("://") ? text : "http://" + text) { load(u) }
    return true
  }

  // an Edit menu, so ⌘V pastes into the address bar
  func menus() {
    let main = NSMenu()
    let appItem = NSMenuItem()
    appItem.submenu = NSMenu()
    appItem.submenu!.addItem(withTitle: "Quit", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
    let editItem = NSMenuItem()
    let edit = NSMenu(title: "Edit")
    edit.addItem(withTitle: "Cut", action: #selector(NSText.cut(_:)), keyEquivalent: "x")
    edit.addItem(withTitle: "Copy", action: #selector(NSText.copy(_:)), keyEquivalent: "c")
    edit.addItem(withTitle: "Paste", action: #selector(NSText.paste(_:)), keyEquivalent: "v")
    edit.addItem(withTitle: "Select All", action: #selector(NSText.selectAll(_:)), keyEquivalent: "a")
    editItem.submenu = edit
    main.addItem(appItem)
    main.addItem(editItem)
    NSApp.mainMenu = main
  }

  func applicationShouldTerminateAfterLastWindowClosed(_ app: NSApplication) -> Bool { true }
}

let a = CommandLine.arguments
guard (7...8).contains(a.count), let url = URL(string: a[1]), let x = Double(a[2]), let y = Double(a[3]),
  let w = Double(a[4]), let h = Double(a[5]), let proxy = UInt16(a[6])
else {
  print("usage: viewer URL X Y W H PROXY [blank]")
  exit(2)
}
let app = NSApplication.shared
app.setActivationPolicy(.regular)
let delegate = Delegate(url, NSRect(x: x, y: y, width: w, height: h), proxy, a.count == 8 && a[7] == "blank")
app.delegate = delegate
app.run()

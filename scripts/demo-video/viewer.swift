// viewer: the web demo's browsers on "your other machines", neutral and unbranded.
//   viewer URL X Y W H PROXY laptop: a browser window at X,Y (points, top left) of W x H: a tab
//     strip, back, forward, reload, an address bar with a padlock; the address bar starts empty
//     and focused, and Return opens what was typed or pasted
//   viewer URL X Y W H PROXY phone: a phone-shaped frame with the page open at URL
// The VM can't resolve the made-up MagicDNS name, so every request goes through kit/bin/proxy.py
// on 127.0.0.1:PROXY, which passes it on unchanged to leon here: leon's Host, Origin and token
// checks see the page by its name (docs/architecture/web.md). The page is HTTPS with a certificate
// kit/web.sh makes for that name alone; this viewer trusts it for that name and nothing else.
// `run-demo.sh push` builds it.
import AppKit
import Network
import WebKit

final class Key: NSWindow {  // a borderless window that still takes the keys (the phone)
  override var canBecomeKey: Bool { true }
  override var canBecomeMain: Bool { true }
}

func symbol(_ name: String, _ size: CGFloat) -> NSImageView {
  let v = NSImageView(image: NSImage(systemSymbolName: name, accessibilityDescription: nil)!)
  v.symbolConfiguration = .init(pointSize: size, weight: .medium)
  v.contentTintColor = .secondaryLabelColor
  return v
}

final class Delegate: NSObject, NSApplicationDelegate, NSTextFieldDelegate, WKNavigationDelegate {
  let url: URL, frame: NSRect, proxy: UInt16, phone: Bool
  var window: NSWindow!
  var web: WKWebView!
  let bar = NSTextField(string: "")
  let tab = NSTextField(labelWithString: "New Tab")
  let lock = symbol("lock.fill", 11)

  init(_ url: URL, _ frame: NSRect, _ proxy: UInt16, _ phone: Bool) {
    (self.url, self.frame, self.proxy, self.phone) = (url, frame, proxy, phone)
  }

  func applicationDidFinishLaunching(_ note: Notification) {
    menus()
    let screen = NSScreen.main!.frame
    let (w, h) = (frame.width, frame.height)
    let rect = NSRect(x: frame.minX, y: screen.height - frame.minY - h, width: w, height: h)
    let config = WKWebViewConfiguration()
    config.websiteDataStore = .nonPersistent()
    if #available(macOS 14.0, *) {
      let at = NWEndpoint.hostPort(host: "127.0.0.1", port: NWEndpoint.Port(rawValue: proxy)!)
      config.websiteDataStore.proxyConfigurations = [ProxyConfiguration(httpCONNECTProxy: at)]
    }
    web = WKWebView(frame: .zero, configuration: config)
    web.navigationDelegate = self
    web.setValue(false, forKey: "drawsBackground")  // dark while the page loads, not a white flash
    if phone { phoneWindow(rect) } else { laptopWindow(rect) }
    window.appearance = NSAppearance(named: .darkAqua)
    window.makeKeyAndOrderFront(nil)
    if phone { load(url) } else { window.makeFirstResponder(bar) }
    NSApp.activate(ignoringOtherApps: true)
  }

  // a browser: the tab strip beside the window's buttons, then back, forward, reload and the address
  func laptopWindow(_ rect: NSRect) {
    window = NSWindow(contentRect: rect, styleMask: [.titled, .closable, .resizable, .fullSizeContentView],
                      backing: .buffered, defer: false)
    window.titlebarAppearsTransparent = true
    window.titleVisibility = .hidden
    let (w, h) = (rect.width, rect.height)
    let content = window.contentView!
    content.wantsLayer = true
    content.layer?.backgroundColor = NSColor(white: 0.16, alpha: 1).cgColor
    // the tab: a lighter rounded shape, joined to the toolbar under it
    let tabBox = NSView(frame: NSRect(x: 80, y: h - 34, width: 220, height: 30))
    tabBox.wantsLayer = true
    tabBox.layer?.backgroundColor = NSColor(white: 0.23, alpha: 1).cgColor
    tabBox.layer?.cornerRadius = 8
    tabBox.layer?.maskedCorners = [.layerMinXMaxYCorner, .layerMaxXMaxYCorner]
    tab.frame = NSRect(x: 14, y: 7, width: 170, height: 17)
    tab.font = .systemFont(ofSize: 12.5, weight: .medium)
    tab.textColor = .labelColor
    tab.lineBreakMode = .byTruncatingTail
    tabBox.addSubview(tab)
    let close = symbol("xmark", 9)
    close.frame = NSRect(x: 196, y: 9, width: 12, height: 12)
    tabBox.addSubview(close)
    let plus = symbol("plus", 12)
    plus.frame = NSRect(x: 310, y: h - 26, width: 16, height: 16)
    let toolbar = NSView(frame: NSRect(x: 0, y: h - 78, width: w, height: 44))
    toolbar.wantsLayer = true
    toolbar.layer?.backgroundColor = NSColor(white: 0.23, alpha: 1).cgColor
    toolbar.autoresizingMask = [.width, .minYMargin]
    for (i, name) in ["chevron.left", "chevron.right", "arrow.clockwise"].enumerated() {
      let b = symbol(name, 14)
      b.frame = NSRect(x: 14 + CGFloat(i) * 32, y: 12, width: 20, height: 20)
      toolbar.addSubview(b)
    }
    let field = NSView(frame: NSRect(x: 112, y: 7, width: w - 126, height: 30))
    field.wantsLayer = true
    field.layer?.backgroundColor = NSColor(white: 0.12, alpha: 1).cgColor
    field.layer?.cornerRadius = 15
    field.autoresizingMask = [.width]
    lock.frame = NSRect(x: 12, y: 8, width: 14, height: 14)
    lock.isHidden = true
    bar.frame = NSRect(x: 32, y: 6, width: field.frame.width - 44, height: 19)
    bar.isBordered = false
    bar.drawsBackground = false
    bar.focusRingType = .none
    bar.font = .systemFont(ofSize: 14)
    bar.textColor = .labelColor
    bar.placeholderString = "Search or enter address"
    bar.delegate = self
    bar.autoresizingMask = [.width]
    field.addSubview(lock)
    field.addSubview(bar)
    toolbar.addSubview(field)
    web.frame = NSRect(x: 0, y: 0, width: w, height: h - 78)
    web.autoresizingMask = [.width, .height]
    for v in [tabBox, plus, toolbar, web!] as [NSView] { content.addSubview(v) }
  }

  // a phone: a black rounded frame, a small address pill at the top, the page under it
  func phoneWindow(_ rect: NSRect) {
    window = Key(contentRect: rect, styleMask: [.borderless], backing: .buffered, defer: false)
    window.isOpaque = false
    window.backgroundColor = .clear
    window.hasShadow = true
    let (w, h, edge) = (rect.width, rect.height, 12.0)
    let body = NSView(frame: NSRect(x: 0, y: 0, width: w, height: h))
    body.wantsLayer = true
    body.layer?.backgroundColor = NSColor(white: 0.05, alpha: 1).cgColor
    body.layer?.cornerRadius = 54
    body.layer?.borderColor = NSColor(white: 0.32, alpha: 1).cgColor
    body.layer?.borderWidth = 2
    let screen = NSView(frame: NSRect(x: edge, y: edge, width: w - 2 * edge, height: h - 2 * edge))
    screen.wantsLayer = true
    screen.layer?.backgroundColor = NSColor(white: 0.12, alpha: 1).cgColor
    screen.layer?.cornerRadius = 44
    screen.layer?.masksToBounds = true
    let sw = screen.frame.width, sh = screen.frame.height
    let island = NSView(frame: NSRect(x: sw / 2 - 52, y: sh - 34, width: 104, height: 28))
    island.wantsLayer = true
    island.layer?.backgroundColor = NSColor.black.cgColor
    island.layer?.cornerRadius = 14
    let pill = NSView(frame: NSRect(x: 16, y: sh - 78, width: sw - 32, height: 32))
    pill.wantsLayer = true
    pill.layer?.backgroundColor = NSColor(white: 0.22, alpha: 1).cgColor
    pill.layer?.cornerRadius = 16
    lock.frame = NSRect(x: 14, y: 9, width: 14, height: 14)
    bar.frame = NSRect(x: 32, y: 7, width: pill.frame.width - 44, height: 18)
    bar.isEditable = false
    bar.isBordered = false
    bar.drawsBackground = false
    bar.font = .systemFont(ofSize: 13, weight: .medium)
    bar.textColor = .labelColor
    pill.addSubview(lock)
    pill.addSubview(bar)
    web.frame = NSRect(x: 0, y: 0, width: sw, height: sh - 86)
    web.pageZoom = 1.15  // 396 pt is 344 CSS px: the phone layout, its text big enough to read
    for v in [island, pill, web!] as [NSView] { screen.addSubview(v) }
    body.addSubview(screen)
    window.contentView = body
  }

  func load(_ u: URL) {
    // the phone shows the host alone, as phone browsers do; the laptop the whole address
    bar.stringValue = phone ? (u.host ?? u.absoluteString) : u.absoluteString
    lock.isHidden = u.scheme != "https"
    web.load(URLRequest(url: u))
    window.makeFirstResponder(web)
  }

  // Return in the address bar opens what is in it
  func control(_ control: NSControl, textView: NSTextView, doCommandBy sel: Selector) -> Bool {
    guard sel == #selector(NSResponder.insertNewline(_:)) else { return false }
    let text = bar.stringValue.trimmingCharacters(in: .whitespaces)
    if let u = URL(string: text.contains("://") ? text : "https://" + text) { load(u) }
    return true
  }

  func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
    tab.stringValue = webView.title.flatMap { $0.isEmpty ? nil : $0 } ?? url.host ?? "leon"
  }

  // the demo's certificate is self-signed for the made-up name: trusted for that host only
  func webView(_ webView: WKWebView, didReceive challenge: URLAuthenticationChallenge,
               completionHandler: @escaping (URLSession.AuthChallengeDisposition, URLCredential?) -> Void) {
    let space = challenge.protectionSpace
    if space.authenticationMethod == NSURLAuthenticationMethodServerTrust, space.host == url.host,
      let trust = space.serverTrust
    {
      completionHandler(.useCredential, URLCredential(trust: trust))
    } else {
      completionHandler(.performDefaultHandling, nil)
    }
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
guard a.count == 8, let url = URL(string: a[1]), let x = Double(a[2]), let y = Double(a[3]),
  let w = Double(a[4]), let h = Double(a[5]), let proxy = UInt16(a[6]), ["laptop", "phone"].contains(a[7])
else {
  print("usage: viewer URL X Y W H PROXY laptop|phone")
  exit(2)
}
let app = NSApplication.shared
app.setActivationPolicy(.regular)
let delegate = Delegate(url, NSRect(x: x, y: y, width: w, height: h), proxy, a[7] == "phone")
app.delegate = delegate
app.run()

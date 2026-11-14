// winshot <owner-name> <out.png>: capture only the frontmost on-screen window owned by a process name.
import Foundation
import CoreGraphics
let args = CommandLine.arguments
guard args.count == 3 else { print("usage: winshot <owner> <out.png>"); exit(2) }
let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as! [[String: Any]]
guard let w = list.first(where: { ($0[kCGWindowOwnerName as String] as? String) == args[1] && ($0[kCGWindowLayer as String] as? Int) == 0 }),
      let id = w[kCGWindowNumber as String] as? Int else { print("no window for \(args[1])"); exit(1) }
let p = Process(); p.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture"); p.arguments = ["-x", "-o", "-l\(id)", args[2]]
try! p.run(); p.waitUntilExit(); print("captured window \(id)"); exit(p.terminationStatus)

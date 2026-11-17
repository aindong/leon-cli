// winshot <pid|owner-name> <out.png>: capture only the frontmost on-screen window of a process, by
// its pid (a name matches any process of that name: another leon running on this Mac too).
import Foundation
import CoreGraphics
let args = CommandLine.arguments
guard args.count == 3 else { print("usage: winshot <pid|owner> <out.png>"); exit(2) }
let pid = Int(args[1])
let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as! [[String: Any]]
let mine = { (w: [String: Any]) -> Bool in
    pid.map { (w[kCGWindowOwnerPID as String] as? Int) == $0 } ?? ((w[kCGWindowOwnerName as String] as? String) == args[1])
}
guard let w = list.first(where: { mine($0) && ($0[kCGWindowLayer as String] as? Int) == 0 }),
      let id = w[kCGWindowNumber as String] as? Int else { print("no window for \(args[1])"); exit(1) }
let p = Process(); p.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture"); p.arguments = ["-x", "-o", "-l\(id)", args[2]]
try! p.run(); p.waitUntilExit(); print("captured window \(id)"); exit(p.terminationStatus)

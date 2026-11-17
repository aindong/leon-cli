// winrec: a video of leon's window as the screen draws it over deskshot's backdrop, for a GIF that
// shows the app's motion (screencapture takes a few frames a second; this takes 30).
//   winrec PID BACKDROP_PID out.mov [PAD]   record PID's frontmost window and PAD points around it
//                                           until SIGTERM or SIGINT; no cursor
// The picture is made from only PID's and BACKDROP_PID's windows (ScreenCaptureKit leaves every
// other window out of it), so nothing else on this machine can be in a frame. Windows PID opens
// after the start are left out too. Build it with swiftc; it needs Screen Recording.
import AVFoundation
import ScreenCaptureKit

let args = CommandLine.arguments
guard args.count == 4 || args.count == 5, let pid = Int32(args[1]), let backdrop = Int32(args[2]),
      let pad = Double(args.count == 5 ? args[4] : "0")
else { print("usage: winrec PID BACKDROP_PID out.mov [PAD]"); exit(2) }

final class Writer: NSObject, SCStreamOutput {
    let writer: AVAssetWriter
    let input: AVAssetWriterInput
    var started = false
    init(_ url: URL, _ w: Int, _ h: Int) throws {
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
        // a frame only when the picture changed: an idle one carries no image
        guard type == .screen, buffer.isValid,
              let info = CMSampleBufferGetSampleAttachmentsArray(buffer, createIfNecessary: false) as? [[SCStreamFrameInfo: Any]],
              let raw = info.first?[.status] as? Int, SCFrameStatus(rawValue: raw) == .complete
        else { return }
        if !started { writer.startSession(atSourceTime: buffer.presentationTimeStamp); started = true }
        if input.isReadyForMoreMediaData { input.append(buffer) }
    }
}

let content = try await SCShareableContent.excludingDesktopWindows(true, onScreenWindowsOnly: true)
guard let win = content.windows.first(where: { $0.owningApplication?.processID == pid && $0.windowLayer == 0 })
else { print("winrec: no window for pid \(pid)"); exit(1) }
let rect = win.frame.insetBy(dx: -pad, dy: -pad).integral
guard let display = content.displays.first(where: { $0.frame.contains(rect) })
else { print("winrec: \(rect) isn't on one display"); exit(1) }
let ours = content.windows.filter { [pid, backdrop].contains($0.owningApplication?.processID) }
let filter = SCContentFilter(display: display, including: ours)
let config = SCStreamConfiguration()
let scale = Double(filter.pointPixelScale)
config.sourceRect = rect.offsetBy(dx: -display.frame.minX, dy: -display.frame.minY)
config.width = Int(rect.width * scale)
config.height = Int(rect.height * scale)
config.minimumFrameInterval = CMTime(value: 1, timescale: 30)
config.showsCursor = false
config.queueDepth = 6

let out = try Writer(URL(fileURLWithPath: args[3]), config.width, config.height)
let stream = SCStream(filter: filter, configuration: config, delegate: nil)
try stream.addStreamOutput(out, type: .screen, sampleHandlerQueue: DispatchQueue(label: "winrec"))
try await stream.startCapture()
print("winrec: recording \(rect)")

let stop = { (_: Int32) in
    Task {
        try? await stream.stopCapture()
        // the last frame lasts until now
        out.writer.endSession(atSourceTime: CMClockGetTime(CMClockGetHostTimeClock()))
        out.input.markAsFinished()
        await out.writer.finishWriting()
        exit(out.writer.status == .completed && out.started ? 0 : 1)
    }
}
var sources: [DispatchSourceSignal] = []
for sig in [SIGTERM, SIGINT] {
    signal(sig, SIG_IGN)
    let s = DispatchSource.makeSignalSource(signal: sig, queue: .global())
    s.setEventHandler { stop(sig) }
    s.resume()
    sources.append(s)
}
while true { try await Task.sleep(nanoseconds: 1_000_000_000) }

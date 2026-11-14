// ocr <image.png>…: print the text macOS Vision reads in each image, one "path: line" per line.
// The privacy test (crates/leon/tests/m13_privacy.rs) checks it for the user's login and hostname.
import Foundation
import Vision
import AppKit

for path in CommandLine.arguments.dropFirst() {
    guard let image = NSImage(contentsOfFile: path)?.cgImage(forProposedRect: nil, context: nil, hints: nil) else {
        print("\(path): unreadable"); exit(1)
    }
    let request = VNRecognizeTextRequest()
    request.recognitionLevel = .accurate
    request.usesLanguageCorrection = false
    try VNImageRequestHandler(cgImage: image).perform([request])
    for o in request.results ?? [] {
        if let t = o.topCandidates(1).first { print("\(path): \(t.string)") }
    }
}

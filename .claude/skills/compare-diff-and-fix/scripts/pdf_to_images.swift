import Foundation
import PDFKit
import AppKit
import CoreGraphics
import ImageIO
import UniformTypeIdentifiers

guard CommandLine.arguments.count >= 4 else {
    fputs("Usage: swift pdf_to_images.swift <input.pdf> <output_dir> <prefix> [scale=2.0] [opacity=0.40] [tint=1]\n", stderr)
    exit(1)
}

let pdfPath = CommandLine.arguments[1]
let outDir = CommandLine.arguments[2]
let prefix = CommandLine.arguments[3]
let scale = CommandLine.arguments.count >= 5 ? (Double(CommandLine.arguments[4]) ?? 2.0) : 2.0
let opacity = CommandLine.arguments.count >= 6 ? (Double(CommandLine.arguments[5]) ?? 0.40) : 0.40
let useTint = CommandLine.arguments.count >= 7 ? (CommandLine.arguments[6] != "0") : true

let pdfURL = URL(fileURLWithPath: pdfPath)
guard let doc = PDFDocument(url: pdfURL) else {
    fputs("Error: failed to open PDF at \(pdfPath)\n", stderr)
    exit(1)
}

try FileManager.default.createDirectory(atPath: outDir, withIntermediateDirectories: true)

let colorSpace = CGColorSpaceCreateDeviceRGB()
let pageCount = doc.pageCount

for idx in 0..<pageCount {
    guard let page = doc.page(at: idx) else { continue }
    let mediaBox = page.bounds(for: .mediaBox)
    let widthPx = max(1, Int((Double(mediaBox.width) * scale).rounded()))
    let heightPx = max(1, Int((Double(mediaBox.height) * scale).rounded()))

    let bytesPerPixel = 4
    let bytesPerRow = widthPx * bytesPerPixel
    var rawData = [UInt8](repeating: 255, count: heightPx * bytesPerRow)

    rawData.withUnsafeMutableBytes { ptr in
        guard let baseAddress = ptr.baseAddress else { return }
        if let ctx = CGContext(
            data: baseAddress,
            width: widthPx,
            height: heightPx,
            bitsPerComponent: 8,
            bytesPerRow: bytesPerRow,
            space: colorSpace,
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ) {
            ctx.setFillColor(red: 1.0, green: 1.0, blue: 1.0, alpha: 1.0)
            ctx.fill(CGRect(x: 0, y: 0, width: widthPx, height: heightPx))
            ctx.saveGState()
            ctx.scaleBy(x: CGFloat(scale), y: CGFloat(scale))
            page.draw(with: .mediaBox, to: ctx)
            ctx.restoreGState()
        }
    }

    func savePNG(_ pixels: [UInt8], to path: String) {
        var copy = pixels
        copy.withUnsafeMutableBytes { ptr in
            guard let baseAddress = ptr.baseAddress else { return }
            if let ctx = CGContext(
                data: baseAddress,
                width: widthPx,
                height: heightPx,
                bitsPerComponent: 8,
                bytesPerRow: bytesPerRow,
                space: colorSpace,
                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
            ), let cgImage = ctx.makeImage() {
                let url = URL(fileURLWithPath: path) as CFURL
                if let dest = CGImageDestinationCreateWithURL(url, UTType.png.identifier as CFString, 1, nil) {
                    CGImageDestinationAddImage(dest, cgImage, nil)
                    CGImageDestinationFinalize(dest)
                }
            }
        }
    }

    let pageNum = idx + 1
    let rawOutPath = "\(outDir)/\(prefix)_raw_\(pageNum).png"
    savePNG(rawData, to: rawOutPath)

    // Create dimmed/tinted background version for Typst `#set page(background: ...)` overlay:
    // White pixels stay pure white (255, 255, 255); ink pixels are dimmed by `opacity`
    // (and optionally shifted toward a coral-red tint so Word's background layer is visually
    // distinct from Typst's black foreground layer).
    var bgData = rawData
    let pixelCount = widthPx * heightPx
    for p in 0..<pixelCount {
        let offset = p * 4
        let r = Double(rawData[offset])
        let g = Double(rawData[offset + 1])
        let b = Double(rawData[offset + 2])

        // Darkness in [0, 1] where 0 is white and 1 is black
        let dr = (255.0 - r) / 255.0
        let dg = (255.0 - g) / 255.0
        let db = (255.0 - b) / 255.0
        let darkness = max(dr, max(dg, db))

        if darkness > 0.01 {
            if useTint {
                // Coral-red / magenta tint for Word background layer:
                // R stays bright, G and B are attenuated proportionally to `opacity * darkness`.
                let nr = 255.0 - (255.0 - r) * (opacity * 0.25)
                let ng = 255.0 - max(255.0 - g, darkness * 255.0) * opacity
                let nb = 255.0 - max(255.0 - b, darkness * 255.0) * (opacity * 0.85)
                bgData[offset] = UInt8(clamping: Int(nr.rounded()))
                bgData[offset + 1] = UInt8(clamping: Int(ng.rounded()))
                bgData[offset + 2] = UInt8(clamping: Int(nb.rounded()))
            } else {
                // Pure brightness/lightness dimming:
                let nr = 255.0 - (255.0 - r) * opacity
                let ng = 255.0 - (255.0 - g) * opacity
                let nb = 255.0 - (255.0 - b) * opacity
                bgData[offset] = UInt8(clamping: Int(nr.rounded()))
                bgData[offset + 1] = UInt8(clamping: Int(ng.rounded()))
                bgData[offset + 2] = UInt8(clamping: Int(nb.rounded()))
            }
        }
    }

    let bgOutPath = "\(outDir)/\(prefix)_bg_\(pageNum).png"
    savePNG(bgData, to: bgOutPath)
    print(String(format: "page=%d width_pt=%.2f height_pt=%.2f raw=%@ bg=%@", pageNum, Double(mediaBox.width), Double(mediaBox.height), rawOutPath, bgOutPath))
}

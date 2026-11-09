import Foundation
import PDFKit
import CoreGraphics
import AppKit

struct LineRecord: Codable {
    let lineIndex: Int
    let text: String
    let firstToken: String
    let lastToken: String
    let xPt: Double
    let yTopPt: Double
    let widthPt: Double
    let heightPt: Double
}

struct PageRecord: Codable {
    let pageNumber: Int
    let widthPt: Double
    let heightPt: Double
    let inkMinXPt: Double
    let inkMaxXPt: Double
    let inkMinYPt: Double
    let inkMaxYPt: Double
    let lines: [LineRecord]
}

struct LineDiffRecord: Codable {
    let index: Int
    let status: String // "EXACT", "WORD_BOUNDARY_DIFF", "DRIFT", "TYPST_ONLY", "WORD_ONLY"
    let typstText: String?
    let wordText: String?
    let typstYTopPt: Double?
    let wordYTopPt: Double?
    let deltaYPt: Double?
    let deltaWidthPt: Double?
}

struct PageComparison: Codable {
    let pageNumber: Int
    let typstLineCount: Int
    let wordLineCount: Int
    let exactLineMatches: Int
    let lineParityRate: Double
    let meanAbsDeltaYPt: Double
    let maxAbsDeltaYPt: Double
    let meanAbsDeltaWidthPt: Double
    let inkTopDeltaPt: Double
    let inkBottomDeltaPt: Double
    let inkLeftDeltaPt: Double
    let inkRightDeltaPt: Double
    let pixelOverlapRate: Double
    let lineDiffs: [LineDiffRecord]
}

struct ComparisonReport: Codable {
    let typstPdfPath: String
    let wordPdfPath: String
    let overlayPdfPath: String?
    let typstPageCount: Int
    let wordPageCount: Int
    let pageCountMatch: Bool
    let totalTypstLines: Int
    let totalWordLines: Int
    let totalExactLineMatches: Int
    let overallLineParityRate: Double
    let overallMeanAbsDeltaYPt: Double
    let overallMaxAbsDeltaYPt: Double
    let pages: [PageComparison]
}

func normalizeText(_ s: String) -> String {
    var out = ""
    for scalar in s.unicodeScalars {
        let v = scalar.value
        if (0x2F00...0x2FDF).contains(v) || (0xF900...0xFAFF).contains(v) {
            let str = String(scalar).precomposedStringWithCompatibilityMapping
            out.append(str)
        } else if v == 0x2018 || v == 0x2019 {
            out.append("'")
        } else if v == 0x201C || v == 0x201D {
            out.append("\"")
        } else if v == 0x00A0 {
            out.append(" ")
        } else {
            out.append(Character(scalar))
        }
    }
    return out
        .replacingOccurrences(of: "人又", with: "人口")
        .replacingOccurrences(of: "转又", with: "转口")
        .replacingOccurrences(of: "\r\n", with: "\n")
        .replacingOccurrences(of: "\r", with: "\n")
        .trimmingCharacters(in: .whitespacesAndNewlines)
}

func extractTokens(_ line: String) -> (String, String) {
    let trimmed = line.trimmingCharacters(in: .whitespacesAndNewlines)
    if trimmed.isEmpty { return ("", "") }
    let parts = trimmed.split(whereSeparator: { $0.isWhitespace })
    if parts.count >= 2 {
        return (String(parts.first!), String(parts.last!))
    }
    let chars = Array(trimmed)
    return (String(chars.first!), String(chars.last!))
}

func extractLinesFromPage(_ page: PDFPage, pageHeight: Double) -> [LineRecord] {
    let mediaBounds = page.bounds(for: .mediaBox)
    guard let fullSel = page.selection(for: mediaBounds) else { return [] }
    let lineSels = fullSel.selectionsByLine()
    if lineSels.isEmpty { return [] }

    struct SegItem {
        let text: String
        let rect: CGRect
        let yTop: Double
        let yBottom: Double
        let yMid: Double
    }

    var segs: [SegItem] = []
    for sel in lineSels {
        guard let raw = sel.string else { continue }
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty { continue }
        let b = sel.bounds(for: page)
        if b.width <= 0.1 || b.height <= 0.1 { continue }
        let yTop = pageHeight - Double(b.maxY)
        let yBottom = pageHeight - Double(b.minY)
        segs.append(SegItem(text: trimmed, rect: b, yTop: yTop, yBottom: yBottom, yMid: 0.5 * (yTop + yBottom)))
    }

    if segs.isEmpty { return [] }

    // Sort segments top-to-bottom by yMid, then left-to-right by minX
    let sortedSegs = segs.sorted { a, b in
        if abs(a.yMid - b.yMid) > 4.5 {
            return a.yMid < b.yMid
        }
        return a.rect.minX < b.rect.minX
    }

    // Merge segments that sit on the same horizontal line (within 4.8pt yMid / yBottom)
    var rawLines: [[SegItem]] = []
    for seg in sortedSegs {
        var matchedIdx: Int? = nil
        for idx in stride(from: rawLines.count - 1, through: max(0, rawLines.count - 4), by: -1) {
            let line = rawLines[idx]
            let avgMid = line.reduce(0.0) { $0 + $1.yMid } / Double(line.count)
            let avgBottom = line.reduce(0.0) { $0 + $1.yBottom } / Double(line.count)
            if abs(seg.yMid - avgMid) <= 4.8 || abs(seg.yBottom - avgBottom) <= 4.5 {
                matchedIdx = idx
                break
            }
        }
        if let idx = matchedIdx {
            rawLines[idx].append(seg)
        } else {
            rawLines.append([seg])
        }
    }

    rawLines.sort { a, b in
        let ya = a.reduce(0.0) { $0 + $1.yBottom } / Double(a.count)
        let yb = b.reduce(0.0) { $0 + $1.yBottom } / Double(b.count)
        return ya < yb
    }

    var out: [LineRecord] = []
    for (idx, lineSegs) in rawLines.enumerated() {
        let xSorted = lineSegs.sorted { $0.rect.minX < $1.rect.minX }
        var built = ""
        var prevMaxX: CGFloat? = nil
        for s in xSorted {
            if let px = prevMaxX {
                let gap = s.rect.minX - px
                if gap > 1.2 && !built.hasSuffix(" ") {
                    built.append(" ")
                }
            }
            built.append(s.text)
            prevMaxX = max(prevMaxX ?? s.rect.maxX, s.rect.maxX)
        }
        let norm = normalizeText(built)
        if norm.isEmpty { continue }
        let minX = xSorted.map { Double($0.rect.minX) }.min() ?? 0.0
        let maxX = xSorted.map { Double($0.rect.maxX) }.max() ?? minX
        let minY = xSorted.map { Double($0.rect.minY) }.min() ?? 0.0
        let maxY = xSorted.map { Double($0.rect.maxY) }.max() ?? minY
        let yBaseline = pageHeight - 0.5 * (minY + maxY)
        let (ft, lt) = extractTokens(norm)
        out.append(LineRecord(
            lineIndex: idx,
            text: norm,
            firstToken: ft,
            lastToken: lt,
            xPt: (minX * 100.0).rounded() / 100.0,
            yTopPt: (yBaseline * 100.0).rounded() / 100.0,
            widthPt: ((maxX - minX) * 100.0).rounded() / 100.0,
            heightPt: ((maxY - minY) * 100.0).rounded() / 100.0
        ))
    }
    return out
}

func renderPageToRGBA(_ cgPage: CGPDFPage?, widthPt: Double, heightPt: Double, scale: Double) -> [UInt8] {
    let w = max(1, Int((widthPt * scale).rounded()))
    let h = max(1, Int((heightPt * scale).rounded()))
    var buf = [UInt8](repeating: 255, count: w * h * 4)
    let cs = CGColorSpaceCreateDeviceRGB()
    buf.withUnsafeMutableBytes { rawPtr in
        guard let base = rawPtr.baseAddress,
              let ctx = CGContext(
                  data: base,
                  width: w,
                  height: h,
                  bitsPerComponent: 8,
                  bytesPerRow: w * 4,
                  space: cs,
                  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
              ) else { return }
        ctx.setFillColor(red: 1, green: 1, blue: 1, alpha: 1)
        ctx.fill(CGRect(x: 0, y: 0, width: w, height: h))
        if let page = cgPage {
            ctx.saveGState()
            ctx.scaleBy(x: CGFloat(scale), y: CGFloat(scale))
            ctx.drawPDFPage(page)
            ctx.restoreGState()
        }
    }
    return buf
}

func computeInkBounds(_ rgba: [UInt8], widthPt: Double, heightPt: Double, scale: Double) -> (Double, Double, Double, Double) {
    let w = max(1, Int((widthPt * scale).rounded()))
    let h = max(1, Int((heightPt * scale).rounded()))
    var minX = w, maxX = 0, minY = h, maxY = 0
    var found = false
    for y in 0..<h {
        let rowOffset = y * w * 4
        for x in 0..<w {
            let idx = rowOffset + x * 4
            let r = Int(rgba[idx])
            let g = Int(rgba[idx + 1])
            let b = Int(rgba[idx + 2])
            if r < 245 || g < 245 || b < 245 {
                found = true
                if x < minX { minX = x }
                if x > maxX { maxX = x }
                if y < minY { minY = y }
                if y > maxY { maxY = y }
            }
        }
    }
    if !found { return (0, 0, 0, 0) }
    return (
        Double(minX) / scale,
        Double(maxX) / scale,
        Double(minY) / scale,
        Double(maxY) / scale
    )
}

func compareLines(_ tLines: [LineRecord], _ wLines: [LineRecord]) -> ([LineDiffRecord], Int, [Double], [Double]) {
    let n = tLines.count
    let m = wLines.count
    // LCS matching where two lines pair if they share identical normalized text OR share first/last token OR have >=65% character overlap
    func canPair(_ a: LineRecord, _ b: LineRecord) -> Bool {
        if a.text == b.text { return true }
        if a.firstToken == b.firstToken || a.lastToken == b.lastToken { return true }
        let minLen = min(a.text.count, b.text.count)
        if minLen >= 6 {
            let prefix = a.text.commonPrefix(with: b.text)
            if prefix.count * 2 >= minLen { return true }
        }
        return false
    }

    var dp = Array(repeating: Array(repeating: 0, count: m + 1), count: n + 1)
    if n > 0 && m > 0 {
        for i in stride(from: n - 1, through: 0, by: -1) {
            for j in stride(from: m - 1, through: 0, by: -1) {
                if canPair(tLines[i], wLines[j]) {
                    dp[i][j] = 1 + dp[i + 1][j + 1]
                } else {
                    dp[i][j] = max(dp[i + 1][j], dp[i][j + 1])
                }
            }
        }
    }

    var i = 0, j = 0, idx = 0
    var exactCount = 0
    var dyList: [Double] = []
    var dwList: [Double] = []
    var diffs: [LineDiffRecord] = []

    while i < n || j < m {
        if i < n && j < m && canPair(tLines[i], wLines[j]) {
            let tl = tLines[i]
            let wl = wLines[j]
            let dy = ((wl.yTopPt - tl.yTopPt) * 100.0).rounded() / 100.0
            let dw = ((wl.widthPt - tl.widthPt) * 100.0).rounded() / 100.0
            dyList.append(abs(dy))
            dwList.append(abs(dw))
            let isExactText = (tl.text.replacingOccurrences(of: " ", with: "") == wl.text.replacingOccurrences(of: " ", with: ""))
            if isExactText {
                exactCount += 1
            }
            let status = isExactText ? (abs(dy) <= 4.0 ? "EXACT" : "DRIFT") : "WORD_BOUNDARY_DIFF"
            if status != "EXACT" {
                diffs.append(LineDiffRecord(
                    index: idx,
                    status: status,
                    typstText: tl.text,
                    wordText: wl.text,
                    typstYTopPt: tl.yTopPt,
                    wordYTopPt: wl.yTopPt,
                    deltaYPt: dy,
                    deltaWidthPt: dw
                ))
            }
            i += 1
            j += 1
            idx += 1
        } else if j < m && (i == n || dp[i][j + 1] >= dp[i + 1][j]) {
            let wl = wLines[j]
            diffs.append(LineDiffRecord(
                index: idx,
                status: "WORD_ONLY",
                typstText: nil,
                wordText: wl.text,
                typstYTopPt: nil,
                wordYTopPt: wl.yTopPt,
                deltaYPt: nil,
                deltaWidthPt: nil
            ))
            j += 1
            idx += 1
        } else {
            let tl = tLines[i]
            diffs.append(LineDiffRecord(
                index: idx,
                status: "TYPST_ONLY",
                typstText: tl.text,
                wordText: nil,
                typstYTopPt: tl.yTopPt,
                wordYTopPt: nil,
                deltaYPt: nil,
                deltaWidthPt: nil
            ))
            i += 1
            idx += 1
        }
    }
    return (diffs, exactCount, dyList, dwList)
}

func buildOverlayPageImage(
    tRGBA: [UInt8],
    wRGBA: [UInt8],
    widthPt: Double,
    heightPt: Double,
    scale: Double
) -> (CGImage?, Double) {
    let w = max(1, Int((widthPt * scale).rounded()))
    let h = max(1, Int((heightPt * scale).rounded()))
    var out = [UInt8](repeating: 255, count: w * h * 4)

    // Dilate mask by 2px radius so sub-point font rendering differences that overlap within ~1pt count as overlapping
    var tMask = [Bool](repeating: false, count: w * h)
    var wMask = [Bool](repeating: false, count: w * h)
    for idx in 0..<(w * h) {
        let p = idx * 4
        tMask[idx] = (Int(tRGBA[p]) + Int(tRGBA[p + 1]) + Int(tRGBA[p + 2])) < 720
        wMask[idx] = (Int(wRGBA[p]) + Int(wRGBA[p + 1]) + Int(wRGBA[p + 2])) < 720
    }

    var overlapInk = 0
    var unionInk = 0
    let r = 2
    for y in 0..<h {
        for x in 0..<w {
            let i = y * w + x
            let hasT = tMask[i]
            let hasW = wMask[i]
            if hasT || hasW {
                unionInk += 1
                var nearOther = false
                let y0 = max(0, y - r), y1 = min(h - 1, y + r)
                let x0 = max(0, x - r), x1 = min(w - 1, x + r)
                if hasT && hasW {
                    nearOther = true
                } else if hasT {
                    outerT: for yy in y0...y1 {
                        for xx in x0...x1 {
                            if wMask[yy * w + xx] { nearOther = true; break outerT }
                        }
                    }
                } else {
                    outerW: for yy in y0...y1 {
                        for xx in x0...x1 {
                            if tMask[yy * w + xx] { nearOther = true; break outerW }
                        }
                    }
                }
                if nearOther { overlapInk += 1 }
            }

            let p = i * 4
            let tDark = 1.0 - (Double(tRGBA[p]) + Double(tRGBA[p + 1]) + Double(tRGBA[p + 2])) / (3.0 * 255.0)
            let wDark = 1.0 - (Double(wRGBA[p]) + Double(wRGBA[p + 1]) + Double(wRGBA[p + 2])) / (3.0 * 255.0)

            if tDark < 0.03 && wDark < 0.03 {
                out[p] = 255; out[p + 1] = 255; out[p + 2] = 255; out[p + 3] = 255
            } else if tDark >= 0.03 && wDark >= 0.03 {
                // Both Typst & Word have ink here -> render as deep charcoal (#1D1D1F)
                let d = min(1.0, (tDark + wDark) * 0.65)
                let v = UInt8(max(20.0, 255.0 * (1.0 - d)))
                out[p] = v; out[p + 1] = v; out[p + 2] = min(255, v + 5); out[p + 3] = 255
            } else if tDark >= 0.03 {
                // Typst-only ink -> Apple Blue (#007AFF)
                out[p]     = UInt8(255.0 * (1.0 - 1.00 * tDark))
                out[p + 1] = UInt8(255.0 * (1.0 - 0.52 * tDark))
                out[p + 2] = 255
                out[p + 3] = 255
            } else {
                // Word-only ink -> Apple Red/Pink (#FF2D55)
                out[p]     = 255
                out[p + 1] = UInt8(255.0 * (1.0 - 0.82 * wDark))
                out[p + 2] = UInt8(255.0 * (1.0 - 0.67 * wDark))
                out[p + 3] = 255
            }
        }
    }

    let rate = unionInk > 0 ? Double(overlapInk) / Double(unionInk) : 1.0
    let cs = CGColorSpaceCreateDeviceRGB()
    var cgImg: CGImage? = nil
    out.withUnsafeMutableBytes { rawPtr in
        guard let base = rawPtr.baseAddress,
              let ctx = CGContext(
                  data: base,
                  width: w,
                  height: h,
                  bitsPerComponent: 8,
                  bytesPerRow: w * 4,
                  space: cs,
                  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
              ) else { return }
        cgImg = ctx.makeImage()
    }
    return (cgImg, (rate * 10000.0).rounded() / 10000.0)
}

let args = CommandLine.arguments
if args.count >= 4 && args[1] == "--merge" {
    let outPath = args[2]
    let masterDoc = PDFDocument()
    var pageInsertIdx = 0
    for inPath in args[3...] {
        if let doc = PDFDocument(url: URL(fileURLWithPath: inPath)) {
            for i in 0..<doc.pageCount {
                if let pg = doc.page(at: i) {
                    masterDoc.insert(pg, at: pageInsertIdx)
                    pageInsertIdx += 1
                }
            }
        }
    }
    masterDoc.write(to: URL(fileURLWithPath: outPath))
    exit(0)
}

if args.count < 3 {
    fputs("Usage: pdf_inspector <typst.pdf> <word.pdf> [--overlay <out.overlay.pdf>] [--title <label>]\n", stderr)
    exit(1)
}

let typstPath = args[1]
let wordPath = args[2]
var overlayPath: String? = nil
var titleLabel: String = URL(fileURLWithPath: typstPath).lastPathComponent
var argIdx = 3
while argIdx < args.count {
    if args[argIdx] == "--overlay" && argIdx + 1 < args.count {
        overlayPath = args[argIdx + 1]
        argIdx += 2
    } else if args[argIdx] == "--title" && argIdx + 1 < args.count {
        titleLabel = args[argIdx + 1]
        argIdx += 2
    } else {
        argIdx += 1
    }
}

guard let tDoc = PDFDocument(url: URL(fileURLWithPath: typstPath)),
      let wDoc = PDFDocument(url: URL(fileURLWithPath: wordPath)),
      let tCgDoc = CGPDFDocument(URL(fileURLWithPath: typstPath) as CFURL),
      let wCgDoc = CGPDFDocument(URL(fileURLWithPath: wordPath) as CFURL) else {
    fputs("Error: could not open input PDFs\n", stderr)
    exit(1)
}

let tPages = tDoc.pageCount
let wPages = wDoc.pageCount
let maxPages = max(tPages, wPages)
let scale = 2.0

var pageComparisons: [PageComparison] = []
var totalTLines = 0
var totalWLines = 0
var totalExactLines = 0
var allDy: [Double] = []

var pdfCtx: CGContext? = nil
if let ovPath = overlayPath {
    var mediaBox = CGRect(x: 0, y: 0, width: 595.28, height: 841.89)
    pdfCtx = CGContext(URL(fileURLWithPath: ovPath) as CFURL, mediaBox: &mediaBox, nil)
}

for pIdx in 0..<maxPages {
    let tPage = pIdx < tPages ? tDoc.page(at: pIdx) : nil
    let wPage = pIdx < wPages ? wDoc.page(at: pIdx) : nil
    let tCgPage = pIdx < tPages ? tCgDoc.page(at: pIdx + 1) : nil
    let wCgPage = pIdx < wPages ? wCgDoc.page(at: pIdx + 1) : nil

    let bounds = tPage?.bounds(for: .mediaBox) ?? wPage?.bounds(for: .mediaBox) ?? CGRect(x: 0, y: 0, width: 595.28, height: 841.89)
    let widthPt = Double(bounds.width)
    let heightPt = Double(bounds.height)

    let tLines = tPage != nil ? extractLinesFromPage(tPage!, pageHeight: heightPt) : []
    let wLines = wPage != nil ? extractLinesFromPage(wPage!, pageHeight: heightPt) : []

    totalTLines += tLines.count
    totalWLines += wLines.count

    let (diffs, exactCount, dyList, dwList) = compareLines(tLines, wLines)
    totalExactLines += exactCount
    allDy.append(contentsOf: dyList)

    let tRGBA = renderPageToRGBA(tCgPage, widthPt: widthPt, heightPt: heightPt, scale: scale)
    let wRGBA = renderPageToRGBA(wCgPage, widthPt: widthPt, heightPt: heightPt, scale: scale)
    let (tMinX, tMaxX, tMinY, tMaxY) = computeInkBounds(tRGBA, widthPt: widthPt, heightPt: heightPt, scale: scale)
    let (wMinX, wMaxX, wMinY, wMaxY) = computeInkBounds(wRGBA, widthPt: widthPt, heightPt: heightPt, scale: scale)
    let (ovImg, pxOverlap) = buildOverlayPageImage(tRGBA: tRGBA, wRGBA: wRGBA, widthPt: widthPt, heightPt: heightPt, scale: scale)

    let denom = max(1, max(tLines.count, wLines.count))
    let parity = Double(exactCount) / Double(denom)
    let meanDy = dyList.isEmpty ? 0.0 : (dyList.reduce(0.0, +) / Double(dyList.count))
    let maxDy = dyList.max() ?? 0.0
    let meanDw = dwList.isEmpty ? 0.0 : (dwList.reduce(0.0, +) / Double(dwList.count))

    pageComparisons.append(PageComparison(
        pageNumber: pIdx + 1,
        typstLineCount: tLines.count,
        wordLineCount: wLines.count,
        exactLineMatches: exactCount,
        lineParityRate: (parity * 10000.0).rounded() / 10000.0,
        meanAbsDeltaYPt: (meanDy * 100.0).rounded() / 100.0,
        maxAbsDeltaYPt: (maxDy * 100.0).rounded() / 100.0,
        meanAbsDeltaWidthPt: (meanDw * 100.0).rounded() / 100.0,
        inkTopDeltaPt: ((wMinY - tMinY) * 100.0).rounded() / 100.0,
        inkBottomDeltaPt: ((wMaxY - tMaxY) * 100.0).rounded() / 100.0,
        inkLeftDeltaPt: ((wMinX - tMinX) * 100.0).rounded() / 100.0,
        inkRightDeltaPt: ((wMaxX - tMaxX) * 100.0).rounded() / 100.0,
        pixelOverlapRate: pxOverlap,
        lineDiffs: diffs
    ))

    if let ctx = pdfCtx, let img = ovImg {
        let bannerH: CGFloat = 28.0
        let pageRect = CGRect(x: 0, y: 0, width: CGFloat(widthPt), height: CGFloat(heightPt) + bannerH)
        let pageInfo = [kCGPDFContextMediaBox as String: NSValue(rect: NSRectFromCGRect(pageRect))] as CFDictionary
        ctx.beginPDFPage(pageInfo)
        ctx.draw(img, in: CGRect(x: 0, y: 0, width: CGFloat(widthPt), height: CGFloat(heightPt)))

        // Draw top telemetry banner
        ctx.setFillColor(red: 0.96, green: 0.96, blue: 0.97, alpha: 1.0)
        ctx.fill(CGRect(x: 0, y: CGFloat(heightPt), width: CGFloat(widthPt), height: bannerH))
        ctx.setStrokeColor(red: 0.82, green: 0.82, blue: 0.84, alpha: 1.0)
        ctx.setLineWidth(0.8)
        ctx.move(to: CGPoint(x: 0, y: CGFloat(heightPt)))
        ctx.addLine(to: CGPoint(x: CGFloat(widthPt), y: CGFloat(heightPt)))
        ctx.strokePath()

        let infoStr = String(
            format: "[%@]  Page %d/%d  |  Lines: T=%d W=%d (Exact=%d, %.1f%%)  |  Mean Δy=%.2fpt  |  Black=Overlap  Blue=Typst  Red=Word",
            titleLabel, pIdx + 1, maxPages, tLines.count, wLines.count, exactCount, parity * 100.0, meanDy
        )
        let nsGraphic = NSGraphicsContext(cgContext: ctx, flipped: false)
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = nsGraphic
        let attrs: [NSAttributedString.Key: Any] = [
            .font: NSFont.monospacedSystemFont(ofSize: 8.5, weight: .semibold),
            .foregroundColor: NSColor(calibratedRed: 0.11, green: 0.11, blue: 0.12, alpha: 1.0)
        ]
        (infoStr as NSString).draw(at: NSPoint(x: 14, y: CGFloat(heightPt) + 8), withAttributes: attrs)
        NSGraphicsContext.restoreGraphicsState()

        ctx.endPDFPage()
    }
}

pdfCtx?.closePDF()

let overallDenom = max(1, max(totalTLines, totalWLines))
let overallParity = Double(totalExactLines) / Double(overallDenom)
let overallMeanDy = allDy.isEmpty ? 0.0 : (allDy.reduce(0.0, +) / Double(allDy.count))
let overallMaxDy = allDy.max() ?? 0.0

let report = ComparisonReport(
    typstPdfPath: typstPath,
    wordPdfPath: wordPath,
    overlayPdfPath: overlayPath,
    typstPageCount: tPages,
    wordPageCount: wPages,
    pageCountMatch: tPages == wPages,
    totalTypstLines: totalTLines,
    totalWordLines: totalWLines,
    totalExactLineMatches: totalExactLines,
    overallLineParityRate: (overallParity * 10000.0).rounded() / 10000.0,
    overallMeanAbsDeltaYPt: (overallMeanDy * 100.0).rounded() / 100.0,
    overallMaxAbsDeltaYPt: (overallMaxDy * 100.0).rounded() / 100.0,
    pages: pageComparisons
)

let encoder = JSONEncoder()
encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
if let data = try? encoder.encode(report), let s = String(data: data, encoding: .utf8) {
    print(s)
}

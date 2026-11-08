import Foundation
import PDFKit
import AppKit
import CoreGraphics

struct SpanBox: Codable {
    let page: Int
    let kind: String // "TEXT" or "GRAPHIC"
    let text: String
    let normText: String
    // PDF selection bounds (top-down pt)
    let pdfLeft: Double
    let pdfTop: Double
    let pdfRight: Double
    let pdfBottom: Double
    // Actual rendered ink bounds (top-down pt)
    let inkLeft: Double
    let inkTop: Double
    let inkRight: Double
    let inkBottom: Double

    var inkWidth: Double { max(0.0, inkRight - inkLeft) }
    var inkHeight: Double { max(0.0, inkBottom - inkTop) }
    var inkMidY: Double { (inkTop + inkBottom) * 0.5 }
}

struct PageBitmap {
    let widthPt: Double
    let heightPt: Double
    let scale: Double
    let widthPx: Int
    let heightPx: Int
    let pixels: [UInt8] // RGBA

    func isInk(px: Int, py: Int) -> Bool {
        if px < 0 || px >= widthPx || py < 0 || py >= heightPx { return false }
        let idx = (py * widthPx + px) * 4
        let r = Int(pixels[idx])
        let g = Int(pixels[idx + 1])
        let b = Int(pixels[idx + 2])
        return min(r, min(g, b)) < 235
    }

    /// Refine a rectangle (top-down pt) to tight ink bounds (top-down pt).
    func refineInkBounds(leftPt: Double, topPt: Double, rightPt: Double, bottomPt: Double) -> (Double, Double, Double, Double)? {
        let x0 = max(0, Int(Foundation.floor(leftPt * scale)))
        let x1 = min(widthPx - 1, Int(Foundation.ceil(rightPt * scale)))
        let y0 = max(0, Int(Foundation.floor(topPt * scale)))
        let y1 = min(heightPx - 1, Int(Foundation.ceil(bottomPt * scale)))
        if x1 < x0 || y1 < y0 {
            return nil
        }

        var minX = Int.max
        var maxX = Int.min
        var minY = Int.max
        var maxY = Int.min

        for py in y0...y1 {
            for px in x0...x1 {
                if isInk(px: px, py: py) {
                    if px < minX { minX = px }
                    if px > maxX { maxX = px }
                    if py < minY { minY = py }
                    if py > maxY { maxY = py }
                }
            }
        }

        if minX == Int.max {
            return nil
        }
        return (
            Double(minX) / scale,
            Double(minY) / scale,
            Double(maxX + 1) / scale,
            Double(maxY + 1) / scale
        )
    }

    /// Split a horizontal band `[leftPt..rightPt, topPt..bottomPt]` into horizontal segments
    /// separated by >= `minGapPt` of empty white columns (including underlines in `[topPt..bottomPt]`).
    func splitHorizontalInkSegments(
        leftPt: Double,
        topPt: Double,
        rightPt: Double,
        bottomPt: Double,
        minGapPt: Double = 26.0
    ) -> [(Double, Double)] {
        let x0 = max(0, Int(Foundation.floor(leftPt * scale)))
        let x1 = min(widthPx - 1, Int(Foundation.ceil(rightPt * scale)))
        let y0 = max(0, Int(Foundation.floor(topPt * scale)))
        let y1 = min(heightPx - 1, Int(Foundation.ceil(bottomPt * scale)))
        if x1 <= x0 || y1 < y0 {
            return [(leftPt, rightPt)]
        }

        let minGapPx = max(2, Int(Foundation.round(minGapPt * scale)))
        var segments: [(Int, Int)] = []
        var inSeg = false
        var segStart = x0
        var lastInkX = x0

        for px in x0...x1 {
            var colHasInk = false
            for py in y0...y1 {
                if isInk(px: px, py: py) {
                    colHasInk = true
                    break
                }
            }
            if colHasInk {
                if !inSeg {
                    inSeg = true
                    segStart = px
                }
                lastInkX = px
            } else if inSeg && (px - lastInkX) >= minGapPx {
                segments.append((segStart, lastInkX))
                inSeg = false
            }
        }
        if inSeg {
            segments.append((segStart, lastInkX))
        }
        if segments.isEmpty {
            return [(leftPt, rightPt)]
        }
        return segments.map { (Double($0.0) / scale, Double($0.1 + 1) / scale) }
    }
}

func renderPageBitmap(page: PDFPage, scale: Double = 2.0) -> PageBitmap {
    let mediaBox = page.bounds(for: .mediaBox)
    let widthPt = Double(mediaBox.width)
    let heightPt = Double(mediaBox.height)
    let widthPx = max(1, Int((widthPt * scale).rounded()))
    let heightPx = max(1, Int((heightPt * scale).rounded()))
    let bytesPerRow = widthPx * 4
    var rawData = [UInt8](repeating: 255, count: heightPx * bytesPerRow)
    let colorSpace = CGColorSpaceCreateDeviceRGB()

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

    return PageBitmap(
        widthPt: widthPt,
        heightPt: heightPt,
        scale: scale,
        widthPx: widthPx,
        heightPx: heightPx,
        pixels: rawData
    )
}

func normalizeForMatch(_ s: String) -> String {
    var out = ""
    for ch in s.unicodeScalars {
        if CharacterSet.whitespacesAndNewlines.contains(ch) { continue }
        if ch == "_" || ch == "\u{200B}" || ch == "\u{00A0}" { continue }
        if ch == "‣" || ch == "◦" || ch == "▪" {
            out.unicodeScalars.append("•")
            continue
        }
        out.unicodeScalars.append(ch)
    }
    return out
}

func extractPageSpans(page: PDFPage, pageNum: Int, bitmap: PageBitmap) -> [SpanBox] {
    let mediaBox = page.bounds(for: .mediaBox)
    let pageH = Double(mediaBox.height)
    guard let fullSel = page.selection(for: mediaBox) else { return [] }
    let lineSels = fullSel.selectionsByLine()

    var rawSpans: [(text: String, left: Double, top: Double, right: Double, bottom: Double)] = []

    for lineSel in lineSels {
        guard let lineStr = lineSel.string else { continue }
        let trimmed = lineStr.trimmingCharacters(in: CharacterSet.whitespacesAndNewlines)
        if trimmed.isEmpty { continue }
        let b = lineSel.bounds(for: page)
        if b.width <= 0 || b.height <= 0 { continue }

        let top = pageH - Double(b.maxY)
        let bottom = pageH - Double(b.minY)
        let left = Double(b.minX)
        let right = Double(b.maxX)

        // Exclude table borders just outside the glyph box when splitting columns;
        // fill-in-the-blank underlines are re-bridged below via `connectedByUnderline`.
        let h = bottom - top
        let gapScanTop = h > 14.0 ? top + 1.5 : top + 0.5
        let gapScanBot = max(gapScanTop + 2.0, bottom - 1.5)

        let xSegs = bitmap.splitHorizontalInkSegments(
            leftPt: left,
            topPt: gapScanTop,
            rightPt: right,
            bottomPt: gapScanBot,
            minGapPt: 26.0
        )

        if xSegs.count <= 1 {
            rawSpans.append((trimmed, left, top, right, bottom))
        } else {
            for (segL, segR) in xSegs {
                let subRect = CGRect(
                    x: segL - 1.5,
                    y: Double(b.minY) + (h > 14.0 ? 1.5 : 0.0),
                    width: (segR - segL) + 3.0,
                    height: max(2.0, Double(b.height) - (h > 14.0 ? 3.0 : 0.0))
                )
                let subText = page.selection(for: subRect)?.string?.replacingOccurrences(of: "\n", with: " ").trimmingCharacters(in: CharacterSet.whitespacesAndNewlines) ?? ""
                if !subText.isEmpty {
                    rawSpans.append((subText, segL, top, segR, bottom))
                }
            }
        }
    }

    // Sort top-to-bottom, then left-to-right
    rawSpans.sort { a, b in
        let midA = (a.top + a.bottom) * 0.5
        let midB = (b.top + b.bottom) * 0.5
        if abs(midA - midB) > 4.5 { return midA < midB }
        return a.left < b.left
    }

    // Merge spans on the same baseline with small horizontal gap (<= 22pt), e.g. list label "1." + item text,
    // or connected by an underline in `[top..bottom+2pt]`.
    var merged: [(text: String, left: Double, top: Double, right: Double, bottom: Double)] = []
    for sp in rawSpans {
        if let last = merged.last {
            let midLast = (last.top + last.bottom) * 0.5
            let midCur = (sp.top + sp.bottom) * 0.5
            let gapX = sp.left - last.right
            if abs(midLast - midCur) <= 4.5 && abs(sp.left - last.left) < 2.0 && sp.text == last.text {
                continue
            }
            var connectedByUnderline = false
            if abs(midLast - midCur) <= 4.5 && gapX > 22.0 && gapX <= 120.0 {
                // Ensure no vertical table border crosses the upper half of the gap
                let upperInk = bitmap.refineInkBounds(
                    leftPt: last.right + 2.0,
                    topPt: min(last.top, sp.top) + 1.0,
                    rightPt: sp.left - 2.0,
                    bottomPt: (midLast + midCur) * 0.5
                )
                if upperInk == nil {
                    // Check if there is a continuous underline bridging `last.right .. sp.left` near `bottom`
                    let bridgeSegs = bitmap.splitHorizontalInkSegments(
                        leftPt: max(0.0, last.right - 2.0),
                        topPt: min(last.top, sp.top),
                        rightPt: sp.left + 2.0,
                        bottomPt: max(last.bottom, sp.bottom) + 2.5,
                        minGapPt: 6.0
                    )
                    if bridgeSegs.count == 1 {
                        let (bL, bR) = bridgeSegs[0]
                        if bL <= last.right + 4.0 && bR >= sp.left - 4.0 {
                            connectedByUnderline = true
                        }
                    }
                }
            }
            if abs(midLast - midCur) <= 4.5 && ((gapX >= -4.0 && gapX <= 22.0) || connectedByUnderline) {
                let sep = gapX > 2.5 ? " " : ""
                merged[merged.count - 1] = (
                    last.text + sep + sp.text,
                    min(last.left, sp.left),
                    min(last.top, sp.top),
                    max(last.right, sp.right),
                    max(last.bottom, sp.bottom)
                )
                continue
            }
        }
        merged.append(sp)
    }

    var result: [SpanBox] = []
    for m in merged {
        let norm = normalizeForMatch(m.text)
        if norm.isEmpty { continue }
        let h = m.bottom - m.top
        let searchTop = h > 14.0 ? m.top + 1.5 : m.top
        let searchBot = h > 14.0 ? m.bottom - 1.2 : m.bottom + 1.5
        let (iL, iT, iR, iB) = bitmap.refineInkBounds(
            leftPt: m.left,
            topPt: searchTop,
            rightPt: m.right,
            bottomPt: searchBot
        ) ?? (m.left, m.top, m.right, m.bottom)

        result.append(SpanBox(
            page: pageNum,
            kind: "TEXT",
            text: m.text,
            normText: norm,
            pdfLeft: m.left,
            pdfTop: m.top,
            pdfRight: m.right,
            pdfBottom: m.bottom,
            inkLeft: iL,
            inkTop: iT,
            inkRight: iR,
            inkBottom: iB
        ))
    }
    return result
}

func charSimilarity(_ a: String, _ b: String) -> Double {
    if a == b { return 1.0 }
    if a.isEmpty || b.isEmpty { return 0.0 }
    let minLen = Double(min(a.count, b.count))
    let maxLen = Double(max(a.count, b.count))
    if (a.contains(b) || b.contains(a)) && minLen >= 2.0 && (minLen / maxLen) >= 0.35 {
        return 0.65 + 0.35 * (minLen / maxLen)
    }
    let setA = Set(a)
    let setB = Set(b)
    let inter = Double(setA.intersection(setB).count)
    let union = Double(setA.union(setB).count)
    return union > 0 ? inter / union : 0.0
}

guard CommandLine.arguments.count >= 3 else {
    fputs("Usage: swift pdf_layout_diff.swift <orig.pdf> <word.pdf> [out_report.txt] [max_pages=0]\n", stderr)
    exit(1)
}

let origPath = CommandLine.arguments[1]
let wordPath = CommandLine.arguments[2]
let reportPath = CommandLine.arguments.count >= 4 ? CommandLine.arguments[3] : nil
let maxPagesArg = CommandLine.arguments.count >= 5 ? (Int(CommandLine.arguments[4]) ?? 0) : 0

guard let origDoc = PDFDocument(url: URL(fileURLWithPath: origPath)) else {
    fputs("Error: failed to open \(origPath)\n", stderr)
    exit(1)
}
guard let wordDoc = PDFDocument(url: URL(fileURLWithPath: wordPath)) else {
    fputs("Error: failed to open \(wordPath)\n", stderr)
    exit(1)
}

let origPages = origDoc.pageCount
let wordPages = wordDoc.pageCount
let maxPages = maxPagesArg > 0 ? min(maxPagesArg, max(origPages, wordPages)) : max(origPages, wordPages)

var origSpansByPage: [Int: [SpanBox]] = [:]
var wordSpansByPage: [Int: [SpanBox]] = [:]

for p in 1...maxPages {
    if p <= origPages, let page = origDoc.page(at: p - 1) {
        let bmp = renderPageBitmap(page: page, scale: 2.0)
        origSpansByPage[p] = extractPageSpans(page: page, pageNum: p, bitmap: bmp)
    }
    if p <= wordPages, let page = wordDoc.page(at: p - 1) {
        let bmp = renderPageBitmap(page: page, scale: 2.0)
        wordSpansByPage[p] = extractPageSpans(page: page, pageNum: p, bitmap: bmp)
    }
}

var lines: [String] = []
var summaryLines: [String] = []
lines.append("==========================================================================================")
lines.append(String(format: "PDF COORDINATE DIFF REPORT: orig_pages=%d, word_pages=%d (units: pt, origin: top-left)", origPages, wordPages))
lines.append("Coordinates use rendered glyph INK bounds (inkTop..inkBottom, inkLeft..inkRight).")
lines.append("Deltas: dY = word.inkTop - orig.inkTop (+ means Word is lower), dX = word.inkLeft - orig.inkLeft")
lines.append("==========================================================================================")

for p in 1...maxPages {
    let oSpans = origSpansByPage[p] ?? []
    let wSpans = wordSpansByPage[p] ?? []
    var usedW = Set<Int>()

    lines.append("")
    lines.append(String(format: "---------------------------------------- PAGE %d ----------------------------------------", p))
    lines.append(String(format: "orig_spans=%d, word_spans=%d", oSpans.count, wSpans.count))
    lines.append("STATUS     | dY(pt)  dX(pt)  dW(pt) | ORIG [top..bot, left..right]     | WORD [top..bot, left..right]     | TEXT")
    lines.append("-----------+------------------------+----------------------------------+----------------------------------+------------------------------")

    var alignedCount = 0
    var shiftedCount = 0
    var missingCount = 0
    var maxAbsDy = 0.0
    var maxAbsDx = 0.0

    for o in oSpans {
        var bestIdx: Int? = nil
        var bestScore = -1.0

        for (wi, w) in wSpans.enumerated() {
            if usedW.contains(wi) { continue }
            let sim = charSimilarity(o.normText, w.normText)
            let distY = abs(o.inkTop - w.inkTop)
            let distX = abs(o.inkLeft - w.inkLeft)
            // Fallback for short math/symbol labels where Word PDF subsetting alters ToUnicode
            let spatialShortMatch = (o.normText.count <= 4 && w.normText.count <= 4 && distY <= 6.0 && distX <= 8.0)
            if sim < 0.45 && !spatialShortMatch { continue }
            let effectiveSim = spatialShortMatch ? max(sim, 0.75) : sim
            let posPenalty = min(0.4, (distY / 200.0) + (distX / 300.0))
            let score = effectiveSim - posPenalty
            if score > bestScore {
                bestScore = score
                bestIdx = wi
            }
        }

        if let wi = bestIdx {
            usedW.insert(wi)
            let w = wSpans[wi]
            let dy = w.inkTop - o.inkTop
            let dx = w.inkLeft - o.inkLeft
            let dw = w.inkWidth - o.inkWidth
            if abs(dy) > maxAbsDy { maxAbsDy = abs(dy) }
            if abs(dx) > maxAbsDx { maxAbsDx = abs(dx) }

            let isOk = abs(dy) <= 2.0 && abs(dx) <= 2.5
            if isOk {
                alignedCount += 1
            } else {
                shiftedCount += 1
            }
            let status = isOk ? "OK        " : "SHIFT     "
            let shortText = o.text.count > 34 ? String(o.text.prefix(34)) + "…" : o.text
            lines.append(String(
                format: "%@ | %+6.1f  %+6.1f  %+6.1f | [%5.1f..%5.1f, %5.1f..%5.1f] | [%5.1f..%5.1f, %5.1f..%5.1f] | %@",
                status,
                dy, dx, dw,
                o.inkTop, o.inkBottom, o.inkLeft, o.inkRight,
                w.inkTop, w.inkBottom, w.inkLeft, w.inkRight,
                shortText
            ))
        } else {
            var movedPage: Int? = nil
            for adjP in [p - 1, p + 1] {
                if let adjSpans = wordSpansByPage[adjP] {
                    if adjSpans.contains(where: { charSimilarity(o.normText, $0.normText) >= 0.75 }) {
                        movedPage = adjP
                        break
                    }
                }
            }
            missingCount += 1
            let status = movedPage != nil ? String(format: "PAGE->P%-3d", movedPage!) : "ORIG_ONLY "
            let shortText = o.text.count > 34 ? String(o.text.prefix(34)) + "…" : o.text
            lines.append(String(
                format: "%@ |    N/A     N/A     N/A | [%5.1f..%5.1f, %5.1f..%5.1f] | [          ---                 ] | %@",
                status,
                o.inkTop, o.inkBottom, o.inkLeft, o.inkRight,
                shortText
            ))
        }
    }

    for (wi, w) in wSpans.enumerated() where !usedW.contains(wi) {
        let shortText = w.text.count > 34 ? String(w.text.prefix(34)) + "…" : w.text
        lines.append(String(
            format: "WORD_ONLY  |    N/A     N/A     N/A | [          ---                 ] | [%5.1f..%5.1f, %5.1f..%5.1f] | %@",
            w.inkTop, w.inkBottom, w.inkLeft, w.inkRight,
            shortText
        ))
    }

    let sumLine = String(
        format: "PAGE %2d SUMMARY: aligned=%2d, shifted=%2d, unmatched=%2d, max|dY|=%5.1fpt, max|dX|=%5.1fpt",
        p, alignedCount, shiftedCount, missingCount, maxAbsDy, maxAbsDx
    )
    lines.append(sumLine)
    summaryLines.append("  " + sumLine)
}

let output = lines.joined(separator: "\n") + "\n"
if let reportPath = reportPath {
    try output.write(toFile: reportPath, atomically: true, encoding: .utf8)
    for s in summaryLines {
        print(s)
    }
} else {
    print(output)
}

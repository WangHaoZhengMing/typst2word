import Foundation
import PDFKit
import CoreGraphics
import ImageIO
import UniformTypeIdentifiers

struct Manifest: Codable {
    let scale: Double
    let jobs: [InspectJob]
}

struct InspectJob: Codable {
    let id: String
    let typstPdf: String
    let wordPdf: String?
    let outDir: String
}

struct BBoxPt: Codable {
    let leftPt: Double
    let topPt: Double
    let rightPt: Double
    let bottomPt: Double
    let widthPt: Double
    let heightPt: Double
}

struct LineMetric: Codable {
    let page: Int
    let text: String
    let xPt: Double
    let yTopPt: Double
    let widthPt: Double
    let heightPt: Double
}

struct PdfVisualSummary: Codable {
    let pageCount: Int
    let pageWidthPt: Double
    let pageHeightPt: Double
    let inkBBox: BBoxPt?
    let inkPixels: Int
    let lines: [LineMetric]
    let pngPath: String
}

struct PairComparison: Codable {
    let deltaLeftPt: Double
    let deltaTopPt: Double
    let deltaRightPt: Double
    let deltaBottomPt: Double
    let deltaWidthPt: Double
    let deltaHeightPt: Double
    let rawIou: Double
    let alignedIou: Double
    let overlapPixels: Int
    let unionPixels: Int
    let lineCountDelta: Int
    let overlayPngPath: String
}

struct JobOutput: Codable {
    let id: String
    let error: String?
    let typst: PdfVisualSummary?
    let word: PdfVisualSummary?
    let comparison: PairComparison?
}

struct RenderedRaster {
    let widthPx: Int
    let heightPx: Int
    let pageCount: Int
    let firstPageWidthPt: Double
    let firstPageHeightPt: Double
    let rgba: [UInt8]
    let inkMask: [Bool]
    let inkPixels: Int
    let minXPx: Int
    let minYPx: Int
    let maxXPx: Int
    let maxYPx: Int
    let lines: [LineMetric]
}

func round2(_ v: Double) -> Double {
    (v * 100.0).rounded() / 100.0
}

func round4(_ v: Double) -> Double {
    (v * 10000.0).rounded() / 10000.0
}

func extractLines(doc: PDFDocument) -> [LineMetric] {
    var result: [LineMetric] = []
    let pageCount = doc.pageCount
    var yOffsetPt = 0.0
    for idx in 0..<pageCount {
        guard let page = doc.page(at: idx) else { continue }
        let media = page.bounds(for: .mediaBox)
        let pageH = Double(media.height)
        if let fullSel = page.selection(for: media) {
            for lineSel in fullSel.selectionsByLine() {
                let raw = lineSel.string ?? ""
                let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
                if trimmed.isEmpty { continue }
                let b = lineSel.bounds(for: page)
                let xPt = Double(b.origin.x - media.origin.x)
                let yTopLocal = pageH - Double(b.origin.y - media.origin.y + b.height)
                result.append(LineMetric(
                    page: idx + 1,
                    text: trimmed,
                    xPt: round2(xPt),
                    yTopPt: round2(yOffsetPt + yTopLocal),
                    widthPt: round2(Double(b.width)),
                    heightPt: round2(Double(b.height))
                ))
            }
        }
        yOffsetPt += pageH
    }
    return result
}

func renderPdf(path: String, scale: Double) -> RenderedRaster? {
    let url = URL(fileURLWithPath: path)
    guard let doc = PDFDocument(url: url), doc.pageCount > 0 else {
        return nil
    }
    let pageCount = doc.pageCount
    var maxWidthPt = 1.0
    var totalHeightPt = 0.0
    var firstW = 0.0
    var firstH = 0.0
    let gapPt = pageCount > 1 ? 6.0 : 0.0

    for idx in 0..<pageCount {
        guard let page = doc.page(at: idx) else { continue }
        let b = page.bounds(for: .mediaBox)
        let w = Double(b.width)
        let h = Double(b.height)
        if idx == 0 {
            firstW = w
            firstH = h
        }
        if w > maxWidthPt { maxWidthPt = w }
        totalHeightPt += h
        if idx + 1 < pageCount {
            totalHeightPt += gapPt
        }
    }

    let widthPx = max(Int((maxWidthPt * scale).rounded()), 1)
    let heightPx = max(Int((totalHeightPt * scale).rounded()), 1)
    let bytesPerRow = widthPx * 4
    var rgba = [UInt8](repeating: 255, count: heightPx * bytesPerRow)
    let colorSpace = CGColorSpaceCreateDeviceRGB()

    let ok = rgba.withUnsafeMutableBytes { rawBuf -> Bool in
        guard let base = rawBuf.baseAddress else { return false }
        guard let ctx = CGContext(
            data: base,
            width: widthPx,
            height: heightPx,
            bitsPerComponent: 8,
            bytesPerRow: bytesPerRow,
            space: colorSpace,
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ) else {
            return false
        }
        ctx.setFillColor(red: 1.0, green: 1.0, blue: 1.0, alpha: 1.0)
        ctx.fill(CGRect(x: 0, y: 0, width: widthPx, height: heightPx))
        ctx.interpolationQuality = .high
        ctx.setAllowsAntialiasing(true)
        ctx.setShouldAntialias(true)

        var currentTopPt = 0.0
        for idx in 0..<pageCount {
            guard let page = doc.page(at: idx) else { continue }
            let media = page.bounds(for: .mediaBox)
            let pageH = Double(media.height)
            // CoreGraphics origin is bottom-left of bitmap.
            let pageBottomPt = totalHeightPt - (currentTopPt + pageH)

            ctx.saveGState()
            ctx.scaleBy(x: CGFloat(scale), y: CGFloat(scale))
            ctx.translateBy(x: -media.origin.x, y: CGFloat(pageBottomPt) - media.origin.y)
            page.draw(with: .mediaBox, to: ctx)
            ctx.restoreGState()

            currentTopPt += pageH + gapPt
        }
        return true
    }
    if !ok { return nil }

    // Scan ink mask (ignore separator gap rows when multi-page).
    var inkMask = [Bool](repeating: false, count: widthPx * heightPx)
    var separatorRow = [Bool](repeating: false, count: heightPx)
    if pageCount > 1 {
        var topAccPt = 0.0
        for idx in 0..<(pageCount - 1) {
            if let p = doc.page(at: idx) {
                topAccPt += Double(p.bounds(for: .mediaBox).height)
                let y0 = max(Int((topAccPt * scale).floor), 0)
                let y1 = min(Int(((topAccPt + gapPt) * scale).ceil), heightPx)
                if y0 < y1 {
                    for y in y0..<y1 { separatorRow[y] = true }
                }
                topAccPt += gapPt
            }
        }
    }

    var minX = widthPx
    var minY = heightPx
    var maxX = -1
    var maxY = -1
    var inkPixels = 0

    for y in 0..<heightPx {
        if separatorRow[y] {
            // Paint subtle light gray divider on PNG after ink scan
            for x in 0..<widthPx {
                let off = (y * widthPx + x) * 4
                rgba[off] = 226
                rgba[off + 1] = 232
                rgba[off + 2] = 240
                rgba[off + 3] = 255
            }
            continue
        }
        let rowOff = y * bytesPerRow
        let maskOff = y * widthPx
        for x in 0..<widthPx {
            let p = rowOff + x * 4
            let r = Int(rgba[p])
            let g = Int(rgba[p + 1])
            let b = Int(rgba[p + 2])
            if r < 246 || g < 246 || b < 246 {
                inkMask[maskOff + x] = true
                inkPixels += 1
                if x < minX { minX = x }
                if x > maxX { maxX = x }
                if y < minY { minY = y }
                if y > maxY { maxY = y }
            }
        }
    }

    let lines = extractLines(doc: doc)
    return RenderedRaster(
        widthPx: widthPx,
        heightPx: heightPx,
        pageCount: pageCount,
        firstPageWidthPt: round2(firstW),
        firstPageHeightPt: round2(firstH),
        rgba: rgba,
        inkMask: inkMask,
        inkPixels: inkPixels,
        minXPx: minX,
        minYPx: minY,
        maxXPx: maxX,
        maxYPx: maxY,
        lines: lines
    )
}

extension Double {
    var floor: Double { Foundation.floor(self) }
    var ceil: Double { Foundation.ceil(self) }
}

func savePng(rgba: [UInt8], width: Int, height: Int, toPath path: String) -> Bool {
    let colorSpace = CGColorSpaceCreateDeviceRGB()
    let bytesPerRow = width * 4
    let data = Data(rgba)
    guard let provider = CGDataProvider(data: data as CFData) else { return false }
    guard let cgImg = CGImage(
        width: width,
        height: height,
        bitsPerComponent: 8,
        bitsPerPixel: 32,
        bytesPerRow: bytesPerRow,
        space: colorSpace,
        bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue),
        provider: provider,
        decode: nil,
        shouldInterpolate: false,
        intent: .defaultIntent
    ) else { return false }

    let url = URL(fileURLWithPath: path) as CFURL
    guard let dest = CGImageDestinationCreateWithURL(
        url,
        UTType.png.identifier as CFString,
        1,
        nil
    ) else { return false }
    CGImageDestinationAddImage(dest, cgImg, nil)
    return CGImageDestinationFinalize(dest)
}

func bboxFromRaster(_ r: RenderedRaster, scale: Double) -> BBoxPt? {
    if r.inkPixels == 0 || r.maxXPx < r.minXPx || r.maxYPx < r.minYPx {
        return nil
    }
    let left = Double(r.minXPx) / scale
    let top = Double(r.minYPx) / scale
    let right = Double(r.maxXPx + 1) / scale
    let bottom = Double(r.maxYPx + 1) / scale
    return BBoxPt(
        leftPt: round2(left),
        topPt: round2(top),
        rightPt: round2(right),
        bottomPt: round2(bottom),
        widthPt: round2(right - left),
        heightPt: round2(bottom - top)
    )
}

func compareRasters(
    typst: RenderedRaster,
    word: RenderedRaster,
    scale: Double,
    overlayPath: String
) -> PairComparison {
    let outW = max(typst.widthPx, word.widthPx)
    let outH = max(typst.heightPx, word.heightPx)
    var overlay = [UInt8](repeating: 255, count: outW * outH * 4)

    var overlap = 0
    var unionCount = 0

    for y in 0..<outH {
        for x in 0..<outW {
            let tInk = (x < typst.widthPx && y < typst.heightPx)
                ? typst.inkMask[y * typst.widthPx + x]
                : false
            let wInk = (x < word.widthPx && y < word.heightPx)
                ? word.inkMask[y * word.widthPx + x]
                : false
            let off = (y * outW + x) * 4
            if tInk && wInk {
                overlap += 1
                unionCount += 1
                // Charcoal overlap
                overlay[off] = 30
                overlay[off + 1] = 41
                overlay[off + 2] = 59
                overlay[off + 3] = 255
            } else if tInk {
                unionCount += 1
                // Cobalt blue: Typst only
                overlay[off] = 37
                overlay[off + 1] = 99
                overlay[off + 2] = 235
                overlay[off + 3] = 255
            } else if wInk {
                unionCount += 1
                // Crimson red: Word only
                overlay[off] = 220
                overlay[off + 1] = 38
                overlay[off + 2] = 38
                overlay[off + 3] = 255
            } else if x >= typst.widthPx || y >= typst.heightPx || x >= word.widthPx || y >= word.heightPx {
                // Out-of-page region when page heights differ
                overlay[off] = 248
                overlay[off + 1] = 250
                overlay[off + 2] = 252
                overlay[off + 3] = 255
            }
        }
    }

    _ = savePng(rgba: overlay, width: outW, height: outH, toPath: overlayPath)

    // Compute aligned IoU by aligning each ink bounding box top-left to (0, 0).
    var alignedIou = 1.0
    if typst.inkPixels > 0 && word.inkPixels > 0 {
        let tw = typst.maxXPx - typst.minXPx + 1
        let th = typst.maxYPx - typst.minYPx + 1
        let ww = word.maxXPx - word.minXPx + 1
        let wh = word.maxYPx - word.minYPx + 1
        let aw = max(tw, ww)
        let ah = max(th, wh)
        var aOverlap = 0
        var aUnion = 0
        for dy in 0..<ah {
            for dx in 0..<aw {
                let ti = (dx < tw && dy < th)
                    ? typst.inkMask[(typst.minYPx + dy) * typst.widthPx + (typst.minXPx + dx)]
                    : false
                let wi = (dx < ww && dy < wh)
                    ? word.inkMask[(word.minYPx + dy) * word.widthPx + (word.minXPx + dx)]
                    : false
                if ti && wi {
                    aOverlap += 1
                    aUnion += 1
                } else if ti || wi {
                    aUnion += 1
                }
            }
        }
        alignedIou = aUnion > 0 ? Double(aOverlap) / Double(aUnion) : 1.0
    } else if typst.inkPixels != word.inkPixels {
        alignedIou = 0.0
    }

    let rawIou = unionCount > 0 ? Double(overlap) / Double(unionCount) : 1.0
    let tb = bboxFromRaster(typst, scale: scale)
    let wb = bboxFromRaster(word, scale: scale)

    let dLeft = (wb?.leftPt ?? 0.0) - (tb?.leftPt ?? 0.0)
    let dTop = (wb?.topPt ?? 0.0) - (tb?.topPt ?? 0.0)
    let dRight = (wb?.rightPt ?? 0.0) - (tb?.rightPt ?? 0.0)
    let dBottom = (wb?.bottomPt ?? 0.0) - (tb?.bottomPt ?? 0.0)
    let dWidth = (wb?.widthPt ?? 0.0) - (tb?.widthPt ?? 0.0)
    let dHeight = (wb?.heightPt ?? 0.0) - (tb?.heightPt ?? 0.0)

    return PairComparison(
        deltaLeftPt: round2(dLeft),
        deltaTopPt: round2(dTop),
        deltaRightPt: round2(dRight),
        deltaBottomPt: round2(dBottom),
        deltaWidthPt: round2(dWidth),
        deltaHeightPt: round2(dHeight),
        rawIou: round4(rawIou),
        alignedIou: round4(alignedIou),
        overlapPixels: overlap,
        unionPixels: unionCount,
        lineCountDelta: word.lines.count - typst.lines.count,
        overlayPngPath: overlayPath
    )
}

func runJob(_ job: InspectJob, scale: Double) -> JobOutput {
    let fm = FileManager.default
    try? fm.createDirectory(atPath: job.outDir, withIntermediateDirectories: true)

    guard let tRaster = renderPdf(path: job.typstPdf, scale: scale) else {
        return JobOutput(
            id: job.id,
            error: "Failed to load/render Typst PDF: \(job.typstPdf)",
            typst: nil,
            word: nil,
            comparison: nil
        )
    }

    let typstPng = (job.outDir as NSString).appendingPathComponent("typst.png")
    _ = savePng(rgba: tRaster.rgba, width: tRaster.widthPx, height: tRaster.heightPx, toPath: typstPng)
    let typstSummary = PdfVisualSummary(
        pageCount: tRaster.pageCount,
        pageWidthPt: tRaster.firstPageWidthPt,
        pageHeightPt: tRaster.firstPageHeightPt,
        inkBBox: bboxFromRaster(tRaster, scale: scale),
        inkPixels: tRaster.inkPixels,
        lines: tRaster.lines,
        pngPath: typstPng
    )

    guard let wordPath = job.wordPdf, fm.fileExists(atPath: wordPath) else {
        return JobOutput(
            id: job.id,
            error: nil,
            typst: typstSummary,
            word: nil,
            comparison: nil
        )
    }

    guard let wRaster = renderPdf(path: wordPath, scale: scale) else {
        return JobOutput(
            id: job.id,
            error: "Failed to load/render Word PDF: \(wordPath)",
            typst: typstSummary,
            word: nil,
            comparison: nil
        )
    }

    let wordPng = (job.outDir as NSString).appendingPathComponent("word.png")
    _ = savePng(rgba: wRaster.rgba, width: wRaster.widthPx, height: wRaster.heightPx, toPath: wordPng)
    let wordSummary = PdfVisualSummary(
        pageCount: wRaster.pageCount,
        pageWidthPt: wRaster.firstPageWidthPt,
        pageHeightPt: wRaster.firstPageHeightPt,
        inkBBox: bboxFromRaster(wRaster, scale: scale),
        inkPixels: wRaster.inkPixels,
        lines: wRaster.lines,
        pngPath: wordPng
    )

    let overlayPng = (job.outDir as NSString).appendingPathComponent("overlay.png")
    let cmp = compareRasters(typst: tRaster, word: wRaster, scale: scale, overlayPath: overlayPng)

    return JobOutput(
        id: job.id,
        error: nil,
        typst: typstSummary,
        word: wordSummary,
        comparison: cmp
    )
}

let args = CommandLine.arguments
guard args.count >= 2 else {
    fputs("Usage: pdf_lab_inspector <manifest.json>\n", stderr)
    exit(2)
}

let manifestUrl = URL(fileURLWithPath: args[1])
guard let manifestData = try? Data(contentsOf: manifestUrl),
      let manifest = try? JSONDecoder().decode(Manifest.self, from: manifestData) else {
    fputs("Failed to decode manifest JSON\n", stderr)
    exit(2)
}

let count = manifest.jobs.count
var outputs = Array<JobOutput?>(repeating: nil, count: count)
let lock = NSLock()

DispatchQueue.concurrentPerform(iterations: count) { idx in
    let out = runJob(manifest.jobs[idx], scale: manifest.scale)
    lock.lock()
    outputs[idx] = out
    lock.unlock()
}

let finalOutputs = outputs.compactMap { $0 }
let encoder = JSONEncoder()
if let data = try? encoder.encode(finalOutputs),
   let str = String(data: data, encoding: .utf8) {
    print(str)
}

// Generates the desktop icons from glyph outlines on one tile, so the three
// desktop platforms cannot drift apart:
//
// - the app icon — macos/App/AppIcon.icns, windows/resources/TaigiKeyboard.ico
//   and the Linux hicolor PNG set under linux/data/icons — from one 台;
// - the input-mode indicator icons, one per `ModeIndicator`
//   (desktop/crates/taigi-desktop-core/src/mode_indicator.rs): Windows
//   windows/resources/mode/<name>.ico and Linux hicolor <name>.png, drawn from
//   that mode's symbol (台 / Ts / 白 / Ch / 方 / 英).
//
// Run it by hand after changing anything below:
//
//     swift tools/desktop/make-app-icon.swift
//     swift tools/desktop/make-app-icon.swift --check   (verify, write nothing)
//
// Requires macOS: it rasterises through CoreText and packs the .icns with
// `iconutil`. That is fine because every output is a committed artefact —
// the Windows release build consumes the .ico files and never regenerates
// them. Do NOT wire this into bundle-app.sh, release-app.sh, a Cargo build
// script or a Makefile release prerequisite: generating during a build would
// let the build machine's font version and rasteriser decide what ships.
//
// iOS and Android keep the "Tâi" wordmark and are not touched here. This
// script reads nothing under ios/ or android/ and writes only the paths
// named above.
//
// The mark is the one the Mac menu bar already wears (scripts/
// make-menubar-icon.swift), on an opaque tile rather than as a template: the
// menu bar's negative-alpha trick has no equivalent on Windows, which never
// recolours a tray icon, so a knocked-out glyph is invisible on one theme or
// the other. An opaque tile carries its own contrast onto any background.

import AppKit
import CoreText
import CryptoKit
import ImageIO

// MARK: - The design

let appGlyph = "台"
let fontPostScriptName = "PingFangTC-Semibold"
/// The outline each mark was approved with. `NSFont(name:)` substitutes rather
/// than fails for some names, and Apple can change a glyph under a stable
/// PostScript name in an OS update — either would silently reshape a committed
/// artefact. Regenerating then fails loudly instead of producing a diff nobody
/// can read. Update an entry ONLY together with a reviewed icon change.
let approvedOutlineFingerprints: [String: String] = [
    "台": "4a24567095da319efa0f0284986163581c459b300822c1b7fadc9efbb5c76f81",
    "Ts": "9a97aba6cd4cfbf3ef297d6876ee3f061ffd1fb41225ebbb96038819cd9d77d5",
    "白": "da4bd709899fd47e2bc9e32706953bbe3fc95c8a9ce873865d9605022544dd1c",
    "Ch": "e5fadb42e65c852e203556d85083999b014c77ed76d6a290ac3349eed9d1cca2",
    "方": "b16abbf1b83c244ff77939f4f4bfd8d6c5118c7b06a54b60fa831b75cb66989d",
    "英": "d7ab35ffc69e292455c3911dcd9ca2406e680557b1f5145bca890cbbeff465db",
]

/// `ModeIndicator::icon_name` → `ModeIndicator::symbol`, in the Rust order.
/// The Rust test `icon_assets_exist_for_every_indicator` fails when a
/// variant has no committed file here. English is Windows' Shift-tap mode
/// alone; Linux has none, so it gets no Linux icon.
let modeIcons: [(name: String, symbol: String, linux: Bool)] = [
    ("taigikeyboard-tl-hanji", "台", true),
    ("taigikeyboard-tl-romanization", "Ts", true),
    ("taigikeyboard-poj-hanji", "白", true),
    ("taigikeyboard-poj-romanization", "Ch", true),
    ("taigikeyboard-tps", "方", true),
    ("taigikeyboard-english", "英", false),
]

/// Sampled from the icon this replaces, so the new mark stays in the family:
/// a pure white tile with Apple's near-black ink.
let tileColor = (red: 1.0, green: 1.0, blue: 1.0)
let inkColor = (red: 0x1D / 255.0, green: 0x1D / 255.0, blue: 0x1F / 255.0)
/// Both from `make-menubar-icon.swift`, so the desktop mark and the menu-bar
/// mark are the same shape at different jobs.
let glyphInsetRatio: CGFloat = 0.13
let cornerRadiusRatio: CGFloat = 3.0 / 16.0

/// `iconutil`'s members. The name carries the point size and the scale; the
/// pixel size is their product, and every one is rasterised from the outline
/// rather than downsampled from a larger page.
let iconsetMembers: [(name: String, pixels: Int)] = [
    ("icon_16x16", 16), ("icon_16x16@2x", 32),
    ("icon_32x32", 32), ("icon_32x32@2x", 64),
    ("icon_128x128", 128), ("icon_128x128@2x", 256),
    ("icon_256x256", 256), ("icon_256x256@2x", 512),
    ("icon_512x512", 512), ("icon_512x512@2x", 1024),
]

/// Windows asks for an exact match before it scales: 16/20/24/32/40/48/64 for
/// tray, title bar and menus, 24 upward for the taskbar across DPI settings,
/// and 256 as the largest an .ico can carry.
let windowsSizes = [16, 20, 24, 32, 40, 48, 64, 96, 256]
/// The taskbar input indicator only: 16 at 100% up to 32 at 200%, with room
/// for the 250–300% steps.
let windowsModeSizes = [16, 20, 24, 32, 40, 48, 64]
/// The freedesktop hicolor sizes a desktop looks an application icon up at
/// (`linux/data/icons/hicolor/<size>x<size>/apps/taigikeyboard.png`): the
/// settings window's own icon and the input method's in the panel menus.
let linuxSizes = [16, 22, 24, 32, 48, 64, 128, 256]
/// A tray or panel indicator: 16–24 at 1×, up to 48 at 2×.
let linuxModeSizes = [16, 22, 24, 32, 48, 64]

// MARK: - Paths

let repositoryRoot = URL(fileURLWithPath: #filePath)
    .deletingLastPathComponent()   // tools/desktop/
    .deletingLastPathComponent()   // tools/
    .deletingLastPathComponent()   // repository root
let icnsURL = repositoryRoot.appendingPathComponent("macos/App/AppIcon.icns")
let icoURL = repositoryRoot.appendingPathComponent("windows/resources/TaigiKeyboard.ico")
let icoPacker = repositoryRoot.appendingPathComponent("tools/windows/make-ico.py")
func linuxIconURL(_ size: Int, name: String = "taigikeyboard") -> URL {
    repositoryRoot.appendingPathComponent("linux/data/icons/hicolor/\(size)x\(size)/apps/\(name).png")
}
func windowsModeIconURL(_ name: String) -> URL {
    repositoryRoot.appendingPathComponent("windows/resources/mode/\(name).ico")
}

let isCheckOnly = CommandLine.arguments.contains("--check")

func fail(_ message: String) -> Never {
    FileHandle.standardError.write(Data("error: \(message)\n".utf8))
    exit(1)
}

// MARK: - The glyphs

/// The text as one outline, so it centres on its own ink rather than on a text
/// line box — a CJK glyph's line box carries ascender and descender slack that
/// would push it visibly off centre at 16 px. Laid out by CoreText, so a
/// two-letter mark keeps the font's own advances and kerning; a single glyph
/// sits at the origin, so its outline is the bare glyph path.
func textOutline(_ text: String) -> CGPath {
    guard let font = NSFont(name: fontPostScriptName, size: 100) else {
        fail("font '\(fontPostScriptName)' is not installed")
    }
    guard font.fontName == fontPostScriptName else {
        fail("font '\(fontPostScriptName)' resolved to '\(font.fontName)'")
    }
    let ctFont = font as CTFont
    let attributed = NSAttributedString(string: text, attributes: [.font: font])
    let line = CTLineCreateWithAttributedString(attributed)
    let outline = CGMutablePath()
    var glyphCount = 0
    for run in CTLineGetGlyphRuns(line) as! [CTRun] {
        let runFont = (CTRunGetAttributes(run) as NSDictionary)[kCTFontAttributeName] as! CTFont
        guard CTFontCopyPostScriptName(runFont) as String == fontPostScriptName else {
            fail("'\(text)' fell back to \(CTFontCopyPostScriptName(runFont))")
        }
        let count = CTRunGetGlyphCount(run)
        var glyphs = [CGGlyph](repeating: 0, count: count)
        var positions = [CGPoint](repeating: .zero, count: count)
        CTRunGetGlyphs(run, CFRange(location: 0, length: count), &glyphs)
        CTRunGetPositions(run, CFRange(location: 0, length: count), &positions)
        for (glyph, position) in zip(glyphs, positions) {
            guard let path = CTFontCreatePathForGlyph(ctFont, glyph, nil) else {
                fail("'\(text)' has a glyph with no outline in \(fontPostScriptName)")
            }
            outline.addPath(path, transform: CGAffineTransform(translationX: position.x, y: position.y))
            glyphCount += 1
        }
    }
    guard glyphCount == text.count else {
        fail("'\(text)' laid out as \(glyphCount) glyphs")
    }
    return outline
}

/// A stable serialisation of the outline's own segments — not of the file it
/// came from, which carries every other glyph and a version number.
func fingerprint(of path: CGPath) -> String {
    var text = ""
    path.applyWithBlock { element in
        let e = element.pointee
        let points = UnsafeBufferPointer(start: e.points, count: {
            switch e.type {
            case .moveToPoint, .addLineToPoint: return 1
            case .addQuadCurveToPoint: return 2
            case .addCurveToPoint: return 3
            case .closeSubpath: return 0
            @unknown default: return 0
            }
        }())
        text += "\(e.type.rawValue)"
        for point in points {
            // Six decimals: far finer than any pixel grid here, coarse enough
            // that a last-bit difference in the same outline cannot trip it.
            text += String(format: " %.6f %.6f", point.x, point.y)
        }
        text += "\n"
    }
    let digest = SHA256.hash(data: Data(text.utf8))
    return digest.map { String(format: "%02x", $0) }.joined()
}

/// The outline of `text`, checked against its approved fingerprint.
func approvedOutline(_ text: String) -> CGPath {
    let outline = textOutline(text)
    let found = fingerprint(of: outline)
    guard let approved = approvedOutlineFingerprints[text] else {
        fail("'\(text)' has no entry in `approvedOutlineFingerprints`")
    }
    if approved == "PLACEHOLDER" {
        FileHandle.standardError.write(Data("""
            note: no approved outline fingerprint recorded yet for '\(text)'. This run's outline is
                  \(found)
                  Put it in `approvedOutlineFingerprints` once the icon is reviewed.

            """.utf8))
    } else if found != approved {
        fail("""
            '\(text)' in \(fontPostScriptName) no longer matches the approved outline.
              approved: \(approved)
              this Mac: \(found)
            The font changed under a stable name. Review the rendered icon before
            updating `approvedOutlineFingerprints`.
            """)
    }
    return outline
}

// MARK: - Rasterising

/// Drawn in an explicit sRGB context rather than a device-dependent one, so
/// the two constants above mean the same colour on every Mac — a device space
/// leaves the numbers open to the host's colour environment, which is a
/// different and worse problem than the byte-level variance this script
/// already declines to promise.
func render(pixels: Int, outline: CGPath) -> Data {
    guard let colorSpace = CGColorSpace(name: CGColorSpace.sRGB) else {
        fail("sRGB is unavailable on this system")
    }
    guard let context = CGContext(
        data: nil, width: pixels, height: pixels,
        bitsPerComponent: 8, bytesPerRow: 0, space: colorSpace,
        bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
    ) else {
        fail("could not allocate a \(pixels)×\(pixels) bitmap")
    }
    let side = CGFloat(pixels)

    let tile = CGPath(
        roundedRect: CGRect(x: 0, y: 0, width: side, height: side),
        cornerWidth: side * cornerRadiusRatio,
        cornerHeight: side * cornerRadiusRatio,
        transform: nil
    )
    context.setFillColor(red: tileColor.red, green: tileColor.green, blue: tileColor.blue, alpha: 1)
    context.addPath(tile)
    context.fillPath()

    // Scale the outline into the inset box, then centre it on its own bounds.
    let bounds = outline.boundingBox
    let target = side - side * glyphInsetRatio * 2
    let scale = min(target / bounds.width, target / bounds.height)
    var scaling = CGAffineTransform(scaleX: scale, y: scale)
    guard let scaled = outline.copy(using: &scaling) else { fail("could not scale the outline") }
    let scaledBounds = scaled.boundingBox
    var centring = CGAffineTransform(
        translationX: (side - scaledBounds.width) / 2 - scaledBounds.minX,
        y: (side - scaledBounds.height) / 2 - scaledBounds.minY
    )
    guard let placed = scaled.copy(using: &centring) else { fail("could not centre the outline") }
    context.setFillColor(red: inkColor.red, green: inkColor.green, blue: inkColor.blue, alpha: 1)
    context.addPath(placed)
    context.fillPath()

    guard let image = context.makeImage() else {
        fail("could not read back the \(pixels)×\(pixels) page")
    }
    let png = NSMutableData()
    guard let destination = CGImageDestinationCreateWithData(png, "public.png" as CFString, 1, nil)
    else {
        fail("could not open a PNG encoder")
    }
    CGImageDestinationAddImage(destination, image, nil)
    guard CGImageDestinationFinalize(destination) else {
        fail("could not encode the \(pixels)×\(pixels) page")
    }
    return png as Data
}

// MARK: - Running a tool

@discardableResult
func run(_ launchPath: String, _ arguments: [String]) -> Int32 {
    let process = Process()
    process.executableURL = URL(fileURLWithPath: launchPath)
    process.arguments = arguments
    do { try process.run() } catch { fail("could not run \(launchPath): \(error)") }
    process.waitUntilExit()
    return process.terminationStatus
}

// MARK: - Main

let staging = URL(fileURLWithPath: NSTemporaryDirectory())
    .appendingPathComponent("taigi-app-icon-\(ProcessInfo.processInfo.processIdentifier)")
let iconset = staging.appendingPathComponent("AppIcon.iconset")
try? FileManager.default.createDirectory(at: iconset, withIntermediateDirectories: true)
defer { try? FileManager.default.removeItem(at: staging) }

func stage(_ png: Data, as name: String, in directory: URL = staging) -> URL {
    let url = directory.appendingPathComponent(name)
    do { try png.write(to: url) } catch { fail("could not write \(url.path): \(error)") }
    return url
}

/// Every output as (staged file, committed path), in the order they are
/// written and reported.
var outputs: [(staged: URL, committed: URL)] = []

let appOutline = approvedOutline(appGlyph)
for member in iconsetMembers {
    _ = stage(render(pixels: member.pixels, outline: appOutline), as: "\(member.name).png", in: iconset)
}
let stagedIcns = staging.appendingPathComponent("AppIcon.icns")
guard run("/usr/bin/iconutil", ["--convert", "icns", iconset.path, "--output", stagedIcns.path]) == 0 else {
    fail("iconutil could not pack the iconset")
}
outputs.append((stagedIcns, icnsURL))

/// Packs one .ico from `sizes` pages of `outline` into staging.
func stageIco(_ outline: CGPath, sizes: [Int], name: String) -> URL {
    let pages = sizes.map { stage(render(pixels: $0, outline: outline), as: "\(name)-win-\($0).png").path }
    let ico = staging.appendingPathComponent("\(name).ico")
    guard run("/usr/bin/env", ["python3", icoPacker.path, ico.path] + pages) == 0 else {
        fail("make-ico.py could not pack \(name).ico")
    }
    return ico
}

outputs.append((stageIco(appOutline, sizes: windowsSizes, name: "TaigiKeyboard"), icoURL))
for size in linuxSizes {
    outputs.append((stage(render(pixels: size, outline: appOutline), as: "linux-\(size).png"), linuxIconURL(size)))
}

for mode in modeIcons {
    let outline = approvedOutline(mode.symbol)
    outputs.append((stageIco(outline, sizes: windowsModeSizes, name: mode.name), windowsModeIconURL(mode.name)))
    for size in linuxModeSizes where mode.linux {
        let png = render(pixels: size, outline: outline)
        outputs.append((stage(png, as: "\(mode.name)-linux-\(size).png"), linuxIconURL(size, name: mode.name)))
    }
}

// Everything is rendered and packed into staging first, so a font, rasteriser,
// `iconutil` or packer failure leaves every committed artefact untouched. The
// replacements below are then sequential, not transactional: a failure
// between them (a full disk, a read-only checkout) can leave some outputs
// updated and others not. `git status` shows that immediately and a re-run
// fixes it, which is proportionate for a generator run by hand.
if isCheckOnly {
    let stale = outputs.filter { (try? Data(contentsOf: $0.staged)) != (try? Data(contentsOf: $0.committed)) }
    for output in stale {
        print("STALE  \(output.committed.path.replacingOccurrences(of: repositoryRoot.path + "/", with: ""))")
    }
    print("\(outputs.count - stale.count) of \(outputs.count) outputs up to date")
    exit(stale.isEmpty ? 0 : 1)
}

for (staged, committed) in outputs {
    do {
        try FileManager.default.createDirectory(
            at: committed.deletingLastPathComponent(), withIntermediateDirectories: true)
        if FileManager.default.fileExists(atPath: committed.path) {
            _ = try FileManager.default.replaceItemAt(committed, withItemAt: staged)
        } else {
            try FileManager.default.copyItem(at: staged, to: committed)
        }
    } catch {
        fail("could not replace \(committed.path): \(error)")
    }
}
print("wrote \(outputs.count) outputs")

// The on-screen TPS key panel: every key of the main block with the glyph it types under TPS.

import AppKit

/// The panel the `showTpsKeyboard` chord asks for (desktop TPS roadmap D6):
/// up while TPS is typed and the user wants it (`SettingsStore.isTpsKeyboardWanted`),
/// following whichever session is typing. Show-only — the mouse goes
/// through it until the click path lands (P6).
///
/// The rows come from desktop-core (`KeyRules.tpsKeyboardRows`), built from
/// the TPS layout table, so the panel draws exactly what the keys type. The
/// same HUD chrome as the Telex guide; owned by the session that raised it,
/// the way the guide is, so a session on its way out cannot take down a
/// panel the incoming one just put up.
///
/// Placed at the bottom centre of the visible frame of the screen the mouse
/// is on — best effort for "the screen being typed on": activation may not
/// ask the client for a caret (`TaigiInputController.activateServer`), and
/// `NSScreen.main` in an input-method agent can be the primary display
/// whatever the user is looking at (`HUDPanel.noticeFrame`).
@MainActor
final class TpsKeyboardPanel {
    static let shared = TpsKeyboardPanel()

    private var panel: NSPanel?

    /// The session the panel is showing for, or nil when nothing is showing.
    private(set) var owner: ComposingSessionToken?

    var isShowing: Bool {
        panel?.isVisible == true
    }

    /// A key cap's side and the gap between caps, in points. Mirrors the
    /// Windows panel's `CAP_SIZE` / `CAP_GAP` (`ui/tps_keyboard.rs`).
    static let capSize: CGFloat = 48
    static let capGap: CGFloat = 6
    private static let padding: CGFloat = 12
    private static let bottomMargin: CGFloat = 24
    private static let glyphFontSize: CGFloat = 22
    private static let shiftGlyphFontSize: CGFloat = 13
    private static let labelFontSize: CGFloat = 11

    /// Read once: the layout table is fixed for the life of the process.
    private lazy var rows: [Taigi_DesktopShell_TpsKeyboardRow] = KeyRules.tpsKeyboardRows() ?? []

    /// How many key caps the panel draws (for the tests).
    var capCount: Int {
        rows.reduce(0) { $0 + $1.caps.count }
    }

    /// Up for `owner`, or kept up and handed to it. Built once and reused —
    /// its content never changes, and AppKit re-colours it for the appearance.
    func show(ownedBy owner: ComposingSessionToken) {
        self.owner = owner
        guard !isShowing else { return }
        guard !rows.isEmpty else { return }
        let panel = panel ?? Self.makePanel(rows: rows)
        self.panel = panel
        if let frame = Self.typingScreenFrame {
            let size = panel.frame.size
            panel.setFrameOrigin(NSPoint(
                x: frame.midX - size.width / 2,
                y: frame.minY + Self.bottomMargin,
            ))
        }
        panel.orderFrontRegardless()
    }

    /// Takes the panel down only if `owner` raised it. IMK activates the
    /// incoming session before it deactivates the outgoing one, so an old
    /// session's teardown must not close a panel the new one just put up.
    func hide(ownedBy owner: ComposingSessionToken) {
        guard self.owner == owner else { return }
        hideNow()
    }

    /// Takes the panel down whatever session raised it.
    func hideNow() {
        owner = nil
        panel?.orderOut(nil)
    }

    /// The visible frame of the screen the mouse is on, else the main one.
    private static var typingScreenFrame: NSRect? {
        let mouse = NSEvent.mouseLocation
        let screen = NSScreen.screens.first { $0.frame.contains(mouse) } ?? NSScreen.main
        return screen?.visibleFrame
    }

    private static func makePanel(rows: [Taigi_DesktopShell_TpsKeyboardRow]) -> NSPanel {
        let pitch = capSize + capGap
        let width = rows
            .map { (CGFloat($0.indent) + CGFloat($0.caps.count)) * pitch - capGap }
            .max() ?? 0
        let height = CGFloat(rows.count) * pitch - capGap
        let size = NSSize(width: width + 2 * padding, height: height + 2 * padding)
        let background = HUDPanel.makeBackground(size: size)
        for (rowIndex, row) in rows.enumerated() {
            // AppKit's y runs up: the first row is the top one.
            let y = size.height - padding - CGFloat(rowIndex + 1) * pitch + capGap
            for (capIndex, cap) in row.caps.enumerated() {
                let x = padding + (CGFloat(row.indent) + CGFloat(capIndex)) * pitch
                background.addSubview(makeCap(cap, origin: NSPoint(x: x, y: y)))
            }
        }
        return HUDPanel.makePanel(background: background)
    }

    /// One cap: its outline, the key's label top left, the Shift glyph top
    /// right, the glyph large in the middle — as the Windows and Linux panels
    /// draw it.
    private static func makeCap(_ cap: Taigi_DesktopShell_TpsKeyCap, origin: NSPoint) -> NSView {
        let frame = NSRect(origin: origin, size: NSSize(width: capSize, height: capSize))
        let view = NSView(frame: frame)
        view.wantsLayer = true
        view.layer?.cornerRadius = 6
        view.layer?.borderWidth = 1
        view.layer?.borderColor = NSColor.separatorColor.cgColor

        let label = NSTextField(labelWithString: cap.label)
        label.font = .systemFont(ofSize: labelFontSize)
        label.textColor = .tertiaryLabelColor
        label.sizeToFit()
        label.setFrameOrigin(NSPoint(x: 4, y: capSize - 4 - label.frame.height))
        view.addSubview(label)

        if cap.hasShiftGlyph {
            let shift = NSTextField(labelWithString: cap.shiftGlyph)
            shift.font = .systemFont(ofSize: shiftGlyphFontSize)
            shift.textColor = .secondaryLabelColor
            shift.sizeToFit()
            shift.setFrameOrigin(NSPoint(
                x: capSize - 4 - shift.frame.width,
                y: capSize - 4 - shift.frame.height,
            ))
            view.addSubview(shift)
        }

        let glyph = NSTextField(labelWithString: cap.glyph)
        glyph.font = .systemFont(ofSize: glyphFontSize)
        glyph.alignment = .center
        glyph.sizeToFit()
        glyph.frame = NSRect(x: 0, y: 4, width: capSize, height: glyph.frame.height)
        view.addSubview(glyph)
        return view
    }
}

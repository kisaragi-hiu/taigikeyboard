// The on-screen TPS key panel: every key of the main block with the glyph it types under TPS.

import AppKit

/// The panel the `showTpsKeyboard` chord asks for (desktop TPS roadmap D6):
/// up while TPS is typed and the user wants it (`SettingsStore.isTpsKeyboardWanted`),
/// following whichever session is typing. A click on a cap types its glyph
/// through the session that shows the panel (`TpsKeyboardTarget`): the
/// Shift glyph from the cap's top half or with Shift held, the bare glyph
/// otherwise. Non-activating, so the click leaves the app being typed in
/// active and its text view focused — the way vChewing's candidate window
/// takes clicks (`CtlCandidateTDK4AppKit.swift:23`, `VwrCandidateTDK4AppKit.swift:169`).
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

    /// Where a click goes: the owner's controller, and the tenure it showed
    /// the panel in (`TpsKeyboardTarget`). Weak, as the shortcut target is —
    /// IMK owns the controller.
    private weak var target: (any TpsKeyboardTarget)?
    private var generation = 0

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

    /// Read once: the layout table is fixed for the life of the process. A
    /// failed read is not kept, so the next `show` asks again.
    private var cachedRows: [Taigi_DesktopShell_TpsKeyboardRow] = []
    private var rows: [Taigi_DesktopShell_TpsKeyboardRow] {
        if cachedRows.isEmpty {
            cachedRows = KeyRules.tpsKeyboardRows() ?? []
        }
        return cachedRows
    }

    /// What a click on `cap` types: the Shift glyph when the click asks for
    /// the Shift layer (top half, or Shift held) and the key has one, the
    /// bare glyph otherwise — `TpsKeyCap::pressed_glyph` in the core, which
    /// the Windows panel calls.
    static func pressedGlyph(of cap: Taigi_DesktopShell_TpsKeyCap, isShiftLayer: Bool) -> String {
        isShiftLayer && cap.hasShiftGlyph ? cap.shiftGlyph : cap.glyph
    }

    /// The core's caps, row by row (for the tests).
    var caps: [Taigi_DesktopShell_TpsKeyCap] {
        rows.flatMap(\.caps)
    }

    /// How many key caps the panel draws (for the tests).
    var capCount: Int {
        rows.reduce(0) { $0 + $1.caps.count }
    }

    /// Up for `owner`, or kept up and handed to it — placed again each time,
    /// so a session in an app on another screen takes it there (IMK activates
    /// the incoming session before the outgoing one goes, so the panel is
    /// still up at every handover). Built once and reused — its content never
    /// changes, and AppKit re-colours it for the appearance.
    func show(ownedBy owner: ComposingSessionToken, target: any TpsKeyboardTarget, generation: Int) {
        self.owner = owner
        self.target = target
        self.generation = generation
        guard !rows.isEmpty else { return }
        let panel = panel ?? Self.makePanel(rows: rows) { [weak self] glyph in self?.press(glyph) }
        self.panel = panel
        if let frame = Self.typingScreenFrame {
            let size = panel.frame.size
            panel.setFrameOrigin(NSPoint(
                x: frame.midX - size.width / 2,
                y: frame.minY + Self.bottomMargin,
            ))
        }
        if !isShowing {
            panel.orderFrontRegardless()
        }
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
        target = nil
        panel?.orderOut(nil)
    }

    /// A click on a cap typing `glyph`: handed to the session showing the
    /// panel, which checks it is still the one typing before anything is
    /// written. Nothing showing, nothing typed.
    func press(_ glyph: String) {
        target?.typeTpsKeyboardGlyph(glyph, generation: generation)
    }

    /// The visible frame of the screen the mouse is on, else the main one.
    private static var typingScreenFrame: NSRect? {
        let mouse = NSEvent.mouseLocation
        let screen = NSScreen.screens.first { $0.frame.contains(mouse) } ?? NSScreen.main
        return screen?.visibleFrame
    }

    private static func makePanel(
        rows: [Taigi_DesktopShell_TpsKeyboardRow],
        onPress: @escaping @MainActor (String) -> Void,
    ) -> NSPanel {
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
                background.addSubview(makeCap(cap, origin: NSPoint(x: x, y: y), onPress: onPress))
            }
        }
        return HUDPanel.makePanel(background: background, acceptsClicks: true)
    }

    /// One cap: its outline, the key's label top left, the Shift glyph top
    /// right, the glyph large in the middle — as the Windows and Linux panels
    /// draw it.
    private static func makeCap(
        _ cap: Taigi_DesktopShell_TpsKeyCap,
        origin: NSPoint,
        onPress: @escaping @MainActor (String) -> Void,
    ) -> NSView {
        let frame = NSRect(origin: origin, size: NSSize(width: capSize, height: capSize))
        let view = TpsKeyCapView(frame: frame, cap: cap, onPress: onPress)
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

/// Where a click on the panel goes: the controller of the session showing
/// it (`TaigiInputController.typeTpsKeyboardGlyph`).
@MainActor
protocol TpsKeyboardTarget: AnyObject {
    /// Types `glyph` if the session is still the one that showed the panel
    /// in tenure `generation`, still owns the engine and still has its client.
    func typeTpsKeyboardGlyph(_ glyph: String, generation: Int)
}

/// One cap, clickable: the whole cap is one target, its labels included.
private final class TpsKeyCapView: NSView {
    private let cap: Taigi_DesktopShell_TpsKeyCap
    private let onPress: @MainActor (String) -> Void

    init(frame: NSRect, cap: Taigi_DesktopShell_TpsKeyCap, onPress: @escaping @MainActor (String) -> Void) {
        self.cap = cap
        self.onPress = onPress
        super.init(frame: frame)
    }

    @available(*, unavailable)
    required init?(coder _: NSCoder) {
        fatalError("not built from a nib")
    }

    /// The first click types: the panel is never key, so every click is a
    /// first one.
    override func acceptsFirstMouse(for _: NSEvent?) -> Bool {
        true
    }

    override func hitTest(_ point: NSPoint) -> NSView? {
        frame.contains(point) ? self : nil
    }

    /// Taken, so the release comes back to this cap (`mouseUp`).
    override func mouseDown(with _: NSEvent) {}

    /// Types on the release, over the cap the press began on — as the
    /// Windows panel does (`WM_LBUTTONUP`); a release dragged off the cap
    /// types nothing. The half is read where the button comes up.
    override func mouseUp(with event: NSEvent) {
        let point = convert(event.locationInWindow, from: nil)
        guard bounds.contains(point) else { return }
        let isShiftLayer = point.y >= bounds.midY || event.modifierFlags.contains(.shift)
        onPress(TpsKeyboardPanel.pressedGlyph(of: cap, isShiftLayer: isShiftLayer))
    }
}

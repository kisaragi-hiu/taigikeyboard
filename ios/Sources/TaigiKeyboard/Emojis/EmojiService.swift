// Wraps the third-party ISEmojiView, translating its delegate events into EmojiServiceDelegate.
import ISEmojiView
import KeyboardKit
import SwiftUI
import UIKit

protocol EmojiServiceDelegate: AnyObject {
    func emojiDidSelect(_ emoji: String)
    func emojiKeyboardShouldSwitchToAlphabetic()
    func emojiKeyboardShouldDismiss()
    func emojiKeyboardShouldDeleteBackward()
}

final class EmojiService: NSObject {
    weak var delegate: EmojiServiceDelegate?
    private let emojiView: EmojiView

    override init() {
        let keyboardSettings = KeyboardSettings(bottomType: .categories)
        keyboardSettings.countOfRecentsEmojis = 30
        keyboardSettings.needToShowAbcButton = true
        keyboardSettings.isShowPopPreview = true
        keyboardSettings.needToShowDeleteButton = true
        keyboardSettings.updateRecentEmojiImmediately = true
        // Single source of truth: shared emoji/dist/emoji.json. TaigiEmojiData
        // asserts on load failure rather than falling back to a plist (no redundant fallback).
        keyboardSettings.customEmojis = TaigiEmojiData.loadISEmojiCategories()

        emojiView = EmojiView(keyboardSettings: keyboardSettings)
        super.init()
        emojiView.delegate = self
    }

    var emojiKeyboardView: AnyView {
        AnyView(EmojiViewRepresentable(emojiView: emojiView))
    }
}

extension EmojiService: EmojiViewDelegate {
    func emojiViewDidSelectEmoji(_ emoji: String, emojiView _: EmojiView) {
        delegate?.emojiDidSelect(emoji)
    }

    func emojiViewDidPressChangeKeyboardButton(_: EmojiView) {
        delegate?.emojiKeyboardShouldSwitchToAlphabetic()
    }

    func emojiViewDidPressDeleteBackwardButton(_: EmojiView) {
        delegate?.emojiKeyboardShouldDeleteBackward()
    }

    func emojiViewDidPressDismissKeyboardButton(_: EmojiView) {
        delegate?.emojiKeyboardShouldDismiss()
    }
}

/// The emoji keyboard chrome colors of a fixed-palette theme (category icons, selected-category
/// circle, "ABC" title, delete icon, long-press popup), or nil for an adaptive theme so ISEmojiView
/// keeps its own system greys.
// CROSS-PLATFORM INVARIANT — mirrors android/app/src/main/java/com/siansiansu/taigikeyboard/ime/media/emoji/EmojiKeyboardView.kt
// ThemedEmojiColors: emoji chrome foreground = keyTextColor, fills from fixedKeyFill.
struct EmojiChromeColors: Equatable {
    /// Unselected category icons and the pressed "ABC" title.
    static let dimmedTextOpacity: Double = 0.5
    /// Long-press popup outline.
    static let popupBorderOpacity: Double = 0.2

    let keyText: CodableColor
    let keyFill: CodableColor
    /// Selected category circle: the pressed key fill (a translucent fill has none → the fill itself).
    let selectionFill: CodableColor

    static func resolved(from colors: KeyboardColorSettings) -> EmojiChromeColors? {
        guard let fill = colors.fixedKeyFill, let text = colors.keyTextColor else { return nil }
        return EmojiChromeColors(keyText: text, keyFill: fill, selectionFill: fill.pressedKeyFill ?? fill)
    }

    var viewColors: EmojiViewColors {
        let text = UIColor(keyText.color)
        return EmojiViewColors(
            foreground: text,
            dimmedForeground: text.withAlphaComponent(Self.dimmedTextOpacity),
            selectionFill: UIColor(selectionFill.color),
            popupFill: UIColor(keyFill.color),
            popupBorder: text.withAlphaComponent(Self.popupBorderOpacity),
        )
    }
}

private struct EmojiViewRepresentable: UIViewRepresentable {
    let emojiView: EmojiView

    func makeUIView(context _: Context) -> EmojiView {
        emojiView
    }

    /// A themed keyboard (custom surface) shows its surface through the emoji keyboard: the root
    /// `ThemeBackgroundSurface` paints behind it, so ISEmojiView's own background goes clear.
    /// The adaptive default keeps ISEmojiView's `.secondarySystemBackground`. A fixed-palette
    /// theme also recolors the chrome (`EmojiChromeColors`); both follow a live theme change
    /// because the environment's `candidateTheme` re-runs this update.
    func updateUIView(_ view: EmojiView, context: Context) {
        let theme = context.environment.candidateTheme
        view.backgroundColor = theme.surface == nil ? .secondarySystemBackground : .clear
        view.colors = theme.emojiChrome?.viewColors
    }
}

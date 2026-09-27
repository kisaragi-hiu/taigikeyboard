//
//  EmojiViewColors.swift
//  ISEmojiView
//

import UIKit

// TaigiKeyboard: local patch — lets the host keyboard paint the emoji chrome in its theme
// colors. `EmojiView.colors == nil` keeps every vendored default color untouched.
public struct EmojiViewColors: Equatable {
    /// Selected category icon, "ABC" title, delete icon.
    public let foreground: UIColor
    /// Unselected category icons and the highlighted "ABC" title.
    public let dimmedForeground: UIColor
    /// Circle behind the selected category icon.
    public let selectionFill: UIColor
    /// Long-press skin-tone popup fill and border.
    public let popupFill: UIColor
    public let popupBorder: UIColor

    public init(foreground: UIColor, dimmedForeground: UIColor, selectionFill: UIColor, popupFill: UIColor, popupBorder: UIColor) {
        self.foreground = foreground
        self.dimmedForeground = dimmedForeground
        self.selectionFill = selectionFill
        self.popupFill = popupFill
        self.popupBorder = popupBorder
    }
}

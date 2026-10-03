//
//  UIScrollView+EdgeEffects.swift
//  ISEmojiView
//

import UIKit

// TaigiKeyboard: local patch — the host's SwiftUI `scrollEdgeEffectHidden` (#319) never reaches
// these UIKit scroll views; on iOS 27 the default top edge effect blurred the first emoji row.
// Called on both collection views (emoji grid, category bar): nothing is pinned over either.
extension UIScrollView {
    func hideScrollEdgeEffects() {
        guard #available(iOS 26.0, *) else { return }
        topEdgeEffect.isHidden = true
        leftEdgeEffect.isHidden = true
        bottomEdgeEffect.isHidden = true
        rightEdgeEffect.isHidden = true
    }
}

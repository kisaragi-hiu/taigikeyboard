// Section label shared by the toolbar overlays (layout, settings).

import SwiftUI

struct KeyboardOverlaySectionHeader: View {
    let title: String

    @Environment(\.candidateTheme) private var theme

    var body: some View {
        Text(title)
            .font(.caption)
            .fontWeight(.semibold)
            .foregroundColor(theme.secondaryTextColor)
            .textCase(.uppercase)
    }
}

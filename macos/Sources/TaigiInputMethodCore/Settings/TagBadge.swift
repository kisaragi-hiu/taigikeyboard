// A small tag after a row's text — a dictionary source, a learned row (§50).

import SwiftUI

/// Mirrors iOS `TagBadge` and Windows `cards::badge`.
struct TagBadge: View {
    let text: String

    var body: some View {
        Text(text)
            .font(.subheadline)
            .foregroundStyle(.secondary)
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .background(.quaternary, in: RoundedRectangle(cornerRadius: 4))
    }
}

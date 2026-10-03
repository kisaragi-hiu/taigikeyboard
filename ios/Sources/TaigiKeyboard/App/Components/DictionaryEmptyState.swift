import SwiftUI

/// The closed-book placeholder a dictionary page's list shows when it holds
/// nothing yet — Custom Dictionary and Learning Records share it.
struct DictionaryEmptyState: View {
    let message: String

    var body: some View {
        VStack(spacing: 16) {
            Image(latinSystemName: "book.closed")
                .font(AppStyle.appFont(size: 48))
                .foregroundColor(.secondary)
            Text(message)
                .foregroundColor(.secondary)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 32)
    }
}

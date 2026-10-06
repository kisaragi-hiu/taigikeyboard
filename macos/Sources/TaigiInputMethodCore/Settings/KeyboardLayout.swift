// The macOS keyboard layout used while this input method is active.

/// macOS translates key positions through this layout before sending events
/// to the controller. These are system keyboard identifiers, not input modes
/// of our composing engine. TPS always uses QWERTY's established positions.
enum KeyboardLayout: String, CaseIterable, Sendable {
    case qwerty
    case dvorak
    case colemak

    var displayName: String {
        switch self {
        case .qwerty: "QWERTY"
        case .dvorak: "Dvorak"
        case .colemak: "Colemak"
        }
    }

    var inputSourceID: String {
        switch self {
        case .qwerty: "com.apple.keylayout.US"
        case .dvorak: "com.apple.keylayout.Dvorak"
        case .colemak: "com.apple.keylayout.Colemak"
        }
    }
}

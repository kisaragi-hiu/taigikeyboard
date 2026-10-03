// Who is allowed to drive the one composing engine.

@testable import TaigiInputMethodCore
import XCTest

/// Each case builds its own coordinator: `shared` is process-wide because the
/// engine state it guards is, and a test that mutated it would decide what
/// the next test starts from. What a handover does to the composition is the
/// core's (`macos/crates/taigi-macos-ffi/src/session.rs`, tokens tests).
@MainActor
final class ComposingSessionCoordinatorTests: XCTestCase {
    func testOnlyTheClaimingSessionCanDriveTheEngine() {
        let coordinator = TestFixtures.makeCoordinator()
        let focused = ComposingSessionToken()
        let background = ComposingSessionToken()

        coordinator.claim(focused)

        XCTAssertTrue(coordinator.owns(focused))
        XCTAssertFalse(
            coordinator.owns(background),
            """
            a controller still alive in another app must not reach the engine — it would \
            append to the composition the user is typing in the foreground one
            """,
        )
    }

    func testRelease_freesTheEngineForTheNextSession() {
        let coordinator = TestFixtures.makeCoordinator()
        let closing = ComposingSessionToken()
        let next = ComposingSessionToken()
        coordinator.claim(closing)

        XCTAssertTrue(coordinator.release(closing))

        XCTAssertFalse(
            coordinator.owns(closing),
            "a closed session must not keep ownership — every session after it would be mute",
        )
        XCTAssertTrue(
            coordinator.claim(next),
            "the next session must be able to take the engine the closed one held",
        )
    }

    /// The core addresses a session by its token's value: counted, so never
    /// repeated, and never 0 — what an unset proto field decodes to.
    func testTokensAreCountedFromOneAndNeverRepeat() {
        let tokens = (0 ..< 3).map { _ in ComposingSessionToken() }

        XCTAssertFalse(tokens.contains { $0.value == 0 })
        XCTAssertEqual(tokens.map(\.value), tokens.map(\.value).sorted())
        XCTAssertEqual(Set(tokens).count, 3)
    }
}

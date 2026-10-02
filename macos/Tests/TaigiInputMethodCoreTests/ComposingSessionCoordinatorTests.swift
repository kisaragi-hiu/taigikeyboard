// Who is allowed to drive the one composing engine, and what a handover costs.

@testable import TaigiInputMethodCore
import XCTest

/// Each case builds its own coordinator (and, for what a handover does to
/// the composition, its own legacy back end): `shared` is process-wide
/// because the engine state it guards is, and a test that mutated it would
/// decide what the next test starts from.
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

    func testHandover_startsTheNextSessionFromAnIdleEngine() throws {
        let (backend, manager) = try TestFixtures.makeLegacyBackend()
        let first = ComposingSessionToken()
        let second = ComposingSessionToken()
        backend.activate(first)
        manager.append("t", executing: RecordingEffectExecutor())

        backend.activate(second)

        XCTAssertFalse(
            backend.isComposing(second),
            "what the previous session was composing belongs to a document this one cannot write to",
        )
        XCTAssertEqual(manager.rawInput, "")
    }

    func testReclaimingAnOwnedSession_leavesTheCompositionRunning() throws {
        let (backend, manager) = try TestFixtures.makeLegacyBackend()
        let owner = ComposingSessionToken()
        backend.activate(owner)
        manager.append("t", executing: RecordingEffectExecutor())

        backend.activate(owner)

        XCTAssertTrue(
            backend.isComposing(owner),
            "a menu or palette taking focus and giving it back must not lose what was typed",
        )
        XCTAssertEqual(manager.rawInput, "t")
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

    func testReleasingASupersededSession_leavesTheLiveOneAlone() throws {
        let (backend, manager) = try TestFixtures.makeLegacyBackend()
        let superseded = ComposingSessionToken()
        let live = ComposingSessionToken()
        backend.activate(superseded)
        backend.activate(live)
        manager.append("t", executing: RecordingEffectExecutor())

        backend.release(superseded)

        XCTAssertTrue(backend.owns(live), "the live session must keep ownership when a dead one closes")
        XCTAssertEqual(
            manager.rawInput,
            "t",
            "a late teardown callback must not wipe the composition that replaced it",
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

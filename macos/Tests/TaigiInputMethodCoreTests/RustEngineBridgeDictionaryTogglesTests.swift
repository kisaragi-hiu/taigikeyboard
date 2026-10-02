// The user's dictionary toggles as `FetchAtPos` carries them to the engine.

@testable import TaigiInputMethodCore
import XCTest

/// `dictionaryTogglesProto` copies 24 flags field by field, and a swapped pair
/// compiles and filters the wrong dictionary. Which bits the engine resolves
/// the flags into is Rust's rule and tested there
/// (`engine/lexicon/src/dictionary_filters.rs`); this pins the Swift copy.
final class RustEngineBridgeDictionaryTogglesTests: XCTestCase {
    private typealias Wire = Taigi_Engine_DictionarySourceToggles
    private typealias Field = (
        name: String,
        toggle: WritableKeyPath<DictionarySourceToggles, Bool>,
        wire: KeyPath<Wire, Bool>,
    )

    /// Computed: key paths are not `Sendable`, so a stored static is refused.
    private static var fields: [Field] {
        [
            ("kautian", \.kautian, \.kautian),
            ("taigitv", \.taigitv, \.taigitv),
            ("itaigi", \.itaigi, \.itaigi),
            ("sitbut", \.sitbut, \.sitbut),
            ("taihoa", \.taihoa, \.taihoa),
            ("taijit", \.taijit, \.taijit),
            ("kungge", \.kungge, \.kungge),
            ("stti", \.stti, \.stti),
            ("khpoo", \.khpoo, \.khpoo),
            ("variant", \.variant, \.variant),
            ("khiin", \.khiin, \.khiin),
            ("lkk", \.lkk, \.lkk),
            ("dev", \.dev, \.dev),
            ("accentLukang", \.kautianSubcollections.accentLukang, \.kautianSubcollections.accentLukang),
            ("accentSansia", \.kautianSubcollections.accentSansia, \.kautianSubcollections.accentSansia),
            ("accentTaipak", \.kautianSubcollections.accentTaipak, \.kautianSubcollections.accentTaipak),
            ("accentGilan", \.kautianSubcollections.accentGilan, \.kautianSubcollections.accentGilan),
            ("accentTainan", \.kautianSubcollections.accentTainan, \.kautianSubcollections.accentTainan),
            ("accentKaohsiung", \.kautianSubcollections.accentKaohsiung, \.kautianSubcollections.accentKaohsiung),
            ("accentKinmen", \.kautianSubcollections.accentKinmen, \.kautianSubcollections.accentKinmen),
            ("accentMakung", \.kautianSubcollections.accentMakung, \.kautianSubcollections.accentMakung),
            ("accentSintik", \.kautianSubcollections.accentSintik, \.kautianSubcollections.accentSintik),
            ("accentTaichung", \.kautianSubcollections.accentTaichung, \.kautianSubcollections.accentTaichung),
            ("nameAppendix", \.kautianSubcollections.nameAppendix, \.kautianSubcollections.nameAppendix),
        ]
    }

    /// A toggle added to the settings without a row above would go untested.
    func testFieldTable_coversEveryToggle() {
        let defaults = DictionarySourceToggles.defaults
        let flags = [Mirror(reflecting: defaults), Mirror(reflecting: defaults.kautianSubcollections)]
            .flatMap(\.children)
            .filter { $0.value is Bool }
        XCTAssertEqual(Self.fields.count, flags.count)
    }

    func testDefaults_copyVerbatim() {
        let wire = RustEngineBridge.dictionaryTogglesProto(.defaults)
        for field in Self.fields {
            XCTAssertEqual(wire[keyPath: field.wire], DictionarySourceToggles.defaults[keyPath: field.toggle], field.name)
        }
    }

    /// Flipping one toggle flips its own wire field and no other.
    func testEachToggle_landsOnItsOwnWireField() {
        let fields = Self.fields
        let baseline = RustEngineBridge.dictionaryTogglesProto(.defaults)
        for flipped in fields {
            var toggles = DictionarySourceToggles.defaults
            toggles[keyPath: flipped.toggle].toggle()
            let wire = RustEngineBridge.dictionaryTogglesProto(toggles)

            for field in fields {
                let expected = baseline[keyPath: field.wire] != (field.name == flipped.name)
                XCTAssertEqual(wire[keyPath: field.wire], expected, "\(field.name) after flipping \(flipped.name)")
            }
        }
    }

    /// An absent subcollection message tells the engine to treat every
    /// subcollection as on (`lexicon.proto`), which would ignore the eleven
    /// toggles — so it is sent even when every one is at its default.
    func testSubcollections_alwaysSent() {
        XCTAssertTrue(RustEngineBridge.dictionaryTogglesProto(.defaults).hasKautianSubcollections)
    }
}

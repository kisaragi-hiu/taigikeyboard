// Lexicon slice of the engine bridge: the dictionary toggles a fetch carries.
// The dictionary data itself is installed by the desktop core at launch
// (`DesktopCoreRuntime.prepare`).

import Foundation

extension RustEngineBridge {
    /// The user's dictionary toggles on the wire — what `FetchAtPos` carries;
    /// the engine resolves them into its source filter.
    static func dictionaryTogglesProto(_ toggles: DictionarySourceToggles) -> Taigi_Engine_DictionarySourceToggles {
        var togglesProto = Taigi_Engine_DictionarySourceToggles()
        togglesProto.kautian = toggles.kautian
        togglesProto.taigitv = toggles.taigitv
        togglesProto.itaigi = toggles.itaigi
        togglesProto.sitbut = toggles.sitbut
        togglesProto.taihoa = toggles.taihoa
        togglesProto.taijit = toggles.taijit
        togglesProto.kungge = toggles.kungge
        togglesProto.stti = toggles.stti
        togglesProto.khpoo = toggles.khpoo
        togglesProto.variant = toggles.variant
        togglesProto.khiin = toggles.khiin
        togglesProto.lkk = toggles.lkk
        togglesProto.dev = toggles.dev

        // Always sent: an absent subcollection message tells the engine to skip
        // the gate and treat every subcollection as on (`lexicon.proto:452-454`),
        // which would quietly ignore the eleven toggles macOS ships.
        var subcollProto = Taigi_Engine_KautianSubcollectionToggles()
        let subcollections = toggles.kautianSubcollections
        subcollProto.accentLukang = subcollections.accentLukang
        subcollProto.accentSansia = subcollections.accentSansia
        subcollProto.accentTaipak = subcollections.accentTaipak
        subcollProto.accentGilan = subcollections.accentGilan
        subcollProto.accentTainan = subcollections.accentTainan
        subcollProto.accentKaohsiung = subcollections.accentKaohsiung
        subcollProto.accentKinmen = subcollections.accentKinmen
        subcollProto.accentMakung = subcollections.accentMakung
        subcollProto.accentSintik = subcollections.accentSintik
        subcollProto.accentTaichung = subcollections.accentTaichung
        subcollProto.nameAppendix = subcollections.nameAppendix
        togglesProto.kautianSubcollections = subcollProto
        return togglesProto
    }
}

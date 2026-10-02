// Lexicon slice of the engine bridge: loading the dictionary data.

import Foundation

/// Record counts the engine reports after loading. Their only job is to make a
/// successful install self-evident in the log — an install that silently loads
/// zero records looks exactly like a working one until the user types.
struct LexiconInstallStats: Equatable {
    let dictionaryRecordCount: UInt64
    let prefixIndexEntryCount: UInt64
}

extension RustEngineBridge {
    /// Points the engine at the dictionary data. Sent once per process: the
    /// files are read-only and outlive every composing session.
    ///
    /// `dictionaryVersion` is the platform's stamp for the data it shipped; the
    /// engine records it so a later slice can tell which build's dictionary the
    /// user's data was learned against.
    ///
    /// `nil` means the engine has no lexicon installed. Nothing here retries or
    /// falls back: a search against an uninstalled engine returns no candidates,
    /// which is the same graceful degradation any other empty result produces.
    static func lexiconInstall(
        artifacts: DictionaryArtifacts,
        dictionaryVersion: UInt32,
    ) -> LexiconInstallStats? {
        var install = Taigi_Engine_InstallRequest()
        install.triePath = artifacts.triePath
        install.dictionaryBinPath = artifacts.dictionaryBinPath
        install.associationBinPath = artifacts.associationBinPath
        install.dictionaryVersion = dictionaryVersion
        install.syllableInventoryPath = artifacts.syllableInventoryPath

        let op = "lexiconInstall"
        guard let response = lexiconResponse(.install(install), op: op) else { return nil }
        guard case let .installResult(result)? = response.result else {
            recordFailure(op: op, message: "response carried no install result")
            return nil
        }
        return LexiconInstallStats(
            dictionaryRecordCount: result.dictionaryRecordCount,
            prefixIndexEntryCount: result.prefixIndexEntryCount,
        )
    }

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

    private static func lexiconResponse(
        _ method: Taigi_Engine_LexiconRequest.OneOf_Method,
        op: String,
    ) -> Taigi_Engine_LexiconResponse? {
        var lexicon = Taigi_Engine_LexiconRequest()
        lexicon.method = method
        guard let payload = roundtrip(payload: .lexicon(lexicon), op: op) else { return nil }
        guard case let .lexicon(response) = payload else {
            recordFailure(op: op, message: "expected a lexicon payload, got \(payload)")
            return nil
        }
        return response
    }
}

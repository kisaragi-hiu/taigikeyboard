package com.siansiansu.taigikeyboard.ime.dictionary

import com.siansiansu.taigikeyboard.engine.RustEngineBridge
import com.siansiansu.taigikeyboard.ime.core.Outcome
import com.siansiansu.taigikeyboard.ime.core.logging.NullLoggerBackend
import com.siansiansu.taigikeyboard.ime.settings.StubEngineSettings
import com.siansiansu.taigikeyboard.ime.settings.StubEngineSettingsProvider
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * JVM tests for the Dictionary-tab search orchestration. The lexicon is faked
 * (no `.so` on the JVM); what is pinned is the platform-side policy that must
 * match iOS `DictionarySearchServiceTests`.
 */
class DictionarySearchServiceTest {
    /** CJK by the BMP range only — enough for the fixtures; production asks the engine. */
    private class FakeLexicon(
        private val rows: List<DictionarySearchResult> = emptyList(),
    ) : LexiconClient {
        val romanCalls = mutableListOf<RustEngineBridge.LexiconInputMode>()
        val hanziCalls = mutableListOf<RustEngineBridge.LexiconInputMode>()

        override fun isHanzi(text: String): Boolean = text.any { it in '一'..'鿿' }

        override fun dictionaryFilters(toggles: RustEngineBridge.DictionaryToggles): RustEngineBridge.DictionaryFilters =
            RustEngineBridge.DictionaryFilters(
                dictionaryFilterBitmask = 0xFFFu,
                enabledSources =
                    if (toggles.kautian) {
                        DictionarySource.entries.toSet()
                    } else {
                        DictionarySource.entries.toSet() - DictionarySource.KAUTIAN
                    },
            )

        override suspend fun searchWithSources(
            input: String,
            inputMode: RustEngineBridge.LexiconInputMode,
            filterBitmask: UInt,
            limit: Int,
        ): Outcome<List<DictionarySearchResult>, DictionaryError> {
            romanCalls += inputMode
            return Outcome.Success(rows)
        }

        override suspend fun searchByHanzi(
            input: String,
            inputMode: RustEngineBridge.LexiconInputMode,
            filterBitmask: UInt,
            limit: Int,
        ): Outcome<List<DictionarySearchResult>, DictionaryError> {
            hanziCalls += inputMode
            return Outcome.Success(rows)
        }
    }

    private class FakeUserData(
        private val words: List<CustomDictionaryWord>,
    ) : UserDataClient {
        var searchCalls = 0

        override suspend fun search(
            query: String,
            inputMode: String,
            limit: Int,
        ): List<CustomDictionaryWord> {
            searchCalls += 1
            return words.filter { it.roman.startsWith(query) }
        }

        override suspend fun listAll(): List<CustomDictionaryWord> = words

        override suspend fun save(word: CustomDictionaryWord) = error("unused")

        override suspend fun delete(id: String) = error("unused")

        override suspend fun deleteAll() = error("unused")

        override suspend fun exportCsv(): ByteArray = error("unused")

        override suspend fun importCsv(csv: ByteArray): CustomDictionaryImportResult = error("unused")

        override suspend fun clearLearningRecords() = error("unused")

        override suspend fun exportBackup(appVersion: String): ByteArray = error("unused")

        override suspend fun importBackup(backup: ByteArray): BackupImportResult = error("unused")
    }

    private fun row(
        id: Int,
        roman: String,
        frequency: Int,
        vararg sources: DictionarySource,
    ) = DictionarySearchResult(id = id, roman = roman, tl = roman, hanzi = "字", frequency = frequency, sources = sources.toList())

    private val myWord = CustomDictionaryWord(roman = "taigi", hanzi = "我的台語")

    private fun service(
        settings: StubEngineSettings = StubEngineSettings(),
        lexicon: FakeLexicon = FakeLexicon(),
        userData: FakeUserData = FakeUserData(listOf(myWord)),
    ) = DictionarySearchService(lexicon, userData, StubEngineSettingsProvider(settings), NullLoggerBackend)

    // CROSS-PLATFORM INVARIANT — mirrors iOS DictionarySearchServiceTests
    // `testCustomDictionaryOff_itsEntriesAreNotSearched`.
    @Test
    fun `custom dictionary off - its entries are neither searched nor listed`() =
        runTest {
            val userData = FakeUserData(listOf(myWord))

            val results = service(StubEngineSettings(isCustomDictEnabled = false), userData = userData).search("taigi")

            assertEquals(0, userData.searchCalls)
            assertFalse(results.any { it.sources == listOf(DictionarySource.CUSTOM) })
        }

    // CROSS-PLATFORM INVARIANT — mirrors iOS `testCustomDictionaryOn_itsEntriesLeadTheSystemRows`.
    @Test
    fun `custom dictionary on - its entries lead the system rows`() =
        runTest {
            val lexicon = FakeLexicon(listOf(row(1, "tâi-gí", 10, DictionarySource.KAUTIAN)))

            val results = service(lexicon = lexicon).search("taigi")

            assertEquals(listOf(DictionarySource.CUSTOM), results.first().sources)
            assertEquals(DictionarySearchResult.CUSTOM_DICT_MARKER_ID, results.first().id)
            assertEquals(listOf(DictionarySource.KAUTIAN), results.last().sources)
            assertEquals(2, results.size)
        }

    @Test
    fun `a Hanji query takes the hanzi path and never consults the custom dictionary`() =
        runTest {
            val lexicon = FakeLexicon()
            val userData = FakeUserData(listOf(CustomDictionaryWord(roman = "taigi", hanzi = "台語")))

            service(lexicon = lexicon, userData = userData).search("台語")

            assertEquals(1, lexicon.hanziCalls.size)
            assertTrue(lexicon.romanCalls.isEmpty())
            assertEquals(0, userData.searchCalls)
        }

    // CROSS-PLATFORM INVARIANT — mirrors iOS `testTpsLayout_searchesTheTpsFamily`: a Zhuyin
    // query on the TPS layout searches the `tps:` family.
    @Test
    fun `the TPS layout searches the TPS family`() =
        runTest {
            val lexicon = FakeLexicon()

            service(StubEngineSettings(inputMode = "tps"), lexicon = lexicon).search("ㄉㄞ")

            assertEquals(listOf(RustEngineBridge.LexiconInputMode.TPS), lexicon.romanCalls)
        }

    @Test
    fun `lexiconMode maps every stored input mode and falls back to POJ`() {
        assertEquals(RustEngineBridge.LexiconInputMode.POJ, DictionarySearchService.lexiconMode("poj"))
        assertEquals(RustEngineBridge.LexiconInputMode.TL, DictionarySearchService.lexiconMode("tl"))
        assertEquals(RustEngineBridge.LexiconInputMode.TPS, DictionarySearchService.lexiconMode("tps"))
        assertEquals(RustEngineBridge.LexiconInputMode.TL, DictionarySearchService.lexiconMode("english"))
        assertEquals(RustEngineBridge.LexiconInputMode.POJ, DictionarySearchService.lexiconMode("garbage"))
    }

    @Test
    fun `kautian rows come first and badges drop the disabled sources`() =
        runTest {
            val lexicon =
                FakeLexicon(
                    listOf(
                        row(1, "a", 100, DictionarySource.TAIGITV),
                        row(2, "b", 1, DictionarySource.KAUTIAN, DictionarySource.TAIGITV),
                    ),
                )

            val results =
                service(StubEngineSettings(isMoeDictEnabled = false), lexicon = lexicon, userData = FakeUserData(emptyList()))
                    .search("x")

            assertEquals(listOf(2, 1), results.map { it.id })
            assertEquals("the disabled kautian badge is dropped", listOf(DictionarySource.TAIGITV), results.first().sources)
        }
}

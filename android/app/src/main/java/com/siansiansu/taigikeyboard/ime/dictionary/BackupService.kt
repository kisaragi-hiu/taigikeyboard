package com.siansiansu.taigikeyboard.ime.dictionary

import android.content.Context
import com.siansiansu.taigikeyboard.engine.RustEngineBridge
import com.siansiansu.taigikeyboard.engine.pojToTl
import com.siansiansu.taigikeyboard.ime.core.logging.LoggerBackend
import com.siansiansu.taigikeyboard.ime.dictionary.CustomDictionaryService.Entry.Origin
import com.siansiansu.taigikeyboard.ime.text.composing.UserFrequencyService
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject

/**
 * Cross-artifact backup + restore.
 *
 * Exports and imports custom dictionary, user frequency, and next-word
 * associations as a single JSON document for `.taigi` file round-trips.
 * Owned by `CompositionRoot`; delegates to the three concrete services.
 */
class BackupService(
    private val logger: LoggerBackend,
    private val customDict: CustomDictionaryService,
    private val userFreq: UserFrequencyService,
    private val nextWord: NextWordService,
) {
    data class ImportResult(
        val customDict: Int,
        val frequency: Int,
        val association: Int,
    )

    /** Export every user-data artifact as a formatted JSON string. */
    suspend fun exportAll(context: Context): String =
        withContext(Dispatchers.IO) {
            val customEntries = customDict.fetchAll()
            // R5: row-level export preserves each `(word, tl)` reading (#7) —
            // NOT an aggregated-by-word query.
            val frequencyData = userFreq.getAllFrequencyRows()
            val associationData = nextWord.allAssociations()

            val appVersion =
                try {
                    context.packageManager.getPackageInfo(context.packageName, 0).versionName ?: "1.0"
                } catch (_: Exception) {
                    "1.0"
                }

            val json =
                JSONObject().apply {
                    put("version", 3)
                    put(
                        "exportedAt",
                        java.text
                            .SimpleDateFormat("yyyy-MM-dd'T'HH:mm:ss'Z'", java.util.Locale.US)
                            .apply {
                                timeZone = java.util.TimeZone.getTimeZone("UTC")
                            }.format(java.util.Date()),
                    )
                    put("platform", "android")
                    put("appVersion", appVersion)

                    put(
                        "customDictionary",
                        JSONArray().apply {
                            for (entry in customEntries) {
                                put(
                                    JSONObject().apply {
                                        put("roman", entry.roman)
                                        put("hanzi", entry.hanzi)
                                        // §50 provenance (backup v3).
                                        put("origin", entry.origin.raw)
                                        put("learnCount", entry.learnCount)
                                    },
                                )
                            }
                        },
                    )

                    put(
                        "userFrequency",
                        JSONArray().apply {
                            for ((word, tl, count) in frequencyData) {
                                put(
                                    JSONObject().apply {
                                        put("word", word)
                                        put("tl", tl)
                                        put("count", count)
                                        put("lastUsed", "")
                                    },
                                )
                            }
                        },
                    )

                    put(
                        "userAssociation",
                        JSONArray().apply {
                            for (entry in associationData) {
                                put(
                                    JSONObject().apply {
                                        put("prevWord", entry.prevWord)
                                        put("prevTl", entry.prevTl)
                                        put("nextWord", entry.nextWord)
                                        put("nextTl", entry.nextTl)
                                        put("count", entry.count)
                                        put("lastUsed", "")
                                    },
                                )
                            }
                        },
                    )
                }

            json.toString(2)
        }

    /** Import all three artifact kinds from a JSON string using merge semantics. */
    suspend fun importAll(jsonString: String): ImportResult =
        withContext(Dispatchers.IO) {
            val json = JSONObject(jsonString)
            val version = json.optInt("version", 0)
            require(version >= 1) { "Unsupported backup version: $version" }

            val customCount = importCustomDictionary(json.optJSONArray("customDictionary"))
            val freqCount = importFrequency(json.optJSONArray("userFrequency"))
            val assocCount = importAssociations(json.optJSONArray("userAssociation"))

            ImportResult(customDict = customCount, frequency = freqCount, association = assocCount)
        }

    /**
     * §50 — provenance-aware. A manual row in the file is the user's own
     * word: skipped only when a manual row for the pair exists; over a
     * learned row it lands through `save`, whose write takes the learned
     * row over (manual wins, never the reverse). A learned row goes through
     * the learn path so the learned cap, eviction and the manual-wins rule
     * apply exactly as on device. An older file (no `origin`) is all manual.
     */
    private suspend fun importCustomDictionary(array: JSONArray?): Int {
        array ?: return 0
        val existing = customDict.fetchAll()
        // Manual over learned when both exist for a pair (the service does
        // not let that state persist, but a stale list is cheap to fold).
        val originByPair = mutableMapOf<String, Origin>()
        for (row in existing.sortedBy { !it.isLearned }) {
            originByPair["${row.roman}\t${row.hanzi}"] = row.origin
        }

        // Respect the manual row cap: grandfather existing entries, stop at the
        // limit. save() swallows the over-cap throw, so without this the
        // reported count would over-count rows that were never written.
        var manualRemaining = CustomDictionaryCapacityPolicy.remainingCapacity(existing.count { !it.isLearned })
        var imported = 0
        for (i in 0 until array.length()) {
            val obj = array.getJSONObject(i)
            val roman = obj.optString("roman", "")
            val hanzi = obj.optString("hanzi", "")
            if (roman.isEmpty() || hanzi.isEmpty()) continue

            val key = "$roman\t$hanzi"
            val origin = Origin.fromRaw(obj.optInt("origin"))
            // Same two skip rules as iOS: a manual row on device wins, and a
            // pair already learned is not learned again.
            val existing = originByPair[key]
            if (existing == Origin.MANUAL || existing == origin) continue
            when (origin) {
                Origin.LEARNED -> {
                    if (!customDict.learnPhrase(hanzi, roman, count = obj.optInt("learnCount", 1))) continue
                }
                Origin.MANUAL -> {
                    if (manualRemaining <= 0) break
                    customDict.save(CustomDictionaryService.Entry(roman = roman, hanzi = hanzi))
                    manualRemaining--
                }
            }
            originByPair[key] = origin
            imported++
        }
        return imported
    }

    private suspend fun importFrequency(array: JSONArray?): Int {
        array ?: return 0
        // R5: a pre-R5 backup has no `tl` key → "" → the legacy fallback
        // bucket (#7 tolerant).
        val entries =
            (0 until array.length())
                .map { i ->
                    val obj = array.getJSONObject(i)
                    Triple(obj.optString("word", ""), obj.optString("tl", ""), obj.optInt("count", 1))
                }.filter { it.first.isNotEmpty() }

        return userFreq.batchImportMerge(entries)
    }

    private suspend fun importAssociations(array: JSONArray?): Int {
        array ?: return 0
        val entries =
            (0 until array.length())
                .map { i ->
                    val obj = array.getJSONObject(i)
                    // Normalize prevTl/nextTl to TL format (old backups or cross-platform may contain POJ).
                    NextWordService.AssociationEntry(
                        prevWord = obj.optString("prevWord", ""),
                        prevTl = RustEngineBridge.pojToTl(obj.optString("prevTl", "")),
                        nextWord = obj.optString("nextWord", ""),
                        nextTl = RustEngineBridge.pojToTl(obj.optString("nextTl", "")),
                        count = obj.optInt("count", 1),
                    )
                }.filter { it.prevWord.isNotEmpty() && it.nextWord.isNotEmpty() }

        return nextWord.batchImportAssociations(entries)
    }
}

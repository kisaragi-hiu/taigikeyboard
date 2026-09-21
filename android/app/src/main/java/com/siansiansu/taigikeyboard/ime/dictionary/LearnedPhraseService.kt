// Learned-phrase store (§50) — `learned_phrases.db`, the pairs the user composed segment by segment.

package com.siansiansu.taigikeyboard.ime.dictionary

import android.content.Context
import android.database.sqlite.SQLiteDatabase
import android.database.sqlite.SQLiteOpenHelper
import androidx.core.database.sqlite.transaction
import com.siansiansu.taigikeyboard.ime.core.db.bindArgs
import com.siansiansu.taigikeyboard.ime.core.db.rowCount
import com.siansiansu.taigikeyboard.ime.core.db.upsert
import com.siansiansu.taigikeyboard.ime.core.db.vacuumBestEffort
import com.siansiansu.taigikeyboard.ime.core.logging.LoggerBackend
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext

/**
 * Learned phrases (§50): a `(hanzi, canonical TL)` pair the user composed
 * segment by segment (`Effect.PhraseLearned`) is one row with a
 * `learn_count`, plus its cross-mode search keys in a side table shaped like
 * `custom_search_key`, so the exact whole-buffer query in any input mode
 * finds it. Its own file, not a table in `custom_dictionary.db`: learned
 * phrases are learning data (like `user_frequency.db`), never listed with
 * the user's own words and never carried by a backup (USER 2026-09-21).
 * Owned by `CompositionRoot`; mirrors iOS `LearnedPhraseRepository` +
 * `LearnedPhraseService`.
 */
class LearnedPhraseService(
    appContext: Context,
    private val logger: LoggerBackend,
) {
    private val appContext: Context = appContext.applicationContext

    companion object {
        private const val TAG = "LearnedPhraseService"
        private const val DATABASE_NAME = "learned_phrases.db"
        private const val DATABASE_VERSION = 1

        /**
         * Learned rows kept; past it the fewest-composed, then least recently
         * touched, row goes (ChiaKey's policy) so a learn never fails.
         */
        // CROSS-PLATFORM INVARIANT — mirrors ios/Sources/TaigiKeyboard/Lexicon/Database/LearnedPhraseRepository.swift (maxEntries).
        // Drift causes silent divergence.
        const val MAX_ENTRIES = 2_000

        /** Largest `learn_count` a row can carry. */
        // CROSS-PLATFORM INVARIANT — mirrors ios LearnedPhraseRepository.maxLearnCount.
        const val MAX_LEARN_COUNT = 1_000_000

        /** Learned rows per fetch (exact whole-buffer match; homophone phrases). */
        private const val KEYSTROKE_LIMIT = 5

        // DDL + queries are `internal` so the JVM SQL test runs the EXACT
        // production strings against a JDBC in-memory DB (Android's
        // `SQLiteDatabase` is unavailable there).
        // CROSS-PLATFORM INVARIANT — mirrors ios LearnedPhraseSchema. Drift causes silent divergence.
        internal const val CREATE_TABLE_SQL =
            "CREATE TABLE IF NOT EXISTS learned_phrases (" +
                "id INTEGER PRIMARY KEY, roman TEXT NOT NULL, hanzi TEXT NOT NULL, " +
                "learn_count INTEGER NOT NULL DEFAULT 1, updated_at TEXT NOT NULL, UNIQUE(hanzi, roman));"
        internal const val CREATE_SEARCH_KEY_TABLE_SQL =
            "CREATE TABLE IF NOT EXISTS learned_search_key (" +
                "phrase_id INTEGER NOT NULL, family TEXT NOT NULL, form TEXT NOT NULL, key TEXT NOT NULL);"
        internal const val CREATE_SEARCH_KEY_LOOKUP_INDEX_SQL =
            "CREATE INDEX IF NOT EXISTS idx_lsk_lookup ON learned_search_key(family, form, key);"
        internal const val CREATE_SEARCH_KEY_PHRASE_INDEX_SQL =
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_lsk_phrase ON learned_search_key(phrase_id, family, form, key);"
        internal const val CREATE_RANK_INDEX_SQL =
            "CREATE INDEX IF NOT EXISTS idx_learned_rank ON learned_phrases(learn_count, updated_at);"

        /**
         * The learn pair for `upsert` (§8a): bump the row for the pair, else
         * insert it. Args (hanzi, roman) for both; the INSERT's rowid is the
         * phrase id the side keys hang off.
         */
        internal const val LEARN_UPDATE_SQL =
            "UPDATE learned_phrases " +
                "SET learn_count = MIN(learn_count + 1, $MAX_LEARN_COUNT), updated_at = CURRENT_TIMESTAMP " +
                "WHERE hanzi = ? AND roman = ?"
        internal const val LEARN_INSERT_SQL =
            "INSERT INTO learned_phrases (hanzi, roman, learn_count, updated_at) VALUES (?, ?, 1, CURRENT_TIMESTAMP)"
        internal const val INSERT_SEARCH_KEY_SQL =
            "INSERT OR IGNORE INTO learned_search_key (phrase_id, family, form, key) VALUES (?, ?, ?, ?)"

        /** Exact (`=`, not `LIKE`) whole-buffer match, most composed first. */
        // CROSS-PLATFORM INVARIANT — mirrors ios LearnedPhraseRepository `exactMatchSQL`. Drift causes silent divergence.
        internal const val EXACT_MATCH_SQL =
            "SELECT p.hanzi, p.roman, p.learn_count " +
                "FROM learned_phrases p " +
                "JOIN learned_search_key k ON k.phrase_id = p.id " +
                "WHERE k.family = ? AND k.form = ? AND k.key = ? " +
                "ORDER BY p.learn_count DESC, p.updated_at DESC " +
                "LIMIT ?"

        /**
         * The rows past the cap, excluding one kept id — never the row just
         * written, whatever its timestamp ties with; `OFFSET cap - 1`
         * selects exactly the rows past the cap once it is set aside.
         */
        // CROSS-PLATFORM INVARIANT — mirrors ios LearnedPhraseRepository.evictPastCap. Drift causes silent divergence.
        internal const val PAST_CAP_SQL =
            "SELECT id FROM learned_phrases WHERE id <> ? " +
                "ORDER BY learn_count DESC, updated_at DESC, id LIMIT -1 OFFSET ?"

        /** Eviction: keys first, so the subquery still resolves against the intact main table. */
        internal const val EVICT_KEYS_SQL = "DELETE FROM learned_search_key WHERE phrase_id IN ($PAST_CAP_SQL)"
        internal const val EVICT_ROWS_SQL = "DELETE FROM learned_phrases WHERE id IN ($PAST_CAP_SQL)"

        /** The wipe, keys then rows. */
        internal val WIPE_SQL = listOf("DELETE FROM learned_search_key", "DELETE FROM learned_phrases")
    }

    /** One learned phrase: the pair and how often it was composed or picked. */
    data class Phrase(
        val hanzi: String,
        val canonicalTl: String,
        val learnCount: Int,
    )

    private var dbHelper: DatabaseHelper? = null
    private val initMutex = Mutex()
    private var isInitialized = false

    private suspend fun initialize() {
        if (isInitialized) return
        initMutex.withLock {
            if (isInitialized) return
            dbHelper = DatabaseHelper(appContext)
            isInitialized = true
        }
    }

    // Learning

    /**
     * Record one `Effect.PhraseLearned`: bump the row for the pair or insert
     * it. A fresh row gets its search keys (from the same engine derivation
     * the custom dictionary uses) and may evict past the cap; all in ONE
     * transaction so a crash cannot leave a row without keys. Best-effort:
     * a failure is logged, never surfaced.
     */
    suspend fun learnPhrase(
        hanzi: String,
        canonicalTl: String,
    ) = withContext(Dispatchers.IO) {
        if (hanzi.isEmpty() || canonicalTl.isEmpty()) return@withContext
        try {
            initialize()
            val db = dbHelper?.writableDatabase ?: return@withContext
            db.transaction {
                // `-1` = the pair was bumped in place: its keys are already
                // there and its count cannot push the table past the cap.
                val phraseId = upsert(LEARN_UPDATE_SQL, LEARN_INSERT_SQL, hanzi, canonicalTl)
                if (phraseId < 0) return@transaction
                compileStatement(INSERT_SEARCH_KEY_SQL).use { insertKey ->
                    for (key in CustomDictionaryDerivation.deriveCustomSearchKeys(canonicalTl)) {
                        insertKey.bindArgs(phraseId, key.family, key.form, key.key)
                        insertKey.executeInsert()
                    }
                }
                evictPastCap(this, keptId = phraseId)
            }
        } catch (e: Exception) {
            logger.e(TAG, "[LEARN] Failed", e)
        }
    }

    /** Bump a phrase the user just picked as one candidate; no-op for an unknown pair. */
    suspend fun touchPhrase(
        hanzi: String,
        canonicalTl: String,
    ) = withContext(Dispatchers.IO) {
        if (hanzi.isEmpty() || canonicalTl.isEmpty()) return@withContext
        try {
            initialize()
            val db = dbHelper?.writableDatabase ?: return@withContext
            db.execSQL(LEARN_UPDATE_SQL, arrayOf(hanzi, canonicalTl))
        } catch (e: Exception) {
            logger.e(TAG, "[LEARN] Touch failed", e)
        }
    }

    // Reads

    /**
     * Phrases whose derived key EQUALS the query key — the whole typed
     * buffer, not a prefix — for `FetchAtPos.learned_entries`.
     */
    suspend fun matches(
        family: String,
        form: String,
        key: String,
    ): List<Phrase> =
        withContext(Dispatchers.IO) {
            try {
                initialize()
                val db = dbHelper?.readableDatabase ?: return@withContext emptyList()
                db.rawQuery(EXACT_MATCH_SQL, arrayOf(family, form, key, KEYSTROKE_LIMIT.toString())).use { cursor ->
                    val out = mutableListOf<Phrase>()
                    while (cursor.moveToNext()) {
                        out.add(Phrase(hanzi = cursor.getString(0), canonicalTl = cursor.getString(1), learnCount = cursor.getInt(2)))
                    }
                    out
                }
            } catch (e: Exception) {
                logger.e(TAG, "[LEARN] Query failed", e)
                emptyList()
            }
        }

    // Wipe

    /**
     * The learning-data wipe: every phrase and its keys, by DELETE inside the
     * open connection — never by unlinking a file the IME may hold open.
     */
    suspend fun deleteAll() =
        withContext(Dispatchers.IO) {
            try {
                initialize()
                val db = dbHelper?.writableDatabase ?: return@withContext
                db.transaction { WIPE_SQL.forEach(::execSQL) }
                vacuumBestEffort(db, logger, TAG)
            } catch (e: Exception) {
                logger.e(TAG, "[DELETE_ALL] Failed", e)
            }
        }

    // Private

    /**
     * A cheap `COUNT(*)` first — under the cap (nearly every user, forever)
     * the ordered walk never runs. Keys first, so the subquery still resolves
     * against the intact main table. Caller holds the transaction.
     */
    private fun evictPastCap(
        db: SQLiteDatabase,
        keptId: Long,
    ) {
        if (db.rowCount("learned_phrases", fallback = 0) <= MAX_ENTRIES) return
        val args = arrayOf<Any>(keptId, MAX_ENTRIES - 1)
        db.execSQL(EVICT_KEYS_SQL, args)
        db.execSQL(EVICT_ROWS_SQL, args)
    }

    private class DatabaseHelper(
        context: Context,
    ) : SQLiteOpenHelper(context, DATABASE_NAME, null, DATABASE_VERSION) {
        override fun onCreate(db: SQLiteDatabase) {
            db.execSQL(CREATE_TABLE_SQL)
            db.execSQL(CREATE_SEARCH_KEY_TABLE_SQL)
            db.execSQL(CREATE_SEARCH_KEY_LOOKUP_INDEX_SQL)
            db.execSQL(CREATE_SEARCH_KEY_PHRASE_INDEX_SQL)
            db.execSQL(CREATE_RANK_INDEX_SQL)
        }

        /** v1 is the first shape; a key-derivation change bumps the version and adds a backfill arm. */
        override fun onUpgrade(
            db: SQLiteDatabase,
            oldVersion: Int,
            newVersion: Int,
        ) = Unit
    }
}

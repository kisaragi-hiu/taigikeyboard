package com.siansiansu.taigikeyboard.ime.dictionary

import android.content.Context
import android.database.sqlite.SQLiteDatabase
import android.database.sqlite.SQLiteOpenHelper
import android.net.Uri
import androidx.core.database.sqlite.transaction
import com.siansiansu.taigikeyboard.ime.core.db.rowCount
import com.siansiansu.taigikeyboard.ime.core.db.upsert
import com.siansiansu.taigikeyboard.ime.core.db.vacuumBestEffort
import com.siansiansu.taigikeyboard.ime.core.logging.LoggerBackend
import com.siansiansu.taigikeyboard.ime.core.logging.debug
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import java.io.BufferedReader
import java.io.InputStreamReader
import java.util.UUID

/**
 * Custom-dictionary CRUD, CSV export, and file import. Persists to
 * `custom_dictionary.db` via `SQLiteOpenHelper`. Owned by `CompositionRoot`;
 * mirrors iOS `CustomDictionaryService.swift`.
 */
class CustomDictionaryService(
    appContext: Context,
    private val logger: LoggerBackend,
) {
    private val appContext: Context = appContext.applicationContext

    companion object {
        private const val TAG = "CustomDictionaryService"
        private const val DATABASE_NAME = "custom_dictionary.db"
        private const val DATABASE_VERSION = 9

        /** Largest `learn_count` a row can carry — a backup file is untrusted input. */
        // CROSS-PLATFORM INVARIANT — mirrors ios CustomDictionaryRepository.maxLearnCount.
        const val MAX_LEARN_COUNT = 1_000_000

        /**
         * v3.6.1 R3 cross-mode side-table DDL + indexes + query. `internal` so
         * the JVM SQL-structure test (`CustomDictionaryServiceCrossModeTest`)
         * runs the EXACT production strings against a JDBC in-memory DB — no
         * SQL duplication that could drift from the live query.
         */
        internal const val CREATE_SEARCH_KEY_TABLE_SQL =
            "CREATE TABLE IF NOT EXISTS custom_search_key (" +
                "entry_id TEXT NOT NULL, family TEXT NOT NULL, form TEXT NOT NULL, key TEXT NOT NULL);"
        internal const val CREATE_SEARCH_KEY_LOOKUP_INDEX_SQL =
            "CREATE INDEX IF NOT EXISTS idx_csk_lookup ON custom_search_key(family, form, key);"
        internal const val CREATE_SEARCH_KEY_ENTRY_INDEX_SQL =
            "CREATE INDEX IF NOT EXISTS idx_csk_entry ON custom_search_key(entry_id);"

        /**
         * Upsert pair shared by [save] and [importFromFile] via [executeUpsert]
         * (`upsert`, SQLite 3.22 ceiling, §8a). Keyed on `id`; one arg tuple.
         */
        internal val UPSERT_UPDATE_SQL =
            """
            UPDATE ${Table.NAME}
            SET ${Table.ROMAN} = ?,
                ${Table.HANZI} = ?,
                ${Table.NOTONE} = ?,
                ${Table.ABBREV} = ?,
                ${Table.ROMAN_NUM} = ?,
                ${Table.UPDATED_AT} = CURRENT_TIMESTAMP,
                ${Table.ORIGIN} = 0,
                ${Table.LEARN_COUNT} = 0
            WHERE ${Table.ID} = ?
            """.trimIndent()

        internal val UPSERT_INSERT_SQL =
            """
            INSERT OR IGNORE INTO ${Table.NAME} (${Table.ROMAN}, ${Table.HANZI}, ${Table.NOTONE}, ${Table.ABBREV}, ${Table.ROMAN_NUM}, ${Table.ID}, ${Table.CREATED_AT}, ${Table.UPDATED_AT}, ${Table.ORIGIN}, ${Table.LEARN_COUNT})
            VALUES (?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, 0, 0)
            """.trimIndent()

        /**
         * §50 learn pair (`upsert` shape, SQLite 3.22 ceiling — no `ON
         * CONFLICT DO UPDATE`): bump the learned row for the pair, else insert
         * it (the caller decides which after one lookup). Args: (count,
         * hanzi, roman) for the UPDATE; the INSERT binds (count, hanzi, roman,
         * id, notone, abbrev, roman_num). The learned-pair unique index keeps
         * the pair single.
         */
        internal val LEARN_UPDATE_SQL =
            """
            UPDATE ${Table.NAME}
            SET ${Table.LEARN_COUNT} = MIN(${Table.LEARN_COUNT} + ?, $MAX_LEARN_COUNT),
                ${Table.UPDATED_AT} = CURRENT_TIMESTAMP
            WHERE ${Table.ORIGIN} = 1 AND ${Table.HANZI} = ? AND ${Table.ROMAN} = ?
            """.trimIndent()

        internal val LEARN_INSERT_SQL =
            """
            INSERT INTO ${Table.NAME} (${Table.LEARN_COUNT}, ${Table.HANZI}, ${Table.ROMAN}, ${Table.ID}, ${Table.NOTONE}, ${Table.ABBREV}, ${Table.ROMAN_NUM}, ${Table.CREATED_AT}, ${Table.UPDATED_AT}, ${Table.ORIGIN})
            VALUES (?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, 1)
            """.trimIndent()

        /** One learned row per `(hanzi, roman)`; manual rows keep their duplicate tolerance. */
        // CROSS-PLATFORM INVARIANT — mirrors ios CustomDictionarySchema.learnedPairIndexSQL.
        internal const val CREATE_LEARNED_PAIR_INDEX_SQL =
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_custom_learned_pair ON custom_dictionary(hanzi, roman) WHERE origin = 1;"

        /** One source for onCreate and the JVM SQL tests. */
        internal val CREATE_TABLE_SQL =
            """
            CREATE TABLE ${Table.NAME} (
                ${Table.ID} TEXT PRIMARY KEY,
                ${Table.ROMAN} TEXT NOT NULL,
                ${Table.HANZI} TEXT NOT NULL,
                ${Table.NOTONE} TEXT DEFAULT '',
                ${Table.ABBREV} TEXT DEFAULT '',
                ${Table.ROMAN_NUM} TEXT DEFAULT '',
                ${Table.CREATED_AT} TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                ${Table.UPDATED_AT} TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                ${Table.ORIGIN} INTEGER NOT NULL DEFAULT 0,
                ${Table.LEARN_COUNT} INTEGER NOT NULL DEFAULT 0
            );
            """.trimIndent()

        /** The columns [readEntries] expects, in order; unambiguous in the side-table JOINs too. */
        private const val ENTRY_COLUMNS = "id, roman, hanzi, created_at, updated_at, origin, learn_count"

        // CROSS-PLATFORM INVARIANT — mirrors ios/Sources/TaigiKeyboard/Lexicon/Database/CustomDictionaryRepository.swift `prefixSearchSQL`.
        // Drift causes silent divergence. `origin = 0`: manual rows only (§50) —
        // learned rows ride `FetchAtPos.learned_entries` via [LEARNED_EXACT_SQL].
        internal const val SEARCH_SQL =
            "SELECT DISTINCT $ENTRY_COLUMNS " +
                "FROM custom_dictionary c " +
                "JOIN custom_search_key k ON k.entry_id = c.id " +
                "WHERE c.origin = 0 " +
                "AND k.family = ? " +
                "AND k.form IN (?, 'abbrev') " +
                "AND k.key LIKE ? || '%' " +
                "ORDER BY c.roman " +
                "LIMIT ?"

        /** §50 — learned-only, exact (`=`, not `LIKE`) counterpart of [SEARCH_SQL]. */
        // CROSS-PLATFORM INVARIANT — mirrors ios CustomDictionaryRepository `learnedExactSearchSQL`.
        internal const val LEARNED_EXACT_SQL =
            "SELECT $ENTRY_COLUMNS " +
                "FROM custom_dictionary c " +
                "JOIN custom_search_key k ON k.entry_id = c.id " +
                "WHERE c.origin = 1 " +
                "AND k.family = ? " +
                "AND k.form = ? " +
                "AND k.key = ? " +
                "ORDER BY c.learn_count DESC, c.updated_at DESC " +
                "LIMIT ?"

        /**
         * Delete-then-insert the `custom_search_key` rows for one entry. Shared
         * by [executeUpsert] (every save / import) and the v5→v6 backfill.
         * Delete-first keeps the side table consistent when an UPSERT edits the
         * entry's `roman`.
         */
        private fun rewriteSearchKeys(
            db: SQLiteDatabase,
            entryId: String,
            roman: String,
        ) {
            db.execSQL(
                "DELETE FROM ${SearchKeyTable.NAME} WHERE ${SearchKeyTable.ENTRY_ID} = ?",
                arrayOf(entryId),
            )
            for (key in CustomDictionaryDerivation.deriveCustomSearchKeys(roman)) {
                db.execSQL(
                    "INSERT INTO ${SearchKeyTable.NAME} (${SearchKeyTable.ENTRY_ID}, ${SearchKeyTable.FAMILY}, ${SearchKeyTable.FORM}, ${SearchKeyTable.KEY}) VALUES (?, ?, ?, ?)",
                    arrayOf(entryId, key.family, key.form, key.key),
                )
            }
        }
    }

    private object Table {
        const val NAME = "custom_dictionary"
        const val ID = "id"
        const val ROMAN = "roman"
        const val HANZI = "hanzi"
        const val NOTONE = "notone"
        const val ABBREV = "abbrev"
        const val ROMAN_NUM = "roman_num"
        const val CREATED_AT = "created_at"
        const val UPDATED_AT = "updated_at"

        /** §50 provenance: `Entry.Origin` raw value. */
        const val ORIGIN = "origin"

        /** §50: times a learned row was composed / picked; 0 for a manual row. */
        const val LEARN_COUNT = "learn_count"
    }

    /**
     * Cross-mode search side table (v3.6.1 R3). One row per (entry, family,
     * form) search key produced by `CustomDictionaryDerivation
     * .deriveCustomSearchKeys`. The NEW query path joins here by the current
     * input's family; the legacy `notone` / `abbrev` / `roman_num` columns on
     * `custom_dictionary` stay write-only for backcompat / rollback.
     */
    private object SearchKeyTable {
        const val NAME = "custom_search_key"
        const val ENTRY_ID = "entry_id"
        const val FAMILY = "family"
        const val FORM = "form"
        const val KEY = "key"
    }

    private var dbHelper: DatabaseHelper? = null
    private val initMutex = Mutex()
    private var isInitialized = false

    /**
     * The MANUAL write (§50): upsert by id as the user's own word (`origin`
     * 0, no count — a learned [entry] written here is adopted), side keys,
     * then the takeover of any learned row for the same `(roman, hanzi)` so
     * the pair is listed once and "manual wins, never the reverse" holds for
     * the list editor, the CSV importer and the backup importer alike. The
     * takeover runs AFTER the write, so a failed write leaves the learned
     * row intact. Caller holds the transaction.
     */
    private fun executeUpsert(
        db: SQLiteDatabase,
        entry: Entry,
    ) {
        val notone = CustomDictionaryDerivation.generateNotone(entry.roman)
        val abbrev = CustomDictionaryDerivation.generateAbbrev(entry.roman)
        val romanNum = CustomDictionaryDerivation.generateRomanNum(entry.roman)
        logger.debug(TAG) { "[UPSERT] roman='${entry.roman}' notone='$notone' abbrev='$abbrev' romanNum='$romanNum'" }
        db.upsert(UPSERT_UPDATE_SQL, UPSERT_INSERT_SQL, entry.roman, entry.hanzi, notone, abbrev, romanNum, entry.id)
        // v3.6.1 R3 — refresh the cross-mode side-table keys for this entry.
        // The legacy notone/abbrev/roman_num columns above stay written for
        // backcompat / rollback; the side table is the NEW query path.
        rewriteSearchKeys(db, entry.id, entry.roman)
        removeLearnedRow(db, entry.roman, entry.hanzi, exceptId = entry.id)
    }

    /** Delete the learned row (and side keys) for the pair, unless it is [exceptId] itself. */
    private fun removeLearnedRow(
        db: SQLiteDatabase,
        roman: String,
        hanzi: String,
        exceptId: String,
    ) {
        val where = "${Table.ORIGIN} = 1 AND ${Table.ROMAN} = ? AND ${Table.HANZI} = ? AND ${Table.ID} <> ?"
        val args = arrayOf(roman, hanzi, exceptId)
        db.execSQL(
            "DELETE FROM ${SearchKeyTable.NAME} WHERE ${SearchKeyTable.ENTRY_ID} IN (SELECT ${Table.ID} FROM ${Table.NAME} WHERE $where)",
            args,
        )
        db.execSQL("DELETE FROM ${Table.NAME} WHERE $where", args)
    }

    private suspend fun initialize() {
        if (isInitialized) return
        initMutex.withLock {
            if (isInitialized) return
            dbHelper = DatabaseHelper(appContext, logger)
            isInitialized = true
        }
    }

    // MARK: - Data Model

    data class Entry(
        val id: String = UUID.randomUUID().toString(),
        val roman: String,
        val hanzi: String,
        val createdAt: String = "",
        val updatedAt: String = "",
        val origin: Origin = Origin.MANUAL,
        /** Times the phrase was composed or picked; 0 for a manual row. */
        val learnCount: Int = 0,
    ) {
        /** Who wrote the row (§50). The raw values are the wire / backup contract. */
        // CROSS-PLATFORM INVARIANT — mirrors ios CustomDictionaryEntry.Origin. Drift causes silent divergence.
        enum class Origin(
            val raw: Int,
        ) {
            MANUAL(0),
            LEARNED(1),
            ;

            companion object {
                fun fromRaw(raw: Int): Origin = entries.firstOrNull { it.raw == raw } ?: MANUAL
            }
        }

        val isLearned: Boolean get() = origin == Origin.LEARNED
    }

    data class ImportResult(
        val imported: Int,
        val skipped: Int,
    )

    // MARK: - Default Entries

    private data class DefaultEntry(
        val id: String,
        val roman: String,
        val hanzi: String,
    )

    private val defaultEntries =
        listOf(
            DefaultEntry("default-gau-tsa", "gâu-tsá", "𠢕早"),
            DefaultEntry("default-tsiah-pa-bue", "tsia̍h-pá--buē", "食飽未"),
        )

    /** Seed default example entries when the dictionary is empty (called from `TaigiKeyboard.onCreate`). */
    suspend fun seedDefaultEntryIfEmpty() =
        withContext(Dispatchers.IO) {
            initialize()
            val db = dbHelper?.readableDatabase ?: return@withContext
            val count = db.rowCount(Table.NAME, fallback = 0)
            if (count > 0) return@withContext
            for (entry in defaultEntries) {
                save(Entry(id = entry.id, roman = entry.roman, hanzi = entry.hanzi))
            }
        }

    // MARK: - CRUD

    suspend fun fetchAll(): List<Entry> =
        withContext(Dispatchers.IO) {
            try {
                initialize()
                val db = dbHelper?.readableDatabase ?: return@withContext emptyList()
                db
                    .rawQuery(
                        "SELECT $ENTRY_COLUMNS FROM ${Table.NAME} ORDER BY ${Table.UPDATED_AT} DESC",
                        null,
                    ).use { readEntries(it) }
            } catch (e: Exception) {
                logger.e(TAG, "[FETCH] Failed", e)
                emptyList()
            }
        }

    /** Every row of a cursor shaped `id, roman, hanzi, created_at, updated_at, origin, learn_count`. */
    private fun readEntries(cursor: android.database.Cursor): List<Entry> {
        val results = mutableListOf<Entry>()
        while (cursor.moveToNext()) {
            results.add(
                Entry(
                    id = cursor.getString(0),
                    roman = cursor.getString(1),
                    hanzi = cursor.getString(2),
                    createdAt = cursor.getString(3) ?: "",
                    updatedAt = cursor.getString(4) ?: "",
                    origin = Entry.Origin.fromRaw(cursor.getInt(5)),
                    learnCount = cursor.getInt(6),
                ),
            )
        }
        return results
    }

    @Suppress("SqlResolve")
    suspend fun save(entry: Entry) =
        withContext(Dispatchers.IO) {
            try {
                initialize()
                val db = dbHelper?.writableDatabase ?: return@withContext
                // Capacity guard + write in one transaction (TOCTOU-safe). A NEW
                // id over the cap throws CustomDictionaryFullException → caught
                // below → entry not added (matches iOS silent single-save).
                db.transaction {
                    CustomDictionaryCapacityPolicy.guardInsertCapacity(this, entry.id)
                    executeUpsert(this, entry)
                }
            } catch (e: Exception) {
                logger.e(TAG, "[SAVE] Failed", e)
            }
        }

    suspend fun delete(id: String) =
        withContext(Dispatchers.IO) {
            try {
                initialize()
                val db = dbHelper?.writableDatabase ?: return@withContext
                db.delete(Table.NAME, "${Table.ID} = ?", arrayOf(id))
                // v3.6.1 R3 — drop the entry's side-table search keys too.
                db.delete(SearchKeyTable.NAME, "${SearchKeyTable.ENTRY_ID} = ?", arrayOf(id))
            } catch (e: Exception) {
                logger.e(TAG, "[DELETE] Failed", e)
            }
        }

    suspend fun deleteAll() =
        withContext(Dispatchers.IO) {
            try {
                initialize()
                val db = dbHelper?.writableDatabase ?: return@withContext
                db.execSQL("DELETE FROM ${Table.NAME}")
                // v3.6.1 R3 — clear the side table alongside the main table.
                db.execSQL("DELETE FROM ${SearchKeyTable.NAME}")
                vacuumBestEffort(db, logger, TAG)
            } catch (e: Exception) {
                logger.e(TAG, "[DELETE_ALL] Failed", e)
            }
        }

    /**
     * Cross-mode prefix search (v3.6.1 R3). Joins `custom_search_key` by the
     * current input's [family]; matches the primary [form] OR `abbrev` rows by
     * [key] prefix. [key] comes from `CustomDictionaryDerivation
     * .deriveCustomQueryKey` (already lowercased family-native form) and is
     * bound verbatim. Returns each entry once (`DISTINCT`).
     *
     * @param family `tl` / `poj` / `tps` — the query key's family.
     * @param form `num` / `notone` — the query key's primary form (`abbrev` is always also matched).
     * @param key Family-native prefix (bound verbatim).
     */
    // SQL is the `SEARCH_SQL` companion constant (shared with the JVM test).
    suspend fun search(
        family: String,
        form: String,
        key: String,
        limit: Int = 50,
    ): List<Entry> =
        withContext(Dispatchers.IO) {
            try {
                initialize()
                val db = dbHelper?.readableDatabase ?: return@withContext emptyList()
                db.rawQuery(SEARCH_SQL, arrayOf(family, form, key, limit.toString())).use { readEntries(it) }
            } catch (e: Exception) {
                logger.e(TAG, "[SEARCH] Failed", e)
                emptyList()
            }
        }

    // MARK: - Learned Phrases (§50)

    /**
     * Record one `Effect.PhraseLearned` (or one backup row, [count] > 1):
     * bump the learned row for the `(hanzi, canonical TL)` pair or insert it
     * — the [LEARN_UPDATE_SQL] / [LEARN_INSERT_SQL] pair on the learned-pair
     * unique index. A manual row for the same pair wins (no-op, never
     * downgraded). Manual check, write, side keys and eviction run in ONE
     * transaction; the row just written is never evicted. Returns `false`
     * when the write failed (logged), so the backup importer does not count
     * a row that never landed.
     */
    suspend fun learnPhrase(
        hanzi: String,
        canonicalTl: String,
        count: Int = 1,
    ): Boolean = withContext(Dispatchers.IO) {
        if (hanzi.isEmpty() || canonicalTl.isEmpty()) return@withContext false
        val boundedCount = count.coerceIn(1, MAX_LEARN_COUNT)
        try {
            initialize()
            val db = dbHelper?.writableDatabase ?: return@withContext false
            db.transaction {
                // One lookup decides: a manual row wins, a learned row is
                // bumped in place (its keys are already there and its count
                // cannot push past the cap), nothing → a fresh learned row.
                val existing = rowForPair(this, canonicalTl, hanzi)
                when (existing?.second) {
                    Entry.Origin.MANUAL -> return@transaction
                    Entry.Origin.LEARNED -> {
                        execSQL(LEARN_UPDATE_SQL, arrayOf<Any>(boundedCount, hanzi, canonicalTl))
                        return@transaction
                    }
                    null -> Unit
                }
                val id = UUID.randomUUID().toString()
                execSQL(
                    LEARN_INSERT_SQL,
                    arrayOf<Any>(
                        boundedCount,
                        hanzi,
                        canonicalTl,
                        id,
                        CustomDictionaryDerivation.generateNotone(canonicalTl),
                        CustomDictionaryDerivation.generateAbbrev(canonicalTl),
                        CustomDictionaryDerivation.generateRomanNum(canonicalTl),
                    ),
                )
                rewriteSearchKeys(this, id, canonicalTl)
                CustomDictionaryCapacityPolicy.evictLearnedPastCap(this, keptId = id)
            }
            true
        } catch (e: Exception) {
            logger.e(TAG, "[LEARN] Failed", e)
            false
        }
    }

    /** Bump a learned row the user just picked whole; no-op for a manual row or an unknown pair. */
    suspend fun touchLearnedPhrase(
        hanzi: String,
        canonicalTl: String,
    ) = withContext(Dispatchers.IO) {
        if (hanzi.isEmpty() || canonicalTl.isEmpty()) return@withContext
        try {
            initialize()
            val db = dbHelper?.writableDatabase ?: return@withContext
            db.execSQL(LEARN_UPDATE_SQL, arrayOf<Any>(1, hanzi, canonicalTl))
        } catch (e: Exception) {
            logger.e(TAG, "[LEARN] Touch failed", e)
        }
    }

    /**
     * Learned rows whose derived key EQUALS the query key — the whole typed
     * buffer, not a prefix — for `FetchAtPos.learned_entries`. Exact so a
     * learned whole-buffer match never falls out of the prefix search's
     * `LIMIT`; learned-only, the mirror of [search] being manual-only.
     */
    suspend fun learnedEntries(
        family: String,
        form: String,
        key: String,
        limit: Int = 5,
    ): List<Entry> =
        withContext(Dispatchers.IO) {
            try {
                initialize()
                val db = dbHelper?.readableDatabase ?: return@withContext emptyList()
                db.rawQuery(LEARNED_EXACT_SQL, arrayOf(family, form, key, limit.toString())).use { readEntries(it) }
            } catch (e: Exception) {
                logger.e(TAG, "[LEARN] Query failed", e)
                emptyList()
            }
        }

    /** `(id, origin)` of the row for the pair — a manual one first when both exist. */
    private fun rowForPair(
        db: SQLiteDatabase,
        roman: String,
        hanzi: String,
    ): Pair<String, Entry.Origin>? =
        db
            .rawQuery(
                "SELECT ${Table.ID}, ${Table.ORIGIN} FROM ${Table.NAME} WHERE ${Table.ROMAN} = ? AND ${Table.HANZI} = ? ORDER BY ${Table.ORIGIN} LIMIT 1",
                arrayOf(roman, hanzi),
            ).use { if (it.moveToFirst()) it.getString(0) to Entry.Origin.fromRaw(it.getInt(1)) else null }

    // MARK: - Export

    suspend fun exportCSV(): String =
        withContext(Dispatchers.IO) {
            val entries = fetchAll()
            val sb = StringBuilder()
            for (entry in entries) {
                sb.append("${DictionaryCsvCodec.escape(entry.roman)},${DictionaryCsvCodec.escape(entry.hanzi)}\n")
            }
            sb.toString()
        }

    // MARK: - File Import

    private val MAX_FILE_SIZE = 5L * 1024 * 1024 // 5 MB
    private val IMPORT_BATCH_SIZE = 500

    @Suppress("SqlResolve")
    suspend fun importFromFile(
        context: Context,
        uri: Uri,
    ): ImportResult =
        withContext(Dispatchers.IO) {
            context.contentResolver.openFileDescriptor(uri, "r")?.use { fd ->
                if (fd.statSize > MAX_FILE_SIZE) {
                    throw Exception("fileTooLarge")
                }
            }

            val csvString =
                context.contentResolver.openInputStream(uri)?.use { inputStream ->
                    BufferedReader(InputStreamReader(inputStream, Charsets.UTF_8)).readText()
                } ?: throw Exception("Cannot read file")

            val entries = parseCSV(csvString)

            val hasContentLines = csvString.split("\n").any { it.trim().isNotEmpty() }
            if (entries.isEmpty() && hasContentLines) {
                throw Exception("檔案格式無正確，請使用 CSV 格式（roman,hanzi）")
            }
            if (entries.isEmpty()) return@withContext ImportResult(0, 0)

            if (entries.size > CustomDictionaryCapacityPolicy.MAX_ENTRIES) {
                throw Exception("tooManyEntries")
            }

            initialize()
            val db = dbHelper?.writableDatabase ?: return@withContext ImportResult(0, 0)

            val existingKeys = mutableSetOf<String>()
            db
                .rawQuery(
                    "SELECT ${Table.ROMAN}, ${Table.HANZI} FROM ${Table.NAME} WHERE ${Table.ORIGIN} = 0",
                    null,
                ).use { cursor ->
                    while (cursor.moveToNext()) {
                        val key = "${cursor.getString(0)}|${cursor.getString(1)}"
                        existingKeys.add(key)
                    }
                }

            // Cumulative row cap: grandfather existing rows, stop at MAX_ENTRIES,
            // overflow entries fall into totalSkipped below (no eviction, no throw).
            val remaining =
                CustomDictionaryCapacityPolicy.remainingCapacity(
                    CustomDictionaryCapacityPolicy.currentEntryCount(db),
                )
            var importedCount = 0
            var skippedCount = 0

            for (batch in entries.chunked(IMPORT_BATCH_SIZE)) {
                if (importedCount >= remaining) break
                db.transaction {
                    for (entry in batch) {
                        if (importedCount >= remaining) return@transaction
                        val key = "${entry.roman}|${entry.hanzi}"
                        if (key in existingKeys) {
                            skippedCount++
                            continue
                        }
                        try {
                            executeUpsert(this, entry)
                            existingKeys.add(key)
                            importedCount++
                        } catch (e: Exception) {
                            logger.w(TAG, "[IMPORT] Skipped entry: ${entry.roman}", e)
                        }
                    }
                }
            }

            val totalSkipped = entries.size - importedCount
            logger.i(TAG, "[IMPORT] Imported $importedCount, skipped $totalSkipped (duplicates: $skippedCount)")
            ImportResult(importedCount, totalSkipped)
        }

    /** Returns the MANUAL entry count (the user's quota), or -1 if DB is not open. */
    fun totalCount(): Int {
        return try {
            val db = dbHelper?.readableDatabase ?: return -1
            CustomDictionaryCapacityPolicy.currentEntryCount(db)
        } catch (_: Exception) {
            -1
        }
    }

    // MARK: - Database Management

    suspend fun deleteDatabase() =
        withContext(Dispatchers.IO) {
            try {
                dbHelper?.close()
                dbHelper = null
                isInitialized = false
                val dbFile = appContext.getDatabasePath(DATABASE_NAME)
                if (dbFile.exists()) {
                    dbFile.delete()
                }
            } catch (e: Exception) {
                logger.e(TAG, "[DELETE_DB] Failed", e)
            }
        }

    // MARK: - CSV Helpers

    internal fun parseCSV(csv: String): List<Entry> {
        val lines = csv.split("\n")
        val entries = mutableListOf<Entry>()

        for (i in lines.indices) {
            val line = lines[i].trim()
            if (line.isEmpty()) continue

            val columns = DictionaryCsvCodec.parseLine(line)
            if (columns.size < 2) continue

            val roman = columns[0].trim()
            val hanzi = columns[1].trim()
            if (roman.isEmpty() || hanzi.isEmpty()) continue

            entries.add(Entry(roman = roman, hanzi = hanzi))
        }

        return entries
    }

    // MARK: - DatabaseHelper

    private class DatabaseHelper(
        context: Context,
        private val logger: LoggerBackend,
    ) : SQLiteOpenHelper(
            context,
            DATABASE_NAME,
            null,
            DATABASE_VERSION,
        ) {
        override fun onCreate(db: SQLiteDatabase) {
            db.execSQL(CREATE_TABLE_SQL)
            db.execSQL("CREATE INDEX idx_custom_roman ON ${Table.NAME}(${Table.ROMAN});")
            db.execSQL("CREATE INDEX idx_custom_notone ON ${Table.NAME}(${Table.NOTONE});")
            db.execSQL("CREATE INDEX idx_custom_abbrev ON ${Table.NAME}(${Table.ABBREV});")
            db.execSQL("CREATE INDEX idx_custom_roman_num ON ${Table.NAME}(${Table.ROMAN_NUM});")
            db.execSQL(CREATE_LEARNED_PAIR_INDEX_SQL)
            createSearchKeyTable(db)
        }

        // v3.6.1 R3 cross-mode search side table + indexes. Shared by [onCreate]
        // and [migrateV5ToV6] (the latter via `IF NOT EXISTS`). DDL strings are
        // companion constants reused by the JVM SQL-structure test.
        // CROSS-PLATFORM INVARIANT — mirrors ios/Sources/TaigiKeyboard/Lexicon/Database/CustomDictionarySchema.swift (custom_search_key).
        // Drift causes silent divergence.
        private fun createSearchKeyTable(db: SQLiteDatabase) {
            db.execSQL(CREATE_SEARCH_KEY_TABLE_SQL)
            db.execSQL(CREATE_SEARCH_KEY_LOOKUP_INDEX_SQL)
            db.execSQL(CREATE_SEARCH_KEY_ENTRY_INDEX_SQL)
        }

        override fun onUpgrade(
            db: SQLiteDatabase,
            oldVersion: Int,
            newVersion: Int,
        ) {
            if (oldVersion < 2) migrateV1ToV2(db)
            if (oldVersion < 3) migrateV2ToV3(db)
            if (oldVersion < 4) migrateV3ToV4(db)
            if (oldVersion < 5) migrateV4ToV5(db)
            if (oldVersion < 6) migrateV5ToV6(db)
            if (oldVersion < 7) migrateV6ToV7(db)
            if (oldVersion < 8) migrateV7ToV8(db)
            if (oldVersion < 9) migrateV8ToV9(db)
            logger.i(TAG, "[UPGRADE] Database upgraded from $oldVersion to $newVersion")
        }

        /** v1 → v2: add notone/abbrev columns and backfill existing rows. */
        private fun migrateV1ToV2(db: SQLiteDatabase) {
            db.execSQL("ALTER TABLE ${Table.NAME} ADD COLUMN ${Table.NOTONE} TEXT DEFAULT '';")
            db.execSQL("ALTER TABLE ${Table.NAME} ADD COLUMN ${Table.ABBREV} TEXT DEFAULT '';")
            db.execSQL("CREATE INDEX IF NOT EXISTS idx_custom_notone ON ${Table.NAME}(${Table.NOTONE});")
            db.execSQL("CREATE INDEX IF NOT EXISTS idx_custom_abbrev ON ${Table.NAME}(${Table.ABBREV});")
            forEachRomanRow(db) { id, roman ->
                db.execSQL(
                    "UPDATE ${Table.NAME} SET ${Table.NOTONE} = ?, ${Table.ABBREV} = ? WHERE ${Table.ID} = ?",
                    arrayOf(
                        CustomDictionaryDerivation.generateNotone(roman),
                        CustomDictionaryDerivation.generateAbbrev(roman),
                        id,
                    ),
                )
            }
        }

        /** v2 → v3: regenerate notone (generateNotone now strips spaces). */
        private fun migrateV2ToV3(db: SQLiteDatabase) = regenerateNotone(db)

        /** v3 → v4: regenerate notone to handle POJ nasal ⁿ (U+207F). */
        private fun migrateV3ToV4(db: SQLiteDatabase) = regenerateNotone(db)

        /** v4 → v5: add roman_num column for tone-aware search and backfill. */
        private fun migrateV4ToV5(db: SQLiteDatabase) {
            db.execSQL("ALTER TABLE ${Table.NAME} ADD COLUMN ${Table.ROMAN_NUM} TEXT DEFAULT '';")
            db.execSQL("CREATE INDEX IF NOT EXISTS idx_custom_roman_num ON ${Table.NAME}(${Table.ROMAN_NUM});")
            forEachRomanRow(db) { id, roman ->
                db.execSQL(
                    "UPDATE ${Table.NAME} SET ${Table.ROMAN_NUM} = ? WHERE ${Table.ID} = ?",
                    arrayOf(CustomDictionaryDerivation.generateRomanNum(roman), id),
                )
            }
        }

        /**
         * v5 → v6: add the `custom_search_key` cross-mode side table + indexes
         * and backfill one bundle per existing entry. Non-destructive — the
         * legacy `notone` / `abbrev` / `roman_num` columns are untouched.
         */
        private fun migrateV5ToV6(db: SQLiteDatabase) {
            createSearchKeyTable(db)
            regenerateSearchKeys(db)
        }

        /**
         * v6 → v7: re-derive after the POJ spelling-glyph fix. `o͘` (U+0358)
         * used to be dropped from a derived key as if it were a tone
         * diacritic and the nasal ⁿ survived into the tone-aware key as a
         * display glyph, while a query key built from the raw keyboard buffer
         * carries the ASCII `oo` / `nn` the user types — so POJ entries
         * containing either were unreachable from the keyboard. Same shape as
         * v3 → v4, which re-derived for the nasal marker.
         */
        private fun migrateV6ToV7(db: SQLiteDatabase) {
            regenerateNotone(db)
            regenerateSearchKeys(db)
        }

        /**
         * v7 → v8: re-derive after the abbreviation face changed from the
         * first letter of each syllable to its leading spelling unit
         * (`ph` / `th` / `kh` / `tsh` whole, `behavioral-invariants.md`
         * §46) — the `abbrev` rows of the side table are what the keyboard
         * matches, and a stored `pt…` row would no longer answer `phth…`.
         * The legacy `abbrev` column keeps its first-letter face.
         */
        private fun migrateV7ToV8(db: SQLiteDatabase) {
            regenerateSearchKeys(db)
        }

        /**
         * v8 → v9: learned phrases (§50) — provenance columns (every existing
         * row reads as manual) + the learned-pair unique index. Derivation
         * unchanged, no key backfill. `SQLiteOpenHelper` runs `onUpgrade`
         * inside a transaction, so a failed ALTER is never stamped as v9.
         */
        private fun migrateV8ToV9(db: SQLiteDatabase) {
            db.execSQL("ALTER TABLE ${Table.NAME} ADD COLUMN ${Table.ORIGIN} INTEGER NOT NULL DEFAULT 0;")
            db.execSQL("ALTER TABLE ${Table.NAME} ADD COLUMN ${Table.LEARN_COUNT} INTEGER NOT NULL DEFAULT 0;")
            db.execSQL(CREATE_LEARNED_PAIR_INDEX_SQL)
        }

        private fun regenerateSearchKeys(db: SQLiteDatabase) {
            forEachRomanRow(db) { id, roman -> rewriteSearchKeys(db, id, roman) }
        }

        private fun regenerateNotone(db: SQLiteDatabase) {
            forEachRomanRow(db) { id, roman ->
                db.execSQL(
                    "UPDATE ${Table.NAME} SET ${Table.NOTONE} = ? WHERE ${Table.ID} = ?",
                    arrayOf(CustomDictionaryDerivation.generateNotone(roman), id),
                )
            }
        }

        private inline fun forEachRomanRow(
            db: SQLiteDatabase,
            action: (id: String, roman: String) -> Unit,
        ) {
            db.rawQuery("SELECT ${Table.ID}, ${Table.ROMAN} FROM ${Table.NAME}", null).use {
                while (it.moveToNext()) {
                    action(it.getString(0), it.getString(1))
                }
            }
        }
    }
}

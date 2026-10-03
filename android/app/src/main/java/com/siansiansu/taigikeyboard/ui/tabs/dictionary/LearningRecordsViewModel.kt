package com.siansiansu.taigikeyboard.ui.tabs.dictionary

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.siansiansu.taigikeyboard.engine.proto.LearningRecord
import com.siansiansu.taigikeyboard.engine.proto.LearningRecordKind
import com.siansiansu.taigikeyboard.engine.proto.LearningRecordOrder
import com.siansiansu.taigikeyboard.ime.core.CompositionRoot
import com.siansiansu.taigikeyboard.ime.dictionary.UserDataClient
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import java.time.Instant
import java.time.ZoneId

/** Rows per engine page. */
internal const val LEARNING_RECORDS_PAGE_SIZE = 100

/** How long the filter must stay unchanged before the engine is asked — the desktop's `FILTER_SETTLE`. */
internal const val LEARNING_RECORDS_FILTER_SETTLE_MILLIS = 200L

/**
 * The largest count the edit dialog accepts. CROSS-PLATFORM INVARIANT — the
 * engine clamps to the same (`engine/userdata/src/learning_records.rs`
 * `MAX_COUNT`).
 */
internal const val MAX_LEARNING_RECORD_COUNT = 1_000_000L

/** Why the screen shows a dialog. */
sealed interface LearningRecordsMessage {
    /** The list could not be read; [detail] is why, in the engine's words. */
    data class ReadFailed(
        val detail: String,
    ) : LearningRecordsMessage

    /** A count edit or a delete failed. */
    data class WriteFailed(
        val detail: String,
    ) : LearningRecordsMessage

    /** The row was no longer stored (deleted, evicted, its id reused) — said, not a failure. */
    data object Gone : LearningRecordsMessage
}

data class LearningRecordsState(
    val kind: LearningRecordKind = LearningRecordKind.LEARNING_RECORD_KIND_FREQUENCY,
    val order: LearningRecordOrder = LearningRecordOrder.LEARNING_RECORD_ORDER_MOST_USED,
    /** The search box as typed; the engine gets it trimmed. */
    val filter: String = "",
    /** The rows loaded so far, from the first. */
    val records: List<LearningRecord> = emptyList(),
    /** Every row of [kind]. */
    val total: Int = 0,
    /** The rows [filter] matches — how far loading more can go. */
    val matchingTotal: Int = 0,
    val isLoading: Boolean = true,
    /** The last read failed, so neither "nothing learned yet" nor "no results" is known. */
    val hasReadFailed: Boolean = false,
    val message: LearningRecordsMessage? = null,
) {
    val canLoadMore: Boolean get() = records.size < matchingTotal
}

// ViewModel for LearningRecordsScreen — one kind of learned row in one order, filtered and paged
// by the engine (docs/architecture/learning-records-page-roadmap.md); the client runs every
// request on Dispatchers.IO.
class LearningRecordsViewModel internal constructor(
    application: Application,
    private val userData: UserDataClient,
) : AndroidViewModel(application) {
    constructor(application: Application) : this(application, CompositionRoot.shared(application).userData)

    private val _state = MutableStateFlow(LearningRecordsState())
    val state: StateFlow<LearningRecordsState> = _state.asStateFlow()

    /**
     * The one list request in flight, a filter's settle delay included. A newer
     * request cancels it, and a cancelled request never lands (the client's
     * `withContext` resumes a cancelled caller with `CancellationException`),
     * so the newest kind / order / filter always wins.
     */
    private var loadJob: Job? = null

    init {
        reload()
    }

    fun selectKind(kind: LearningRecordKind) {
        if (kind == _state.value.kind) return
        _state.update { it.copy(kind = kind, records = emptyList(), total = 0, matchingTotal = 0) }
        reload()
    }

    fun selectOrder(order: LearningRecordOrder) {
        if (order == _state.value.order) return
        _state.update { it.copy(order = order, records = emptyList()) }
        reload()
    }

    fun updateFilter(filter: String) {
        _state.update { it.copy(filter = filter) }
        reload(settleMillis = LEARNING_RECORDS_FILTER_SETTLE_MILLIS)
    }

    /** The next page, when the list has more and no request is in flight. */
    fun loadMore() {
        val shown = _state.value
        if (loadJob?.isActive == true || !shown.canLoadMore) return
        _state.update { it.copy(isLoading = true) }
        loadJob = viewModelScope.launch { fetch(offset = shown.records.size, limit = LEARNING_RECORDS_PAGE_SIZE) }
    }

    fun setCount(
        record: LearningRecord,
        count: Long,
    ) = write { userData.setLearningRecordCount(record, count) != null }

    /** No confirmation, like deleting one custom word: the keyboard learns the row again on the next pick. */
    fun delete(record: LearningRecord) = write { userData.deleteLearningRecord(record) }

    fun dismissMessage() {
        _state.update { it.copy(message = null) }
    }

    /**
     * Runs [request] — `false` when the row was gone — then reloads every row
     * loaded so far: the row moved, went, or was never there.
     */
    private fun write(request: suspend () -> Boolean) {
        viewModelScope.launch {
            val message =
                try {
                    if (request()) null else LearningRecordsMessage.Gone
                } catch (e: CancellationException) {
                    throw e
                } catch (e: Exception) {
                    LearningRecordsMessage.WriteFailed(e.message.orEmpty())
                }
            if (message != null) _state.update { it.copy(message = message) }
            reload(limit = maxOf(LEARNING_RECORDS_PAGE_SIZE, _state.value.records.size))
        }
    }

    /** Lists from the first row again, [limit] rows, after [settleMillis]. */
    private fun reload(
        settleMillis: Long = 0,
        limit: Int = LEARNING_RECORDS_PAGE_SIZE,
    ) {
        loadJob?.cancel()
        _state.update { it.copy(isLoading = true) }
        loadJob =
            viewModelScope.launch {
                delay(settleMillis)
                fetch(offset = 0, limit = limit)
            }
    }

    /**
     * Lands one page: from the first row it replaces the list, further on it
     * appends. A later page the engine served from an earlier offset (the
     * matches shrank under it) would overlap the list, so the loaded rows are
     * read again instead.
     */
    private suspend fun fetch(
        offset: Int,
        limit: Int,
    ) {
        val asked = _state.value
        val page =
            try {
                userData.listLearningRecords(asked.kind, asked.order, asked.filter.trim(), limit, offset)
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                _state.update {
                    it.copy(isLoading = false, hasReadFailed = true, message = LearningRecordsMessage.ReadFailed(e.message.orEmpty()))
                }
                return
            }
        if (offset > 0 && page.offset != offset) {
            reload(limit = offset)
            return
        }
        _state.update {
            it.copy(
                // A row the keyboard moved between two pages may come twice; it is listed once.
                records = if (offset == 0) page.recordsList else (it.records + page.recordsList).distinctBy { record -> record.id },
                total = page.total,
                matchingTotal = page.matchingTotal,
                isLoading = false,
                hasReadFailed = false,
            )
        }
    }
}

/**
 * The day a row was last used, `yyyy-MM-dd` in [zone] — the desktop's
 * `last_used_label`. Empty when the store held no readable time.
 */
internal fun learningRecordLastUsedLabel(
    lastUsedMs: Long,
    zone: ZoneId = ZoneId.systemDefault(),
): String =
    if (lastUsedMs <= 0) {
        ""
    } else {
        Instant
            .ofEpochMilli(lastUsedMs)
            .atZone(zone)
            .toLocalDate()
            .toString()
    }

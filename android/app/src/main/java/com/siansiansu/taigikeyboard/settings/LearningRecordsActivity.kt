package com.siansiansu.taigikeyboard.settings

import android.content.Context
import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.viewModels
import com.siansiansu.taigikeyboard.engine.proto.LearningRecordKind
import com.siansiansu.taigikeyboard.ime.settings.PrefHelper
import com.siansiansu.taigikeyboard.ui.setTaigiContent
import com.siansiansu.taigikeyboard.ui.tabs.dictionary.LEARNING_RECORDS_KIND_ARG
import com.siansiansu.taigikeyboard.ui.tabs.dictionary.LearningRecordsScreen
import com.siansiansu.taigikeyboard.ui.tabs.dictionary.LearningRecordsViewModel

// Learning records of one kind (word frequency or learned phrases) — what the keyboard learned, one row at a time
class LearningRecordsActivity : ComponentActivity() {
    companion object {
        fun createIntent(
            context: Context,
            kind: LearningRecordKind,
        ): Intent = Intent(context, LearningRecordsActivity::class.java).putExtra(LEARNING_RECORDS_KIND_ARG, kind.name)
    }

    // The default factory hands the intent extras to the view model's SavedStateHandle.
    private val viewModel: LearningRecordsViewModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val prefs = PrefHelper(this)

        setTaigiContent(prefs) {
            LearningRecordsScreen(
                viewModel = viewModel,
                onNavigateBack = {
                    onBackPressedDispatcher.onBackPressed()
                },
            )
        }
    }
}

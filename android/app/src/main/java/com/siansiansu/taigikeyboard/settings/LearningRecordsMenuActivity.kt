package com.siansiansu.taigikeyboard.settings

import android.content.Context
import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import com.siansiansu.taigikeyboard.ime.settings.PrefHelper
import com.siansiansu.taigikeyboard.ui.setTaigiContent
import com.siansiansu.taigikeyboard.ui.tabs.dictionary.LearningRecordsMenuScreen

// Learning Records — picks the kind whose list LearningRecordsActivity shows
class LearningRecordsMenuActivity : ComponentActivity() {
    companion object {
        fun createIntent(context: Context): Intent = Intent(context, LearningRecordsMenuActivity::class.java)
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val prefs = PrefHelper(this)

        setTaigiContent(prefs) {
            LearningRecordsMenuScreen(
                onNavigateBack = {
                    onBackPressedDispatcher.onBackPressed()
                },
                onKind = { kind ->
                    startActivity(LearningRecordsActivity.createIntent(this, kind))
                },
            )
        }
    }
}

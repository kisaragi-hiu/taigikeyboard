package com.siansiansu.taigikeyboard.settings

import android.content.Context
import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import com.siansiansu.taigikeyboard.ime.settings.PrefHelper
import com.siansiansu.taigikeyboard.ui.setTaigiContent
import com.siansiansu.taigikeyboard.ui.tabs.dictionary.DictionarySourcesScreen

// Manage Dictionaries — the dictionary-source toggles
class DictionarySourcesActivity : ComponentActivity() {
    companion object {
        fun createIntent(context: Context): Intent = Intent(context, DictionarySourcesActivity::class.java)
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val prefs = PrefHelper(this)

        setTaigiContent(prefs) {
            DictionarySourcesScreen(
                prefs = prefs,
                onNavigateBack = {
                    onBackPressedDispatcher.onBackPressed()
                },
            )
        }
    }
}

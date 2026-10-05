package com.siansiansu.taigikeyboard.ui.tabs.dictionary

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.siansiansu.taigikeyboard.engine.proto.LearningRecordKind
import com.siansiansu.taigikeyboard.i18n.generated.L10n
import com.siansiansu.taigikeyboard.ui.components.ActionRow
import com.siansiansu.taigikeyboard.ui.components.SettingsCard
import com.siansiansu.taigikeyboard.ui.components.SettingsDivider

// Learning Records — one row per kind (word frequency, learned phrases), each opening that kind's list
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun LearningRecordsMenuScreen(
    onNavigateBack: () -> Unit,
    onKind: (LearningRecordKind) -> Unit,
) {
    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Text(
                        text = L10n.dictionaryLearningRecords,
                        fontWeight = FontWeight.Bold,
                    )
                },
                navigationIcon = {
                    IconButton(onClick = onNavigateBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = L10n.commonBack)
                    }
                },
                colors =
                    TopAppBarDefaults.topAppBarColors(
                        containerColor = MaterialTheme.colorScheme.surfaceContainer,
                    ),
            )
        },
        containerColor = MaterialTheme.colorScheme.surfaceContainer,
    ) { padding ->
        Column(
            modifier =
                Modifier
                    .fillMaxSize()
                    .padding(padding)
                    .verticalScroll(rememberScrollState())
                    .padding(horizontal = 20.dp),
        ) {
            Spacer(Modifier.height(8.dp))
            SettingsCard {
                ActionRow(
                    label = kindLabel(LearningRecordKind.LEARNING_RECORD_KIND_FREQUENCY),
                    onClick = { onKind(LearningRecordKind.LEARNING_RECORD_KIND_FREQUENCY) },
                    trailingIcon = Icons.AutoMirrored.Filled.KeyboardArrowRight,
                )
                SettingsDivider()
                ActionRow(
                    label = kindLabel(LearningRecordKind.LEARNING_RECORD_KIND_LEARNED_PHRASE),
                    onClick = { onKind(LearningRecordKind.LEARNING_RECORD_KIND_LEARNED_PHRASE) },
                    trailingIcon = Icons.AutoMirrored.Filled.KeyboardArrowRight,
                )
            }
        }
    }
}

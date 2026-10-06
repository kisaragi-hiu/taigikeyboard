package com.siansiansu.taigikeyboard.ui.tabs.dictionary

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.LargeTopAppBar
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.unit.dp
import com.siansiansu.taigikeyboard.i18n.generated.L10n
import com.siansiansu.taigikeyboard.ui.components.ActionRow
import com.siansiansu.taigikeyboard.ui.components.SettingsCard
import com.siansiansu.taigikeyboard.ui.components.SettingsDivider
import com.siansiansu.taigikeyboard.ui.theme.AppStyle

// Dictionary tab — Manage Dictionaries and keyboard data navigation
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DictionarySettingsScreen(
    onManageDictionaries: () -> Unit,
    onCustomDictionary: () -> Unit,
    onLearningRecords: () -> Unit,
    onBackupRestore: () -> Unit,
) {
    val scrollBehavior = TopAppBarDefaults.exitUntilCollapsedScrollBehavior()

    Scaffold(
        modifier = Modifier.nestedScroll(scrollBehavior.nestedScrollConnection),
        containerColor = MaterialTheme.colorScheme.surfaceContainer,
        topBar = {
            LargeTopAppBar(
                title = {
                    Text(
                        text = L10n.navTabDictionary,
                        style = MaterialTheme.typography.headlineLarge,
                    )
                },
                expandedHeight = AppStyle.largeTopAppBarExpandedHeight,
                colors =
                    TopAppBarDefaults.topAppBarColors(
                        containerColor = MaterialTheme.colorScheme.surfaceContainer,
                        scrolledContainerColor = MaterialTheme.colorScheme.surfaceContainer,
                    ),
                scrollBehavior = scrollBehavior,
            )
        },
    ) { innerPadding ->
        Column(
            modifier =
                Modifier
                    .fillMaxSize()
                    .padding(innerPadding)
                    .verticalScroll(rememberScrollState())
                    .padding(horizontal = 20.dp)
                    .padding(bottom = AppStyle.scrollContentBottomPadding),
        ) {
            SettingsCard {
                ActionRow(
                    label = L10n.desktopDictionarySourcesLink,
                    onClick = onManageDictionaries,
                    trailingIcon = Icons.AutoMirrored.Filled.KeyboardArrowRight,
                )
                SettingsDivider()
                ActionRow(
                    label = L10n.dictionaryCustomDictionary,
                    onClick = onCustomDictionary,
                    trailingIcon = Icons.AutoMirrored.Filled.KeyboardArrowRight,
                )
                SettingsDivider()
                ActionRow(
                    label = L10n.dictionaryLearningRecords,
                    onClick = onLearningRecords,
                    trailingIcon = Icons.AutoMirrored.Filled.KeyboardArrowRight,
                )
                SettingsDivider()
                ActionRow(
                    label = L10n.dictionaryBackupRestore,
                    onClick = onBackupRestore,
                    trailingIcon = Icons.AutoMirrored.Filled.KeyboardArrowRight,
                )
            }
        }
    }
}

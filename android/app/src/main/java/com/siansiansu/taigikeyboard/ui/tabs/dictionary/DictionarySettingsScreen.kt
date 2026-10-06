package com.siansiansu.taigikeyboard.ui.tabs.dictionary

import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
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
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.siansiansu.taigikeyboard.i18n.generated.L10n
import com.siansiansu.taigikeyboard.ui.components.ActionRow
import com.siansiansu.taigikeyboard.ui.components.FilterSearchBar
import com.siansiansu.taigikeyboard.ui.components.SettingsCard
import com.siansiansu.taigikeyboard.ui.components.SettingsDivider
import com.siansiansu.taigikeyboard.ui.theme.AppStyle
import com.siansiansu.taigikeyboard.ui.theme.SectionHeader

private const val MAX_VISIBLE_SEARCH_RESULTS = 5
private val SEARCH_RESULTS_MAX_HEIGHT = 200.dp

// Dictionary tab — Manage Dictionaries and keyboard data navigation, dictionary search
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DictionarySettingsScreen(
    onManageDictionaries: () -> Unit,
    onCustomDictionary: () -> Unit,
    onLearningRecords: () -> Unit,
    onBackupRestore: () -> Unit,
    searchViewModel: DictionarySearchViewModel? = null,
) {
    val focusManager = LocalFocusManager.current

    // Search state
    val searchText by searchViewModel?.searchText?.collectAsStateWithLifecycle() ?: remember { mutableStateOf("") }
    val searchResults by searchViewModel?.results?.collectAsStateWithLifecycle() ?: remember { mutableStateOf(emptyList()) }
    val isSearching by searchViewModel?.isSearching?.collectAsStateWithLifecycle() ?: remember { mutableStateOf(false) }
    var selectedResultIndex by remember { mutableStateOf<Int?>(null) }
    LaunchedEffect(searchText) { selectedResultIndex = null }

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
        Column(modifier = Modifier.fillMaxSize().padding(innerPadding)) {
            // Scrollable content
            Column(
                modifier =
                    Modifier
                        .weight(1f)
                        .verticalScroll(rememberScrollState())
                        .pointerInput(Unit) { detectTapGestures { focusManager.clearFocus() } }
                        .padding(horizontal = 20.dp)
                        .padding(bottom = AppStyle.scrollContentBottomPadding),
            ) {
                SettingsCard {
                    ActionRow(
                        label = L10n.dictionaryManageDictionaries,
                        onClick = onManageDictionaries,
                        trailingIcon = Icons.AutoMirrored.Filled.KeyboardArrowRight,
                    )
                }

                Spacer(Modifier.height(24.dp))

                // Keyboard data
                SectionHeader(L10n.dictionaryDataManagement)

                SettingsCard {
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

            // Search bar with popup results (pinned at bottom)
            if (searchViewModel != null) {
                Column {
                    // Search results popup above search bar
                    if (searchText.isNotEmpty()) {
                        if (searchResults.isEmpty() && !isSearching) {
                            Text(
                                text = L10n.dictionaryNoResults,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                style = MaterialTheme.typography.bodyLarge,
                                modifier =
                                    Modifier
                                        .padding(horizontal = 20.dp)
                                        .padding(bottom = 8.dp),
                            )
                        } else if (searchResults.isNotEmpty()) {
                            SettingsCard(
                                modifier =
                                    Modifier
                                        .padding(horizontal = 20.dp)
                                        .padding(bottom = 8.dp)
                                        .heightIn(max = SEARCH_RESULTS_MAX_HEIGHT),
                            ) {
                                val visible = searchResults.take(MAX_VISIBLE_SEARCH_RESULTS)
                                LazyColumn {
                                    itemsIndexed(visible) { index, result ->
                                        SearchResultRow(
                                            result = result,
                                            isExpanded = selectedResultIndex == index,
                                            onToggle = {
                                                selectedResultIndex = if (selectedResultIndex == index) null else index
                                            },
                                        )
                                        if (index < visible.size - 1) {
                                            SettingsDivider()
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Search bar
                    FilterSearchBar(
                        value = searchText,
                        onValueChange = { searchViewModel.updateSearchText(it) },
                        placeholder = L10n.dictionarySearchPlaceholder,
                        onClear = { focusManager.clearFocus() },
                    )
                }
            }
        }
    }
}

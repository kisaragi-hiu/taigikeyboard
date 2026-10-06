import SwiftUI
import UIKit

/// Dictionary tab.
///
/// Entry rows for Manage Dictionaries and the keyboard data pages, plus dictionary search.
struct DictionaryTab: View {
    @Environment(DisplayLanguageStore.self) private var lang
    @StateObject private var searchVM = DictionarySearchViewModel()

    /// Search focus
    @FocusState private var isSearchFocused: Bool

    // Dictionary lookup selection
    @State private var selectedResult: DictionarySearchResult?
    @State private var showLookupDialog = false

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    NavigationLink(destination: DictionarySourcesView()) {
                        Text(lang.string(.dictionaryManageDictionaries))
                    }
                }

                // Keyboard data
                Section {
                    NavigationLink(destination: CustomDictionaryView()) {
                        Text(lang.string(.dictionaryCustomDictionary))
                    }
                    NavigationLink(destination: LearningRecordsMenuView()) {
                        Text(lang.string(.dictionaryLearningRecords))
                    }
                    NavigationLink(destination: DataManagementView()) {
                        Text(lang.string(.dictionaryBackupRestore))
                    }
                } header: {
                    Text(lang.string(.dictionaryDataManagement))
                        .font(AppStyle.sectionHeaderFont)
                }
            }
            .navigationTitle(lang.string(TabType.dictionary.titleKey))
            .navigationBarTitleDisplayMode(.large)
            .scrollDismissesKeyboard(.interactively)
            .safeAreaInset(edge: .bottom) {
                VStack(spacing: 0) {
                    Divider()
                    // Search results above search bar
                    if !searchVM.searchText.isEmpty {
                        if searchVM.results.isEmpty, !searchVM.isSearching {
                            Text(lang.string(.dictionaryNoResults))
                                .font(AppStyle.captionFont)
                                .foregroundStyle(.secondary)
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .padding(.horizontal, AppStyle.horizontalPadding)
                                .padding(.vertical, AppStyle.verticalPadding)
                        } else if !searchVM.results.isEmpty {
                            Divider()
                            ScrollView {
                                LazyVStack(spacing: 0) {
                                    let visible = Array(searchVM.results.prefix(5))
                                    ForEach(Array(visible.enumerated()), id: \.offset) { index, result in
                                        searchResultRow(result)
                                        if index < visible.count - 1 {
                                            Divider()
                                                .padding(.leading, 16)
                                        }
                                    }
                                }
                            }
                            .frame(maxHeight: 200)
                        }
                    }

                    // Search bar
                    HStack {
                        Image(latinSystemName: "magnifyingglass")
                            .foregroundStyle(.secondary)
                        TextField(
                            lang.string(.dictionarySearchPlaceholder),
                            text: $searchVM.searchText,
                        )
                        .focused($isSearchFocused)
                        .onChange(of: searchVM.searchText) { _, _ in
                            searchVM.onSearchTextChanged()
                        }
                        if !searchVM.searchText.isEmpty {
                            Button {
                                searchVM.searchText = ""
                                isSearchFocused = false
                            } label: {
                                Image(latinSystemName: "xmark.circle.fill")
                                    .foregroundStyle(.secondary)
                            }
                            .buttonStyle(.plain)
                        }
                    }
                    .padding(.horizontal, AppStyle.innerHorizontalPadding)
                    .padding(.vertical, AppStyle.verticalPadding)
                    .background(Color(.tertiarySystemFill))
                    .clipShape(RoundedRectangle(cornerRadius: AppStyle.cardCornerRadius, style: .continuous))
                    .padding(.horizontal, AppStyle.horizontalPadding)
                    .padding(.vertical, AppStyle.verticalPadding)
                }
                .background(Color(.systemBackground))
                .padding(.bottom, 8)
            }
        }
        .confirmationDialog("", isPresented: $showLookupDialog) {
            if let result = selectedResult {
                if let moeURL = result.moeURL {
                    Button(lang.string(.dictionaryLookupMoe)) {
                        UIApplication.shared.open(moeURL)
                    }
                }
                if let chhoeURL = result.chhoeURL {
                    Button(lang.string(.dictionaryLookupChhoe)) {
                        UIApplication.shared.open(chhoeURL)
                    }
                }
            }
            Button(lang.string(.commonCancel), role: .cancel) {}
        }
    }

    // MARK: - Search Result Row

    private func searchResultRow(_ result: DictionarySearchResult) -> some View {
        Button {
            selectedResult = result
            showLookupDialog = true
        } label: {
            HStack {
                Text(result.roman)
                    .foregroundStyle(.primary)
                if let hanji = result.hanji {
                    Text(hanji)
                        .foregroundStyle(.primary)
                }
                ForEach(uniqueTagKeys(for: result), id: \.self) { tagKey in
                    TagBadge(text: lang.string(tagKey))
                }
                Spacer()
                Image(latinSystemName: "arrow.up.right")
                    .font(AppStyle.captionFont)
                    .foregroundStyle(.secondary)
            }
            .padding(.horizontal, AppStyle.horizontalPadding)
            .padding(.vertical, 10)
        }
        .buttonStyle(.plain)
    }

    // MARK: - Source Badge Tags

    /// i18n key for a dictionary source's compact badge label. Resolved at the
    /// call site via the active display language, so the shared-core
    /// `DictionarySource` enum stays free of App-layer i18n types.
    private func tagKey(for source: DictionarySource) -> StringKey {
        switch source {
        case .kautian: .dictionaryKautianTag
        case .taigitv: .dictionaryTaigitvTag
        case .itaigi: .dictionaryITaigiTag
        case .sitbut: .dictionarySitbutTag
        case .taihoa: .dictionaryTaihoaTag
        case .taijit: .dictionaryTaijitTag
        case .kungge: .dictionaryKunggeTag
        case .stti: .dictionarySttiTag
        case .lkk: .dictionaryLkkTag
        // khpoo/khiin/dev/custom collapse to one "Supplementary Data" badge (reuses the section-title key).
        case .khpoo, .khiin, .dev, .custom: .dictionarySupplementSectionTitle
        }
    }

    /// Deduplicated badge keys for a result, preserving `sources` order. Dedup by
    /// key (not resolved string) so the 4 supplementary sources collapse to one
    /// badge regardless of the active display language.
    private func uniqueTagKeys(for result: DictionarySearchResult) -> [StringKey] {
        var seen = Set<StringKey>()
        return result.sources.compactMap { source in
            let key = tagKey(for: source)
            return seen.insert(key).inserted ? key : nil
        }
    }
}

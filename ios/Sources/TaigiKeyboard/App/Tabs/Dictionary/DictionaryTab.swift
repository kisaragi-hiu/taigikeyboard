import SwiftUI

/// Dictionary tab.
///
/// Entry rows for Manage Dictionaries and the keyboard data pages.
struct DictionaryTab: View {
    @Environment(DisplayLanguageStore.self) private var lang

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    NavigationLink(destination: DictionarySourcesView()) {
                        Text(lang.string(.desktopDictionarySourcesLink))
                    }
                    NavigationLink(destination: CustomDictionaryView()) {
                        Text(lang.string(.dictionaryCustomDictionary))
                    }
                    NavigationLink(destination: LearningRecordsMenuView()) {
                        Text(lang.string(.dictionaryLearningRecords))
                    }
                    NavigationLink(destination: DataManagementView()) {
                        Text(lang.string(.dictionaryBackupRestore))
                    }
                }
            }
            .navigationTitle(lang.string(TabType.dictionary.titleKey))
            .navigationBarTitleDisplayMode(.large)
        }
    }
}

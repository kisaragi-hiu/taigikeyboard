import SwiftUI

/// Learning Records subpage — one row per kind (word frequency, learned
/// phrases), each opening that kind's `LearningRecordsView`.
struct LearningRecordsMenuView: View {
    @Environment(DisplayLanguageStore.self) private var lang

    var body: some View {
        List {
            Section {
                NavigationLink(destination: LearningRecordsView(kind: .frequency)) {
                    Text(lang.string(Taigi_Engine_LearningRecordKind.frequency.titleKey))
                }
                NavigationLink(destination: LearningRecordsView(kind: .learnedPhrase)) {
                    Text(lang.string(Taigi_Engine_LearningRecordKind.learnedPhrase.titleKey))
                }
            }
        }
        .navigationTitle(lang.string(.dictionaryLearningRecords))
        .navigationBarTitleDisplayMode(.large)
    }
}

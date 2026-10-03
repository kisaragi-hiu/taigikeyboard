import SwiftUI

/// Learning Records subpage
/// What the keyboard learned from the user's picks, one kind at a time:
/// edit one row's count, swipe to delete one row. No add (a word the user
/// wants is a custom word) and no wipe — Delete Learning Records stays on
/// the Custom Dictionary page.
struct LearningRecordsView: View {
    @Environment(DisplayLanguageStore.self) private var lang
    @StateObject private var viewModel = LearningRecordsViewModel()

    @State private var filterText = ""
    @State private var showCountAlert = false
    @State private var editingRecord: Taigi_Engine_LearningRecord?
    @State private var countInput = ""

    var body: some View {
        List {
            if viewModel.isLoading {
                Section {
                    ProgressView()
                        .frame(maxWidth: .infinity)
                }
            } else {
                // Which kind, in which order
                Section {
                    Picker(
                        "",
                        selection: Binding(
                            get: { viewModel.kind },
                            set: { viewModel.selectKind($0) },
                        ),
                    ) {
                        Text(lang.string(.dictionaryLearningRecordsFrequency)).tag(Taigi_Engine_LearningRecordKind.frequency)
                        Text(lang.string(.dictionaryLearningRecordsPhrases)).tag(Taigi_Engine_LearningRecordKind.learnedPhrase)
                        Text(lang.string(.dictionaryLearningRecordsAssociation)).tag(Taigi_Engine_LearningRecordKind.association)
                    }
                    .pickerStyle(.segmented)
                    .labelsHidden()

                    Picker(
                        selection: Binding(
                            get: { viewModel.order },
                            set: { viewModel.selectOrder($0) },
                        ),
                    ) {
                        Text(lang.string(.dictionaryLearningRecordsOrderMostUsed)).tag(Taigi_Engine_LearningRecordOrder.mostUsed)
                        Text(lang.string(.dictionaryLearningRecordsOrderMostRecent)).tag(Taigi_Engine_LearningRecordOrder.mostRecent)
                    } label: {
                        Text(lang.string(.dictionaryLearningRecordsOrder))
                    }
                    .pickerStyle(.menu)
                } footer: {
                    Text(lang.string(.dictionaryLearningRecordsInfo))
                }

                // Record list
                Section {
                    if viewModel.records.isEmpty {
                        if viewModel.lastLoadFailed {
                            Text(lang.string(.dictionaryLearningRecordsReadFailed))
                                .foregroundColor(.secondary)
                        } else if filterText.isEmpty {
                            VStack(spacing: 16) {
                                Image(latinSystemName: "book.closed")
                                    .font(AppStyle.appFont(size: 48))
                                    .foregroundColor(.secondary)
                                Text(lang.string(.dictionaryLearningRecordsEmpty))
                                    .foregroundColor(.secondary)
                            }
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, 32)
                        } else {
                            Text(lang.string(.dictionaryNoResults))
                                .foregroundColor(.secondary)
                        }
                    } else {
                        ForEach(viewModel.records, id: \.id) { record in
                            recordRow(record)
                        }
                        if viewModel.hasMoreRows {
                            nextPageRow
                        }
                    }
                } header: {
                    Text(lang.string(.dictionaryLearningRecords))
                        .font(AppStyle.sectionHeaderFont)
                }
            }
        }
        .safeAreaInset(edge: .bottom) {
            SearchBar(text: $filterText, placeholder: lang.string(.dictionarySearchPlaceholder))
        }
        .onChange(of: filterText) { _, text in
            viewModel.filterChanged(text)
        }
        .navigationTitle(lang.string(.dictionaryLearningRecords))
        .navigationBarTitleDisplayMode(.large)
        .alert(lang.string(.dictionaryLearningRecordsEditCount), isPresented: $showCountAlert) {
            TextField(lang.string(.dictionaryLearningRecordsCount), text: $countInput)
                .keyboardType(.numberPad)
            Button(lang.string(.commonCancel), role: .cancel) {
                editingRecord = nil
            }
            Button(lang.string(.commonSave)) {
                saveCountFromAlert()
            }
            .disabled(LearningRecordsViewModel.count(from: countInput) == nil)
        } message: {
            // Only word frequency's boost stops growing (at count 40).
            if editingRecord?.kind == .frequency {
                Text(lang.string(.dictionaryLearningRecordsCountCapInfo))
            }
        }
        .alert(
            viewModel.notice.map { lang.string($0.titleKey) } ?? "",
            isPresented: Binding(
                get: { viewModel.notice != nil },
                set: {
                    if !$0 {
                        viewModel.notice = nil
                    }
                },
            ),
            presenting: viewModel.notice,
        ) { _ in
            Button(lang.string(.commonOk), role: .cancel) {}
        } message: { notice in
            if let detail = notice.detail {
                Text(detail)
            }
        }
        .task {
            await viewModel.load()
        }
    }

    // MARK: - Rows

    /// The list end: shown, it asks for the next page — again after every
    /// load that lands (`pagingKey`). After a failed read it waits for a tap.
    @ViewBuilder
    private var nextPageRow: some View {
        if viewModel.nextPageFailed {
            Button {
                Task { await viewModel.loadNextPage() }
            } label: {
                HStack {
                    Image(latinSystemName: "arrow.clockwise")
                    Text(lang.string(.dictionaryLearningRecordsReadFailed))
                }
                .foregroundColor(.secondary)
            }
        } else {
            ProgressView()
                .frame(maxWidth: .infinity)
                .task(id: viewModel.pagingKey) {
                    await viewModel.loadNextPage()
                }
        }
    }

    private func recordRow(_ record: Taigi_Engine_LearningRecord) -> some View {
        Button {
            editingRecord = record
            countInput = String(record.count)
            showCountAlert = true
        } label: {
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text(record.wordLabel)
                        .font(AppStyle.bodyFont)
                        .foregroundColor(.primary)
                    Text(record.readingLabel)
                        .font(AppStyle.captionFont)
                        .foregroundColor(.secondary)
                }
                Spacer()
                VStack(alignment: .trailing, spacing: 2) {
                    Text(String(record.count))
                        .font(AppStyle.bodyFont)
                        .monospacedDigit()
                        .foregroundColor(.primary)
                    Text(record.lastUsedLabel)
                        .font(AppStyle.captionFont)
                        .foregroundColor(.secondary)
                }
                Image(latinSystemName: "chevron.right")
                    .font(AppStyle.captionFont)
                    .foregroundColor(.secondary)
            }
        }
        .swipeActions(edge: .trailing) {
            Button(role: .destructive) {
                Task { await viewModel.delete(record) }
            } label: {
                Image(latinSystemName: "trash")
            }
        }
    }

    // MARK: - Actions

    private func saveCountFromAlert() {
        guard let record = editingRecord, let count = LearningRecordsViewModel.count(from: countInput) else { return }
        editingRecord = nil
        Task { await viewModel.setCount(record, to: count) }
    }
}

private extension LearningRecordsNotice {
    var titleKey: StringKey {
        switch self {
        case .readFailed: .dictionaryLearningRecordsReadFailed
        case .writeFailed: .dictionaryLearningRecordsWriteFailed
        case .gone: .dictionaryLearningRecordGone
        }
    }

    var detail: String? {
        switch self {
        case let .readFailed(detail), let .writeFailed(detail): detail
        case .gone: nil
        }
    }
}

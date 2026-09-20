import Foundation

// Custom dictionary entry model

// MARK: - Shared-Core Candidate

// Pure logic, Foundation-only. Eligible for cross-platform extraction.
struct CustomDictionaryEntry: Identifiable, Equatable {
    /// Who wrote the row (`behavioral-invariants.md` §50). Stored as the
    /// `origin` INTEGER column; the raw values are the wire / backup contract.
    // CROSS-PLATFORM INVARIANT — mirrors android/app/src/main/java/com/siansiansu/taigikeyboard/ime/dictionary/CustomDictionaryService.kt (Entry.Origin). Drift causes silent divergence.
    enum Origin: Int {
        /// Added or imported by the user.
        case manual = 0
        /// Learned from a segment-by-segment continuous composition.
        case learned = 1
    }

    let id: String
    var roman: String
    var hanzi: String
    let createdAt: Date
    var updatedAt: Date
    let origin: Origin
    /// Times the phrase was composed or picked; `0` for a manual row.
    let learnCount: Int

    init(
        id: String = UUID().uuidString,
        roman: String,
        hanzi: String,
        createdAt: Date = Date(),
        updatedAt: Date = Date(),
        origin: Origin = .manual,
        learnCount: Int = 0,
    ) {
        self.id = id
        self.roman = roman
        self.hanzi = hanzi
        self.createdAt = createdAt
        self.updatedAt = updatedAt
        self.origin = origin
        self.learnCount = learnCount
    }

    var isLearned: Bool { origin == .learned }

    /// The same row as the user's own word (`origin` manual, no count).
    var asManual: CustomDictionaryEntry {
        CustomDictionaryEntry(id: id, roman: roman, hanzi: hanzi, createdAt: createdAt, updatedAt: updatedAt)
    }
}

// Core candidate-word model — id / roman / hanji / lengthScore.

import Foundation

// MARK: - Shared-Core Candidate

/// Pure logic, Foundation-only. Eligible for cross-platform extraction.
public struct TaigiWord: Equatable {
    let id: Int
    let roman: String
    let hanji: String?
    let lengthScore: Int?

    init(
        id: Int,
        roman: String,
        hanji: String?,
        lengthScore: Int?,
    ) {
        self.id = id
        self.roman = roman
        self.hanji = hanji
        self.lengthScore = lengthScore
    }

    var displayText: String {
        if let hanji, !hanji.isEmpty {
            return hanji
        }
        return roman
    }
}

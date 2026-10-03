// What one cell holds, and how wide the cell that holds both scripts is.

@testable import TaigiInputMethodCore
import XCTest

/// A cell shows both scripts of a `(Hanji, romanization)` pair; which one
/// leads is desktop-core's (`composing/presentation.rs`). These cases are
/// about the cell itself.
final class CandidateCellContentTests: XCTestCase {
    /// Cells render at whatever metrics their panel was built with; these
    /// tests are about the cell's own behaviour, so they use the shipped
    /// defaults.
    private let metrics = TestFixtures.defaultCandidateMetrics

    func testEmptyAnnotationString_normalizesToNil() {
        XCTAssertNil(CandidateCellContent(text: "guá", annotation: "").annotation)
    }

    // MARK: - Cell reconfiguration

    /// Cells are recycled across pages and across the vertical layout's
    /// renumbering, so an annotation that goes away must give its width back —
    /// and coming back must take it again.
    @MainActor
    func testCellReconfiguration_tracksTheAnnotationBothWays() {
        let item = CandidateItemView(style: .sequoia, metrics: metrics)
        let annotated = CandidateCellContent(text: "tâi-gí", annotation: "台語")
        let bare = CandidateCellContent(text: "guá", annotation: nil)

        item.configure(annotated)
        let withAnnotation = item.fittingSize.width
        item.configure(bare)
        let withoutAnnotation = item.fittingSize.width
        item.configure(annotated)
        let withAnnotationAgain = item.fittingSize.width

        XCTAssertLessThan(withoutAnnotation, withAnnotation)
        XCTAssertEqual(withAnnotationAgain, withAnnotation, accuracy: 0.01)
    }

    /// The candidate column can be widened for column alignment, and never
    /// shrinks below the one-glyph floor.
    @MainActor
    func testPrimaryColumnWidth_widensTheCellAndFloorsAtOneGlyph() {
        let item = CandidateItemView(style: .sequoia, metrics: metrics)
        item.configure(CandidateCellContent(text: "guá", annotation: "我"))
        let natural = item.fittingSize.width

        item.setPrimaryColumnWidth(natural + 40)

        XCTAssertGreaterThan(item.fittingSize.width, natural)

        item.setPrimaryColumnWidth(0)

        XCTAssertEqual(item.fittingSize.width, natural, accuracy: 0.01)
    }

    // MARK: - Clamped cells

    /// A candidate longer than the cell it is clamped into truncates rather
    /// than breaking the cell's geometry: both the horizontal packer's row
    /// limit and the vertical window's column cap hand a cell less width than
    /// its text wants, and neither may push the label past the cell's edge.
    /// Both arrangements, since the stacked one centres its two lines in a
    /// text area the digit column has already taken width from.
    @MainActor
    func testCellClampedNarrowerThanItsText_keepsItsLabelsInside() {
        let cell = CandidateCellContent(
            text: "tâi-gí khí-puânn tsin hó-sè", annotation: "台語齒盤真好勢",
        )
        for arrangement in [CandidateCellArrangement.inline, .stacked] {
            let metrics = metrics.arranged(arrangement)
            let item = CandidateItemView(style: .sequoia, metrics: metrics)
            item.configure(cell)
            item.setIndexLabel("1")
            // Deliberately narrower than `measureWidth` wants — the clamp the
            // packer applies to an oversized candidate. Hosted in a container
            // the way the panels host their cells, because a detached view
            // never runs the layout pass that applies the frame to its
            // subviews.
            let clampedWidth = metrics.baseWidth * 2
            let container = FlippedContainerView(frame: NSRect(
                x: 0, y: 0, width: clampedWidth, height: metrics.itemHeight,
            ))
            container.addSubview(item)
            item.frame = container.bounds
            container.layoutSubtreeIfNeeded()

            let labels = item.subviews.compactMap { $0 as? NSTextField }
            XCTAssertEqual(
                labels.count, 3,
                "\(arrangement): the cell lays out a digit, a candidate and an annotation",
            )
            for label in labels {
                // Non-zero first, so a layout pass that never ran cannot pass
                // this case by leaving every frame at the origin.
                XCTAssertGreaterThan(
                    label.frame.width, 0,
                    "\(arrangement): \"\(label.stringValue)\" was never laid out",
                )
                XCTAssertGreaterThanOrEqual(
                    label.frame.minX, -0.01,
                    "\(arrangement): \"\(label.stringValue)\" runs past the cell's leading edge",
                )
                XCTAssertLessThanOrEqual(
                    label.frame.maxX, clampedWidth + 0.01,
                    "\(arrangement): \"\(label.stringValue)\" runs past the clamped cell's edge",
                )
            }
        }
    }
}

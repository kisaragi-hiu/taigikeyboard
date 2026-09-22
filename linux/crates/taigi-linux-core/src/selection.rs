//! The highlighted candidate and its page, for a list the DAEMON pages
//! (roadmap L4): the panel shows `page_size` cells per page from index 0,
//! and the engine only tells it which absolute index is highlighted. So
//! this is the part of `HorizontalListModel` that decides rather than
//! measures — with no widths, a "page" is a fixed run of cells.
//!
//! `Down` / `PageDown` step a page, `Right` / `Next` a cell, exactly as
//! `HorizontalPageLayout::target` reads the six directions; a step past
//! either end is refused (the highlight stays), as on the Mac.

use taigi_desktop_core::keys::CandidateNavigation;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LookupSelection {
    count: usize,
    page_size: usize,
    selected: usize,
}

impl LookupSelection {
    pub fn new(count: usize, page_size: usize) -> Self {
        Self {
            count,
            page_size: page_size.max(1),
            selected: 0,
        }
    }

    /// The highlighted absolute index, `None` for an empty list.
    pub fn selected_index(&self) -> Option<usize> {
        (self.count > 0).then_some(self.selected)
    }

    pub fn current_page(&self) -> usize {
        self.selected / self.page_size
    }

    fn page_count(&self) -> usize {
        self.count.div_ceil(self.page_size)
    }

    /// The absolute index the `slot`-th key on the current page addresses,
    /// `None` past the end of a short last page (the key is consumed all the
    /// same — `SelectCandidateSlot`).
    pub fn candidate_index_for_key_slot(&self, slot: usize) -> Option<usize> {
        if slot >= self.page_size {
            return None;
        }
        let index = self.current_page() * self.page_size + slot;
        (index < self.count).then_some(index)
    }

    /// The absolute index of the `position`-th cell on the current page —
    /// what the panel reports for a click.
    pub fn candidate_index_on_page(&self, position: usize) -> Option<usize> {
        self.candidate_index_for_key_slot(position)
    }

    pub fn select(&mut self, index: usize) {
        if index < self.count {
            self.selected = index;
        }
    }

    /// Answers whether the highlight moved.
    pub fn navigate(&mut self, direction: CandidateNavigation) -> bool {
        let Some(target) = self.target(direction) else {
            return false;
        };
        self.selected = target;
        true
    }

    fn target(&self, direction: CandidateNavigation) -> Option<usize> {
        if self.count == 0 {
            return None;
        }
        match direction {
            CandidateNavigation::Right | CandidateNavigation::NextCandidate => {
                (self.selected + 1 < self.count).then_some(self.selected + 1)
            }
            CandidateNavigation::Left | CandidateNavigation::PreviousCandidate => {
                self.selected.checked_sub(1)
            }
            CandidateNavigation::Down | CandidateNavigation::PageDown => {
                let next_page = self.current_page() + 1;
                (next_page < self.page_count())
                    .then(|| (self.selected + self.page_size).min(self.count - 1))
            }
            CandidateNavigation::Up | CandidateNavigation::PageUp => self
                .current_page()
                .checked_sub(1)
                .map(|_| self.selected - self.page_size),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_step_and_pages_step_with_the_ends_refused() {
        // trace: 20 cells, 9 per page → pages [0..9), [9..18), [18..20).
        let mut selection = LookupSelection::new(20, 9);
        assert_eq!(selection.selected_index(), Some(0));
        assert!(!selection.navigate(CandidateNavigation::Left));
        assert!(selection.navigate(CandidateNavigation::Right));
        assert_eq!(selection.selected_index(), Some(1));
        assert!(selection.navigate(CandidateNavigation::PageDown));
        assert_eq!(selection.selected_index(), Some(10));
        assert_eq!(selection.current_page(), 1);
        assert!(selection.navigate(CandidateNavigation::Down));
        // Past the last page's short end: clamped to the last cell.
        assert_eq!(selection.selected_index(), Some(19));
        assert!(!selection.navigate(CandidateNavigation::PageDown));
        assert!(selection.navigate(CandidateNavigation::Up));
        assert_eq!(selection.selected_index(), Some(10));
        assert!(selection.navigate(CandidateNavigation::PreviousCandidate));
        assert_eq!(selection.selected_index(), Some(9));
    }

    #[test]
    fn slot_keys_address_the_current_page_and_refuse_a_short_pages_gap() {
        let mut selection = LookupSelection::new(11, 9);
        assert_eq!(selection.candidate_index_for_key_slot(3), Some(3));
        selection.select(10);
        assert_eq!(selection.current_page(), 1);
        assert_eq!(selection.candidate_index_for_key_slot(0), Some(9));
        assert_eq!(selection.candidate_index_for_key_slot(1), Some(10));
        assert_eq!(selection.candidate_index_for_key_slot(2), None);
        assert_eq!(selection.candidate_index_for_key_slot(9), None);
    }

    #[test]
    fn an_empty_list_has_no_selection_and_moves_nowhere() {
        let mut selection = LookupSelection::new(0, 9);
        assert_eq!(selection.selected_index(), None);
        assert!(!selection.navigate(CandidateNavigation::Right));
        assert_eq!(selection.candidate_index_for_key_slot(0), None);
    }
}

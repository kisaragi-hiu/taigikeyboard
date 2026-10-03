//! The paging arithmetic every listed page shares.

/// The offset a page starts at: the requested one, pulled back to the last
/// page that exists — the matches can shrink under the page a platform is
/// on (a delete on the last page).
pub(crate) fn last_page_offset(matching_total: usize, limit: usize, requested: usize) -> usize {
    let limit = limit.max(1);
    requested.min(matching_total.saturating_sub(1) / limit * limit)
}

/// A count for the wire, which carries `u32`.
pub(crate) fn saturated_u32(count: impl TryInto<u32>) -> u32 {
    count.try_into().unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_offset_past_the_end_is_pulled_back_to_the_last_page() {
        // trace: 5 matches, pages of 2 → offsets 0, 2, 4.
        assert_eq!(last_page_offset(5, 2, 40), 4);
        assert_eq!(last_page_offset(5, 2, 2), 2);
        assert_eq!(last_page_offset(4, 2, 4), 2, "a full last page");
        assert_eq!(last_page_offset(0, 2, 40), 0);
        assert_eq!(last_page_offset(5, usize::MAX, 3), 0, "one page holds all");
        assert_eq!(saturated_u32(-1_i64), u32::MAX);
        assert_eq!(saturated_u32(7_usize), 7);
    }
}

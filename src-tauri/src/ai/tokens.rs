//! Token estimation without a model-specific tokenizer.
//!
//! Counts are deliberately conservative: one token per CJK character and about
//! one token per 3.5 other characters, which over-estimates for most current
//! models so budgets stay inside the real context window.

use crate::search::segment::is_cjk;

pub fn estimate(text: &str) -> usize {
    let (cjk, other) = text
        .chars()
        .fold((0usize, 0usize), |(c, o), ch| if is_cjk(ch) { (c + 1, o) } else { (c, o + 1) });
    cjk + (other * 10).div_ceil(35) + 1
}

/// Per-message overhead used by chat formats (role markers, separators).
pub const MESSAGE_OVERHEAD: usize = 6;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chinese_counts_per_character_and_ascii_is_cheaper() {
        assert_eq!(estimate("导数"), 3);
        let english = estimate("The derivative measures the rate of change.");
        assert!(english > 8 && english < 20, "{english}");
        assert!(estimate(&"字".repeat(1000)) >= 1000);
    }
}

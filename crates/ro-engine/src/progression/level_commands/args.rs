//! Command argument parsing.

/// `atoi` of the first word: `None` for nothing, a non-number or 0.
pub(super) fn level_argument(params: &str) -> Option<i32> {
    params
        .split_whitespace()
        .next()
        .and_then(|word| word.parse::<i32>().ok())
        .filter(|&n| n != 0)
}

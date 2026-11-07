use super::args::level_argument;
use crate::progression::points;

#[test]
fn level_argument_is_atoi_of_the_first_word() {
    assert_eq!(level_argument("10"), Some(10));
    assert_eq!(level_argument(" -5 extra"), Some(-5));
    for bad in ["", "0", "abc", "+"] {
        assert_eq!(level_argument(bad), None, "{bad:?}");
    }
}

/// Finish review m-2: points never outgrow their `smallint` columns.
#[test]
fn points_cap_at_the_column_range() {
    assert_eq!(points(48), 48);
    assert_eq!(points(32_767), 32_767);
    assert_eq!(points(32_768), 32_767);
    assert_eq!(points(u32::MAX), 32_767);
}

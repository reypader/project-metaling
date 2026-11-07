use super::parse_item_params;

#[test]
fn item_params() {
    assert_eq!(parse_item_params("501 5"), Some(("501", 5)));
    assert_eq!(parse_item_params("knife"), Some(("knife", 1)));
    assert_eq!(
        parse_item_params("\"Red Potion\" 3"),
        Some(("Red Potion", 3))
    );
    assert_eq!(parse_item_params("\"Red Potion\""), Some(("Red Potion", 1)));
    assert_eq!(parse_item_params("a:b -4"), Some(("a:b", 1)));
    assert_eq!(parse_item_params("a 0"), Some(("a", 1)));
    assert_eq!(parse_item_params("a lots"), Some(("a", 1)));
    assert_eq!(parse_item_params(""), None);
    assert_eq!(parse_item_params("\"unclosed 3"), None);
    assert_eq!(parse_item_params("\"\" 3"), None);
}

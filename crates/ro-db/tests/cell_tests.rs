//! `.gat` cell classification (spec DB-02).

use ro_db::CellType;

#[test]
fn db_02_cell_type_from_raw_maps_rathena_values() {
    assert_eq!(CellType::from_raw(0), CellType::Walkable);
    assert_eq!(CellType::from_raw(1), CellType::Wall);
    assert_eq!(CellType::from_raw(2), CellType::Walkable2);
    assert_eq!(CellType::from_raw(3), CellType::Water);
    assert_eq!(CellType::from_raw(4), CellType::Walkable4);
    assert_eq!(CellType::from_raw(5), CellType::Cliff);
    assert_eq!(CellType::from_raw(6), CellType::Walkable6);
    assert_eq!(CellType::from_raw(7), CellType::Unknown(7));
    assert_eq!(CellType::from_raw(255), CellType::Unknown(255));
}

#[test]
fn db_02_cell_type_walkability() {
    for raw in [0u8, 2, 3, 4, 6] {
        assert!(
            CellType::from_raw(raw).is_walkable(),
            "raw {raw} must be walkable"
        );
    }
    for raw in [1u8, 5, 7, 8, 200, 255] {
        assert!(
            !CellType::from_raw(raw).is_walkable(),
            "raw {raw} must not be walkable"
        );
    }
}

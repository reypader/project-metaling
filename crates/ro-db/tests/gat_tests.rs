//! `.gat` parsing and in-memory walkability grids (spec DB-01..04).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use ro_db::map::{GatMapGrid, MemoryMapGrid};
use ro_db::{CellType, MapDataError, MapWalkability};

const HEADER: &[u8; 6] = b"GRAT\x01\x02";

/// Builds a `.gat` buffer: header, u32 LE dimensions, then one 20-byte record per cell
/// with the raw type at record offset 16.
fn gat_bytes(width: u32, height: u32, types: &[u32]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(HEADER);
    buf.extend_from_slice(&width.to_le_bytes());
    buf.extend_from_slice(&height.to_le_bytes());
    for &t in types {
        buf.extend_from_slice(&1.5f32.to_le_bytes());
        buf.extend_from_slice(&[0u8; 12]);
        buf.extend_from_slice(&t.to_le_bytes());
    }
    buf
}

#[test]
fn db_01_parses_row_major_grid() {
    // Row 0: 0 1 2, row 1: 3 5 9.
    let grid = GatMapGrid::from_bytes("t.gat", &gat_bytes(3, 2, &[0, 1, 2, 3, 5, 9]))
        .expect("valid 3x2 gat parses");
    assert_eq!(grid.width(), 3);
    assert_eq!(grid.height(), 2);
    assert_eq!(grid.cell_type(0, 0), Some(CellType::Walkable));
    assert_eq!(grid.cell_type(1, 0), Some(CellType::Wall));
    assert_eq!(grid.cell_type(2, 0), Some(CellType::Walkable2));
    assert_eq!(grid.cell_type(0, 1), Some(CellType::Water));
    assert_eq!(grid.cell_type(1, 1), Some(CellType::Cliff));
    assert_eq!(grid.cell_type(2, 1), Some(CellType::Unknown(9)));
    assert!(grid.is_walkable(0, 0));
    assert!(!grid.is_walkable(1, 0));
    assert!(grid.is_walkable(0, 1));
    assert!(!grid.is_walkable(1, 1));
    assert!(!grid.is_walkable(2, 1));
}

#[test]
fn db_01_raw_type_above_u8_is_unknown_and_not_walkable() {
    let grid = GatMapGrid::from_bytes("t.gat", &gat_bytes(1, 1, &[0x1_0000])).expect("parses");
    assert!(matches!(grid.cell_type(0, 0), Some(CellType::Unknown(_))));
    assert!(!grid.is_walkable(0, 0));
}

#[test]
fn db_01_bad_magic() {
    let mut buf = gat_bytes(1, 1, &[0]);
    buf[0] = b'X';
    assert_eq!(
        GatMapGrid::from_bytes("bad.gat", &buf),
        Err(MapDataError::BadMagic {
            path: "bad.gat".to_string()
        })
    );
}

#[test]
fn db_01_truncated_header() {
    let buf = gat_bytes(1, 1, &[0]);
    assert!(matches!(
        GatMapGrid::from_bytes("t.gat", &buf[..13]),
        Err(MapDataError::Truncated {
            expected: 14,
            actual: 13,
            ..
        })
    ));
    assert!(matches!(
        GatMapGrid::from_bytes("t.gat", &[]),
        Err(MapDataError::Truncated { .. })
    ));
}

#[test]
fn db_01_truncated_cells() {
    let buf = gat_bytes(3, 2, &[0, 0, 0, 0, 0]); // one record short
    assert_eq!(
        GatMapGrid::from_bytes("t.gat", &buf),
        Err(MapDataError::Truncated {
            path: "t.gat".to_string(),
            expected: 14 + 6 * 20,
            actual: 14 + 5 * 20,
        })
    );
}

#[test]
fn db_01_invalid_dimensions() {
    for (w, h) in [(0, 2), (2, 0), (u16::MAX as u32 + 1, 1), (1, u32::MAX)] {
        let buf = gat_bytes(w, h, &[]);
        assert_eq!(
            GatMapGrid::from_bytes("t.gat", &buf),
            Err(MapDataError::InvalidDimensions {
                path: "t.gat".to_string(),
                width: w,
                height: h
            }),
            "{w}x{h} must be rejected"
        );
    }
}

#[test]
fn db_01_load_reads_file_and_wraps_io_errors() {
    let dir = std::env::temp_dir().join(format!("ro-db-gat-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("tiny.gat");
    std::fs::write(&path, gat_bytes(2, 2, &[0, 1, 1, 0])).expect("write gat");

    let grid = GatMapGrid::load(&path).expect("load parses file");
    assert_eq!((grid.width(), grid.height()), (2, 2));
    assert!(!grid.is_walkable(1, 0));

    let missing = dir.join("missing.gat");
    match GatMapGrid::load(&missing) {
        Err(MapDataError::Io { path, .. }) => assert!(path.ends_with("missing.gat")),
        other => panic!("expected Io error, got {other:?}"),
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn db_03_bounds_never_panic() {
    let grid =
        GatMapGrid::from_bytes("t.gat", &gat_bytes(3, 2, &[0, 0, 0, 0, 0, 1])).expect("parses");
    assert_eq!(grid.cell_type(0, 0), Some(CellType::Walkable));
    assert_eq!(grid.cell_type(2, 1), Some(CellType::Wall));
    for (x, y) in [
        (3, 0),
        (0, 2),
        (u16::MAX, 0),
        (0, u16::MAX),
        (u16::MAX, u16::MAX),
    ] {
        assert_eq!(grid.cell_type(x, y), None, "({x},{y})");
        assert!(!grid.is_walkable(x, y), "({x},{y})");
    }
}

#[test]
fn db_04_memory_grid_builder() {
    let mut grid = MemoryMapGrid::all_walkable(10, 5).expect("valid dims");
    assert_eq!((grid.width(), grid.height()), (10, 5));
    assert!(grid.is_walkable(0, 0));
    assert!(grid.is_walkable(9, 4));
    assert!(grid.set(3, 2, CellType::Wall));
    assert!(!grid.is_walkable(3, 2));
    assert_eq!(grid.cell_type(3, 2), Some(CellType::Wall));
    assert!(
        !grid.set(10, 0, CellType::Wall),
        "out of bounds set reports false"
    );
    assert!(!grid.is_walkable(10, 0));
    assert_eq!(grid.cell_type(0, 5), None);
}

#[test]
fn db_04_memory_grid_zero_dimensions() {
    assert!(matches!(
        MemoryMapGrid::all_walkable(0, 5),
        Err(MapDataError::InvalidDimensions {
            width: 0,
            height: 5,
            ..
        })
    ));
    assert!(matches!(
        MemoryMapGrid::all_walkable(5, 0),
        Err(MapDataError::InvalidDimensions { .. })
    ));
}

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn db_04_grids_are_shareable_trait_objects() {
    assert_send_sync::<MemoryMapGrid>();
    assert_send_sync::<GatMapGrid>();
    let mem: Arc<dyn MapWalkability> =
        Arc::new(MemoryMapGrid::all_walkable(4, 4).expect("valid dims"));
    assert!(mem.is_walkable(1, 1));
    let shared: Arc<dyn MapWalkability> = MemoryMapGrid::all_walkable(4, 4)
        .expect("valid dims")
        .into();
    assert_eq!(shared.width(), 4);
}

#[test]
fn db_04_memory_grid_set_round_trips_every_variant() {
    let mut grid = MemoryMapGrid::all_walkable(2, 2).expect("valid dims");
    for cell in [
        CellType::Walkable,
        CellType::Wall,
        CellType::Walkable2,
        CellType::Water,
        CellType::Walkable4,
        CellType::Cliff,
        CellType::Walkable6,
        CellType::Unknown(9),
    ] {
        assert!(grid.set(1, 1, cell));
        assert_eq!(grid.cell_type(1, 1), Some(cell));
        assert_eq!(grid.is_walkable(1, 1), cell.is_walkable(), "{cell:?}");
    }
}

#[test]
fn db_04_unknown_with_known_raw_value_stays_unwalkable() {
    let mut grid = MemoryMapGrid::all_walkable(2, 2).expect("valid dims");
    for raw in [0u8, 2, 3, 4, 6] {
        assert!(grid.set(0, 0, CellType::Unknown(raw)));
        assert!(
            !grid.is_walkable(0, 0),
            "Unknown({raw}) must not become walkable"
        );
        assert!(matches!(grid.cell_type(0, 0), Some(CellType::Unknown(_))));
    }
}

#[test]
fn db_01_accepts_other_grat_versions() {
    // rAthena reads sizes at fixed offsets and never checks the version bytes.
    let mut buf = gat_bytes(1, 1, &[0]);
    buf[5] = 0x03;
    let grid = GatMapGrid::from_bytes("v13.gat", &buf).expect("GRAT 1.3 parses");
    assert!(grid.is_walkable(0, 0));
}

#[test]
fn db_debug_output_omits_cells() {
    let grid = MemoryMapGrid::all_walkable(512, 512).expect("valid dims");
    let text = format!("{grid:?}");
    assert!(
        text.len() < 100,
        "Debug should print dimensions only: {text}"
    );
    assert!(text.contains("512"));
}

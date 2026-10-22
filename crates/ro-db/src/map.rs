//! Map walkability grids: `.gat` files and in-memory fixtures.
//!
//! Both store one raw cell type byte per cell, row-major (`x + y * width`).
//! The `MapWalkability` view stays raw: the "last row/column is never walkable"
//! rule belongs to the pathfinder, not here.

use std::path::Path;
use std::sync::Arc;

use ro_contracts::{CellType, MapDataError, MapWalkability};

/// rAthena reads sizes at fixed offsets and never checks the magic; we check the `GRAT` tag
/// but accept any version bytes (offsets 4..6, `\x01\x02` in shipped maps).
const GAT_MAGIC: &[u8; 4] = b"GRAT";
const GAT_HEADER_LEN: usize = 14;
const GAT_RECORD_LEN: usize = 20;
/// Offset of the `u32` cell type inside a record (after four `f32` corner altitudes).
const GAT_TYPE_OFFSET: usize = 16;

/// Raw type stored for `.gat` values that don't fit a byte; any unknown value is not walkable.
const UNKNOWN_RAW: u8 = u8::MAX;

/// Row-major raw cell storage shared by both grid types.
#[derive(Clone, PartialEq, Eq)]
struct CellGrid {
    width: u16,
    height: u16,
    cells: Vec<u8>,
}

impl CellGrid {
    fn index(&self, x: u16, y: u16) -> Option<usize> {
        (x < self.width && y < self.height)
            .then(|| usize::from(y) * usize::from(self.width) + usize::from(x))
    }

    fn cell_type(&self, x: u16, y: u16) -> Option<CellType> {
        let idx = self.index(x, y)?;
        self.cells.get(idx).copied().map(CellType::from_raw)
    }

    fn is_walkable(&self, x: u16, y: u16) -> bool {
        self.cell_type(x, y).is_some_and(CellType::is_walkable)
    }
}

fn checked_dimensions(path: &str, width: u32, height: u32) -> Result<(u16, u16), MapDataError> {
    let invalid = || MapDataError::InvalidDimensions {
        path: path.to_string(),
        width,
        height,
    };
    if width == 0 || height == 0 {
        return Err(invalid());
    }
    let w = u16::try_from(width).map_err(|_| invalid())?;
    let h = u16::try_from(height).map_err(|_| invalid())?;
    Ok((w, h))
}

fn read_u32_le(buf: &[u8], offset: usize) -> Option<u32> {
    let bytes = buf.get(offset..offset + 4)?;
    let arr: [u8; 4] = bytes.try_into().ok()?;
    Some(u32::from_le_bytes(arr))
}

/// Debug prints only the dimensions; a real map has 100k+ cells.
impl std::fmt::Debug for CellGrid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CellGrid")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}

/// Walkability grid parsed from a client `.gat` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatMapGrid {
    grid: CellGrid,
}

impl GatMapGrid {
    /// Parses `.gat` bytes. `path` is only a label for errors.
    pub fn from_bytes(path: &str, buf: &[u8]) -> Result<Self, MapDataError> {
        let truncated = |expected: usize| MapDataError::Truncated {
            path: path.to_string(),
            expected,
            actual: buf.len(),
        };
        if buf.len() < GAT_HEADER_LEN {
            return Err(truncated(GAT_HEADER_LEN));
        }
        if !buf.starts_with(GAT_MAGIC) {
            return Err(MapDataError::BadMagic {
                path: path.to_string(),
            });
        }
        let raw_w = read_u32_le(buf, 6).ok_or_else(|| truncated(GAT_HEADER_LEN))?;
        let raw_h = read_u32_le(buf, 10).ok_or_else(|| truncated(GAT_HEADER_LEN))?;
        let (width, height) = checked_dimensions(path, raw_w, raw_h)?;

        let cell_count = usize::from(width) * usize::from(height);
        let expected = cell_count
            .checked_mul(GAT_RECORD_LEN)
            .and_then(|n| n.checked_add(GAT_HEADER_LEN))
            .ok_or_else(|| MapDataError::InvalidDimensions {
                path: path.to_string(),
                width: raw_w,
                height: raw_h,
            })?;
        let body = buf
            .get(GAT_HEADER_LEN..expected)
            .ok_or_else(|| truncated(expected))?;

        let cells = body
            .chunks_exact(GAT_RECORD_LEN)
            .map(|record| {
                read_u32_le(record, GAT_TYPE_OFFSET)
                    .and_then(|raw| u8::try_from(raw).ok())
                    .unwrap_or(UNKNOWN_RAW)
            })
            .collect();

        Ok(Self {
            grid: CellGrid {
                width,
                height,
                cells,
            },
        })
    }

    /// Reads and parses a `.gat` file. I/O failures become `MapDataError::Io`.
    pub fn load(path: &Path) -> Result<Self, MapDataError> {
        let label = path.display().to_string();
        let buf = std::fs::read(path).map_err(|err| MapDataError::Io {
            path: label.clone(),
            message: err.to_string(),
        })?;
        Self::from_bytes(&label, &buf)
    }
}

impl MapWalkability for GatMapGrid {
    fn width(&self) -> u16 {
        self.grid.width
    }
    fn height(&self) -> u16 {
        self.grid.height
    }
    fn is_walkable(&self, x: u16, y: u16) -> bool {
        self.grid.is_walkable(x, y)
    }
    fn cell_type(&self, x: u16, y: u16) -> Option<CellType> {
        self.grid.cell_type(x, y)
    }
}

impl From<GatMapGrid> for Arc<dyn MapWalkability> {
    fn from(grid: GatMapGrid) -> Self {
        Arc::new(grid)
    }
}

/// In-memory grid for tests and the runtime fallback when a `.gat` file is missing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryMapGrid {
    grid: CellGrid,
}

impl MemoryMapGrid {
    /// A `width` x `height` grid where every cell is `CellType::Walkable`.
    pub fn all_walkable(width: u16, height: u16) -> Result<Self, MapDataError> {
        let (width, height) = checked_dimensions("<memory>", width.into(), height.into())?;
        Ok(Self {
            grid: CellGrid {
                width,
                height,
                cells: vec![0; usize::from(width) * usize::from(height)],
            },
        })
    }

    /// Sets one cell. Returns `false` (and changes nothing) when out of bounds.
    #[must_use]
    pub fn set(&mut self, x: u16, y: u16, cell: CellType) -> bool {
        let Some(slot) = self
            .grid
            .index(x, y)
            .and_then(|idx| self.grid.cells.get_mut(idx))
        else {
            return false;
        };
        *slot = raw_of(cell);
        true
    }
}

fn raw_of(cell: CellType) -> u8 {
    match cell {
        CellType::Walkable => 0,
        CellType::Wall => 1,
        CellType::Walkable2 => 2,
        CellType::Water => 3,
        CellType::Walkable4 => 4,
        CellType::Cliff => 5,
        CellType::Walkable6 => 6,
        // Unknown(r) with a known r (0..=6) must not read back as a known (maybe walkable) type.
        CellType::Unknown(raw) if raw <= 6 => UNKNOWN_RAW,
        CellType::Unknown(raw) => raw,
    }
}

impl MapWalkability for MemoryMapGrid {
    fn width(&self) -> u16 {
        self.grid.width
    }
    fn height(&self) -> u16 {
        self.grid.height
    }
    fn is_walkable(&self, x: u16, y: u16) -> bool {
        self.grid.is_walkable(x, y)
    }
    fn cell_type(&self, x: u16, y: u16) -> Option<CellType> {
        self.grid.cell_type(x, y)
    }
}

impl From<MemoryMapGrid> for Arc<dyn MapWalkability> {
    fn from(grid: MemoryMapGrid) -> Self {
        Arc::new(grid)
    }
}

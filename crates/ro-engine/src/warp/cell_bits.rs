//! A per-map bitset of cells, for the cells inside warp trigger areas.

/// One bit per map cell.
#[derive(Debug)]
pub(crate) struct CellBits {
    width: u16,
    height: u16,
    words: Vec<u64>,
}

impl CellBits {
    pub(crate) fn new(width: u16, height: u16) -> Self {
        let cells = usize::from(width) * usize::from(height);
        Self {
            width,
            height,
            words: vec![0; cells.div_ceil(64)],
        }
    }

    fn bit(&self, x: u16, y: u16) -> Option<usize> {
        (x < self.width && y < self.height)
            .then(|| usize::from(y) * usize::from(self.width) + usize::from(x))
    }

    pub(crate) fn set(&mut self, x: u16, y: u16) {
        if let Some(b) = self.bit(x, y) {
            if let Some(word) = self.words.get_mut(b / 64) {
                *word |= 1 << (b % 64);
            }
        }
    }

    pub(crate) fn get(&self, x: u16, y: u16) -> bool {
        self.bit(x, y)
            .and_then(|b| self.words.get(b / 64).map(|w| w & (1 << (b % 64)) != 0))
            .unwrap_or(false)
    }

    /// Whether any set cell lies within Chebyshev distance 1 of `(x, y)`.
    pub(crate) fn any_near(&self, x: u16, y: u16) -> bool {
        let xs = x.saturating_sub(1)..=x.saturating_add(1);
        xs.into_iter()
            .any(|cx| (y.saturating_sub(1)..=y.saturating_add(1)).any(|cy| self.get(cx, cy)))
    }
}

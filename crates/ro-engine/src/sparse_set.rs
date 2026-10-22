//! `SparseSet<T>`: entity-indexed component storage with contiguous dense slices.

use crate::entity::Entity;

const EMPTY: u32 = u32::MAX;

/// `sparse[entity.index()]` points into the dense arrays. Lookups also compare the stored
/// entity, so a stale handle (old generation) never sees the new occupant's data.
#[derive(Debug)]
pub(crate) struct SparseSet<T> {
    sparse: Vec<u32>,
    dense_entities: Vec<Entity>,
    dense_data: Vec<T>,
}

impl<T> Default for SparseSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> SparseSet<T> {
    pub(crate) fn new() -> Self {
        Self {
            sparse: Vec::new(),
            dense_entities: Vec::new(),
            dense_data: Vec::new(),
        }
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.dense_data.len()
    }

    fn dense_index(&self, entity: Entity) -> Option<usize> {
        let slot = *self.sparse.get(entity.index())?;
        if slot == EMPTY {
            return None;
        }
        let slot = slot as usize;
        (self.dense_entities.get(slot) == Some(&entity)).then_some(slot)
    }

    pub(crate) fn contains(&self, entity: Entity) -> bool {
        self.dense_index(entity).is_some()
    }

    pub(crate) fn get(&self, entity: Entity) -> Option<&T> {
        self.dense_index(entity)
            .and_then(|slot| self.dense_data.get(slot))
    }

    pub(crate) fn get_mut(&mut self, entity: Entity) -> Option<&mut T> {
        self.dense_index(entity)
            .and_then(|slot| self.dense_data.get_mut(slot))
    }

    /// Inserts or replaces; returns the previous value for this exact entity.
    pub(crate) fn insert(&mut self, entity: Entity, value: T) -> Option<T> {
        if let Some(slot) = self.dense_index(entity) {
            return self
                .dense_data
                .get_mut(slot)
                .map(|old| std::mem::replace(old, value));
        }
        // A stale occupant of the same index (older generation) is dropped first.
        if let Some(&slot) = self.sparse.get(entity.index()) {
            if slot != EMPTY {
                if let Some(&stale) = self.dense_entities.get(slot as usize) {
                    self.remove(stale);
                }
            }
        }
        if self.sparse.len() <= entity.index() {
            self.sparse.resize(entity.index() + 1, EMPTY);
        }
        // Dense length is bounded by the entity capacity (u32), so this fits.
        let slot = u32::try_from(self.dense_data.len()).unwrap_or(EMPTY);
        if let Some(sparse) = self.sparse.get_mut(entity.index()) {
            *sparse = slot;
        }
        self.dense_entities.push(entity);
        self.dense_data.push(value);
        None
    }

    /// Swap-removes; the moved element's sparse entry is repointed. Missing is a no-op.
    pub(crate) fn remove(&mut self, entity: Entity) -> Option<T> {
        let slot = self.dense_index(entity)?;
        let last = self.dense_data.len().checked_sub(1)?;
        self.dense_entities.swap(slot, last);
        self.dense_data.swap(slot, last);
        self.dense_entities.pop();
        let value = self.dense_data.pop();
        if let Some(sparse) = self.sparse.get_mut(entity.index()) {
            *sparse = EMPTY;
        }
        if slot != last {
            if let Some(&moved) = self.dense_entities.get(slot) {
                if let Some(sparse) = self.sparse.get_mut(moved.index()) {
                    *sparse = slot as u32;
                }
            }
        }
        value
    }

    pub(crate) fn entities(&self) -> &[Entity] {
        &self.dense_entities
    }

    #[cfg(test)]
    pub(crate) fn values(&self) -> &[T] {
        &self.dense_data
    }

    #[cfg(test)]
    pub(crate) fn values_mut(&mut self) -> &mut [T] {
        &mut self.dense_data
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::Entity;

    fn e(index: u32, generation: u32) -> Entity {
        Entity::new(index, generation)
    }

    #[test]
    fn eng_02_insert_get_contains() {
        let mut set = SparseSet::new();
        assert_eq!(set.insert(e(3, 1), "a"), None);
        assert!(set.contains(e(3, 1)));
        assert_eq!(set.get(e(3, 1)), Some(&"a"));
        assert_eq!(set.insert(e(3, 1), "b"), Some("a"), "insert replaces");
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn eng_02_remove_swap_keeps_other_entries_reachable() {
        let mut set = SparseSet::new();
        set.insert(e(0, 1), 10);
        set.insert(e(5, 1), 50);
        set.insert(e(9, 1), 90);
        assert_eq!(set.remove(e(0, 1)), Some(10));
        assert_eq!(set.get(e(5, 1)), Some(&50));
        assert_eq!(set.get(e(9, 1)), Some(&90));
        assert!(!set.contains(e(0, 1)));
        assert_eq!(set.len(), 2);
        if let Some(v) = set.get_mut(e(9, 1)) {
            *v = 91;
        }
        assert_eq!(set.get(e(9, 1)), Some(&91));
    }

    #[test]
    fn eng_02_dense_slices_iterate() {
        let mut set = SparseSet::new();
        set.insert(e(2, 1), 'x');
        set.insert(e(4, 1), 'y');
        assert_eq!(set.entities(), &[e(2, 1), e(4, 1)]);
        assert_eq!(set.values(), &['x', 'y']);
        for v in set.values_mut() {
            *v = 'z';
        }
        assert!(set.values().iter().all(|&v| v == 'z'));
    }

    #[test]
    fn eng_02_remove_missing_is_noop() {
        let mut set: SparseSet<u8> = SparseSet::new();
        assert_eq!(set.remove(e(1, 1)), None);
        set.insert(e(1, 1), 1);
        assert_eq!(set.remove(e(100, 1)), None);
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn eng_02_stale_handle_returns_none() {
        let mut set = SparseSet::new();
        set.insert(e(1, 2), 7);
        assert_eq!(set.get(e(1, 1)), None);
        assert!(!set.contains(e(1, 1)));
        assert_eq!(set.remove(e(1, 1)), None);
        assert_eq!(set.get(e(1, 2)), Some(&7));
    }
}

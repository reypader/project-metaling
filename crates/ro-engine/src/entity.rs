//! Generational entity handles and their allocator.

use ro_contracts::EngineError;

/// A slot index plus the generation it was allocated with. A handle is stale once its slot
/// is freed, even if the slot is later reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Entity {
    index: u32,
    generation: u32,
}

impl Entity {
    pub(crate) const fn new(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }

    /// Index into the dense columns.
    pub(crate) const fn index(self) -> usize {
        self.index as usize
    }

    #[cfg(test)]
    pub(crate) const fn generation(self) -> u32 {
        self.generation
    }
}

/// Hands out entity slots up to a fixed capacity and recycles freed ones.
#[derive(Debug)]
pub(crate) struct EntityAllocator {
    /// Current generation per slot; generations start at 1.
    generations: Vec<u32>,
    alive: Vec<bool>,
    free: Vec<u32>,
    capacity: u32,
}

impl EntityAllocator {
    pub(crate) fn with_capacity(capacity: u32) -> Self {
        Self {
            generations: Vec::new(),
            alive: Vec::new(),
            free: Vec::new(),
            capacity,
        }
    }

    /// Number of slots ever created (live, free, or retired); dense columns must be this long.
    pub(crate) fn slot_count(&self) -> usize {
        self.generations.len()
    }

    pub(crate) fn allocate(&mut self) -> Result<Entity, EngineError> {
        if let Some(index) = self.free.pop() {
            let slot = index as usize;
            if let (Some(generation), Some(alive)) =
                (self.generations.get_mut(slot), self.alive.get_mut(slot))
            {
                // Retired slots never enter the free list, so this cannot wrap.
                *generation = generation.saturating_add(1);
                *alive = true;
                return Ok(Entity::new(index, *generation));
            }
        }
        let index = u32::try_from(self.generations.len())
            .ok()
            .filter(|&i| i < self.capacity)
            .ok_or(EngineError::WorldCapacityExceeded)?;
        self.generations.push(1);
        self.alive.push(true);
        Ok(Entity::new(index, 1))
    }

    /// Frees a live entity. A stale or unknown handle is an error and changes nothing.
    pub(crate) fn free(&mut self, entity: Entity) -> Result<(), EngineError> {
        if !self.is_alive(entity) {
            return Err(EngineError::EntityNotFound(entity.index));
        }
        if let Some(alive) = self.alive.get_mut(entity.index()) {
            *alive = false;
        }
        // A slot whose generation can't grow any more is retired, so no stale handle revives.
        if entity.generation < u32::MAX {
            self.free.push(entity.index);
        }
        Ok(())
    }

    pub(crate) fn is_alive(&self, entity: Entity) -> bool {
        let slot = entity.index();
        self.alive.get(slot).copied().unwrap_or(false)
            && self.generations.get(slot) == Some(&entity.generation)
    }

    #[cfg(test)]
    pub(crate) fn force_generation_for_test(&mut self, index: usize, generation: u32) {
        self.generations[index] = generation;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ro_contracts::EngineError;

    #[test]
    fn eng_01_first_allocation_is_index_0_generation_1() {
        let mut alloc = EntityAllocator::with_capacity(4);
        let e = alloc.allocate().expect("capacity available");
        assert_eq!(e.index(), 0);
        assert_eq!(e.generation(), 1);
        assert!(alloc.is_alive(e));
    }

    #[test]
    fn eng_01_free_then_allocate_reuses_index_with_next_generation() {
        let mut alloc = EntityAllocator::with_capacity(4);
        let first = alloc.allocate().expect("alloc");
        alloc.free(first).expect("free live entity");
        assert!(!alloc.is_alive(first), "stale handle is dead");
        let second = alloc.allocate().expect("alloc");
        assert_eq!(second.index(), 0);
        assert_eq!(second.generation(), 2);
        assert!(alloc.is_alive(second));
        assert!(
            !alloc.is_alive(first),
            "old generation stays dead after reuse"
        );
    }

    #[test]
    fn eng_01_double_free_errors_and_keeps_free_list_intact() {
        let mut alloc = EntityAllocator::with_capacity(4);
        let a = alloc.allocate().expect("alloc");
        let b = alloc.allocate().expect("alloc");
        alloc.free(a).expect("first free");
        assert!(alloc.free(a).is_err(), "double free must error");
        // Only one slot (index 0) is free: next alloc reuses it, the one after is fresh.
        let c = alloc.allocate().expect("alloc");
        let d = alloc.allocate().expect("alloc");
        assert_eq!(c.index(), 0);
        assert_eq!(d.index(), 2);
        assert!(alloc.is_alive(b));
    }

    #[test]
    fn eng_01_allocating_past_capacity_fails() {
        let mut alloc = EntityAllocator::with_capacity(2);
        alloc.allocate().expect("1");
        alloc.allocate().expect("2");
        assert_eq!(alloc.allocate(), Err(EngineError::WorldCapacityExceeded));
    }

    #[test]
    fn eng_01_generation_wrap_retires_slot_instead_of_reviving() {
        let mut alloc = EntityAllocator::with_capacity(2);
        let e = alloc.allocate().expect("alloc");
        alloc.force_generation_for_test(e.index(), u32::MAX);
        let max_gen = Entity::new(0, u32::MAX);
        assert!(alloc.is_alive(max_gen));
        alloc.free(max_gen).expect("free");
        let next = alloc.allocate().expect("alloc");
        assert_ne!(next.index(), 0, "a slot at u32::MAX generation is retired");
        assert!(!alloc.is_alive(Entity::new(0, 0)));
        assert!(!alloc.is_alive(Entity::new(0, 1)));
        assert!(!alloc.is_alive(max_gen));
    }

    #[test]
    fn eng_01_unknown_index_is_not_alive() {
        let alloc = EntityAllocator::with_capacity(2);
        assert!(!alloc.is_alive(Entity::new(7, 1)));
    }
}

//! Allocator for generational entity slots.

use super::Entity;
use crate::error::EngineError;

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
mod tests;

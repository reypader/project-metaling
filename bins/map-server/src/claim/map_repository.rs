//! The repository the map-server reads and writes.

use ro_persistence::{CharacterRepository, InventoryRepository};

/// Everything the map-server reads and writes in the database.
pub trait MapRepository: CharacterRepository + InventoryRepository {}

impl<T: CharacterRepository + InventoryRepository + ?Sized> MapRepository for T {}

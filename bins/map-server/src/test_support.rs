//! Fakes and helpers shared by the map-server unit tests.

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use async_trait::async_trait;
use ro_core::{
    AccountId, CharId, CharacterInventory, CharacterLooks, CharacterProgress, CharacterSummary,
    CreateCharacterRequest, InventoryItem, ItemId, LearnedSkill, MapPosition, Sex,
};
use ro_engine::PlaySnapshot;
use ro_persistence::{
    CancelDeleteOutcome, CharacterRepository, CreateCharacterError, DeleteReserveOutcome,
    InventoryRepository, PlayState, RepositoryError,
};
use tokio::sync::Semaphore;

use crate::claim::lock;

pub(crate) const CHAR: CharId = CharId(150_001);
pub(crate) const ACCOUNT: AccountId = AccountId(2_000_001);

pub(crate) fn at(x: u16, y: u16) -> MapPosition {
    MapPosition { x, y, dir: 0 }
}

/// One character on `prontera.gat` at (100, 100). Each write waits for a `gate` permit.
pub(crate) struct FakeRepo {
    chars: Mutex<HashMap<CharId, CharacterSummary>>,
    pub(crate) writes: Mutex<Vec<(CharId, String, MapPosition)>>,
    pub(crate) finds: AtomicUsize,
    pub(crate) gate: Semaphore,
    /// Writes that fail before one succeeds.
    pub(crate) failures: AtomicUsize,
    /// Stored inventories and the inventory saves, in order.
    pub(crate) inventories: Mutex<HashMap<CharId, CharacterInventory>>,
    pub(crate) inventory_saves: Mutex<Vec<(CharId, CharacterInventory)>>,
    /// The looks of the last inventory save, by character.
    pub(crate) looks: Mutex<HashMap<CharId, CharacterLooks>>,
    /// The error every inventory save returns, if set.
    pub(crate) inventory_error: Mutex<Option<RepositoryError>>,
}

impl FakeRepo {
    pub(crate) fn new(open_gate: bool) -> Arc<Self> {
        let summary = CharacterSummary {
            char_id: CHAR,
            account_id: ACCOUNT,
            slot: 0,
            name: "Char1".into(),
            sex: Sex::Female,
            job_class: 0,
            base_level: 1,
            job_level: 1,
            hair_style: 1,
            hair_color: 1,
            last_map: "prontera.gat".into(),
            last_pos: at(100, 100),
            delete_date: None,
            looks: CharacterLooks::default(),
            select_stats: Default::default(),
        };
        let permits = if open_gate { Semaphore::MAX_PERMITS } else { 0 };
        Arc::new(Self {
            chars: Mutex::new(HashMap::from([(CHAR, summary)])),
            writes: Mutex::new(Vec::new()),
            finds: AtomicUsize::new(0),
            gate: Semaphore::new(permits),
            failures: AtomicUsize::new(0),
            inventories: Mutex::new(HashMap::new()),
            inventory_saves: Mutex::new(Vec::new()),
            looks: Mutex::new(HashMap::new()),
            inventory_error: Mutex::new(None),
        })
    }

    pub(crate) fn writes(&self) -> Vec<(CharId, String, MapPosition)> {
        lock(&self.writes).clone()
    }
}

#[async_trait]
impl CharacterRepository for FakeRepo {
    async fn list_by_account(
        &self,
        _account_id: AccountId,
    ) -> Result<Vec<CharacterSummary>, RepositoryError> {
        Ok(Vec::new())
    }
    async fn find_by_slot(
        &self,
        _account_id: AccountId,
        _slot: u8,
    ) -> Result<Option<CharacterSummary>, RepositoryError> {
        Ok(None)
    }
    async fn find_by_id(&self, id: CharId) -> Result<Option<CharacterSummary>, RepositoryError> {
        self.finds.fetch_add(1, Ordering::Relaxed);
        Ok(lock(&self.chars).get(&id).cloned())
    }
    async fn create_character(
        &self,
        _req: CreateCharacterRequest,
        _max_slots: u8,
    ) -> Result<CharacterSummary, CreateCharacterError> {
        Err(CreateCharacterError::CreationDenied)
    }
    async fn reserve_delete(
        &self,
        _account_id: AccountId,
        _char_id: CharId,
        _delay_seconds: u32,
    ) -> Result<DeleteReserveOutcome, RepositoryError> {
        Ok(DeleteReserveOutcome::Denied)
    }
    async fn cancel_delete(
        &self,
        _account_id: AccountId,
        _char_id: CharId,
    ) -> Result<CancelDeleteOutcome, RepositoryError> {
        Ok(CancelDeleteOutcome::NotFound)
    }
    async fn update_position(
        &self,
        id: CharId,
        map_name: &str,
        pos: MapPosition,
    ) -> Result<(), RepositoryError> {
        self.gate.acquire().await.expect("gate open").forget();
        if self
            .failures
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_sub(1))
            .is_ok()
        {
            return Err(RepositoryError::Unavailable("injected failure".into()));
        }
        lock(&self.writes).push((id, map_name.to_string(), pos));
        if let Some(c) = lock(&self.chars).get_mut(&id) {
            c.last_map = map_name.to_string();
            c.last_pos = pos;
        }
        Ok(())
    }
}

#[async_trait]
impl InventoryRepository for FakeRepo {
    async fn load_play_state(&self, char_id: CharId) -> Result<Option<PlayState>, RepositoryError> {
        if !lock(&self.chars).contains_key(&char_id) {
            return Ok(None);
        }
        Ok(Some(PlayState {
            status: Default::default(),
            group_id: 0,
            inventory: lock(&self.inventories)
                .get(&char_id)
                .cloned()
                .unwrap_or_default(),
            skills: Vec::new(),
            change_level: [0; 3],
        }))
    }

    async fn save_play_state(
        &self,
        char_id: CharId,
        inventory: &CharacterInventory,
        looks: &CharacterLooks,
        _progress: &CharacterProgress,
        _skills: &[LearnedSkill],
    ) -> Result<(), RepositoryError> {
        self.gate.acquire().await.expect("gate open").forget();
        if let Some(err) = lock(&self.inventory_error).clone() {
            return Err(err);
        }
        lock(&self.inventory_saves).push((char_id, inventory.clone()));
        lock(&self.looks).insert(char_id, *looks);
        lock(&self.inventories).insert(char_id, inventory.clone());
        Ok(())
    }
}

pub(crate) fn potions(n: u16) -> CharacterInventory {
    CharacterInventory {
        slots: vec![Some(InventoryItem::new(ItemId(501), n))],
        unique_item_counter: 0,
    }
}

/// `potions(n)` with bare looks, as the engine would snapshot it.
pub(crate) fn snapshot(n: u16) -> PlaySnapshot {
    PlaySnapshot {
        inventory: potions(n),
        looks: CharacterLooks::default(),
        progress: CharacterProgress::default(),
        skills: Vec::new(),
    }
}

/// Polls `cond` every 5 ms for up to 2 s.
pub(crate) async fn eventually(cond: impl Fn() -> bool) -> bool {
    for _ in 0..400 {
        if cond() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    false
}

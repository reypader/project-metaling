//! An in-memory character and inventory repository, and the rows tests seed it with.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use ro_core::{
    AccountId, CharId, CharacterInventory, CharacterLooks, CharacterProgress, CharacterSelectStats,
    CharacterStatus, CharacterSummary, CreateCharacterRequest, InventoryItem, ItemId, LearnedSkill,
    MapPosition, Sex, SkillFlag,
};
use ro_persistence::{
    CancelDeleteOutcome, CharacterRepository, CreateCharacterError, DeleteReserveOutcome,
    InventoryRepository, PlayState, RepositoryError,
};

use super::*;

/// Characters keyed by char id; positions update on `update_position`.
#[derive(Default)]
pub struct InMemoryRepo {
    chars: Mutex<HashMap<CharId, CharacterSummary>>,
    /// Simulated latency of `update_position` and `save_play_state` (a slow database).
    persist_delay: Mutex<Duration>,
    /// Stored inventories, account groups and STR by character.
    inventories: Mutex<HashMap<CharId, CharacterInventory>>,
    groups: Mutex<HashMap<CharId, i32>>,
    /// Stored stats, points, exp and HP/SP (job and base level live in `chars`).
    statuses: Mutex<HashMap<CharId, CharacterStatus>>,
    /// Every inventory save, in order.
    pub inventory_saves: Mutex<Vec<(CharId, CharacterInventory)>>,
    /// Stored skill rows and class-change levels by character.
    skills: Mutex<HashMap<CharId, Vec<LearnedSkill>>>,
    change_levels: Mutex<HashMap<CharId, [u16; 3]>>,
}

pub fn row(id: u16, level: u8) -> LearnedSkill {
    LearnedSkill {
        id,
        level,
        flag: SkillFlag::Permanent,
    }
}

impl InMemoryRepo {
    /// Characters `n` (char 15000n, account 200000n) named `Char<n>` at the given cells.
    pub fn seeded(chars: &[(u32, u16, u16)]) -> Self {
        let on_map: Vec<_> = chars.iter().map(|&(n, x, y)| (n, MAP, x, y)).collect();
        Self::seeded_on(&on_map)
    }

    /// Like `seeded`, with each character's stored map.
    pub fn seeded_on(chars: &[(u32, &str, u16, u16)]) -> Self {
        let repo = Self::default();
        for &(n, map, x, y) in chars {
            repo.chars.lock().unwrap().insert(
                CharId(char_id(n)),
                CharacterSummary {
                    char_id: CharId(char_id(n)),
                    account_id: AccountId(account(n)),
                    slot: 0,
                    name: format!("Char{n}"),
                    sex: Sex::Female,
                    job_class: 0,
                    base_level: 10,
                    job_level: 1,
                    hair_style: 2,
                    hair_color: 3,
                    last_map: map.to_string(),
                    last_pos: MapPosition { x, y, dir: 0 },
                    delete_date: None,
                    looks: CharacterLooks::default(),
                    select_stats: Default::default(),
                },
            );
        }
        repo
    }

    /// Moves character `n` to another account.
    pub fn set_account(&self, n: u32, account_id: u32) {
        if let Some(c) = self.chars.lock().unwrap().get_mut(&CharId(char_id(n))) {
            c.account_id = AccountId(account_id);
        }
    }

    pub fn with_persist_delay(self, delay: Duration) -> Self {
        *self.persist_delay.lock().unwrap() = delay;
        self
    }

    /// Makes character `n`'s account a GM (group 99).
    pub fn set_gm(&self, n: u32) {
        self.groups.lock().unwrap().insert(CharId(char_id(n)), 99);
    }

    pub fn set_inventory(&self, n: u32, inventory: CharacterInventory) {
        self.inventories
            .lock()
            .unwrap()
            .insert(CharId(char_id(n)), inventory);
    }

    pub fn stored_inventory(&self, n: u32) -> Option<CharacterInventory> {
        self.inventories
            .lock()
            .unwrap()
            .get(&CharId(char_id(n)))
            .cloned()
    }

    pub fn inventory_saves_of(&self, n: u32) -> usize {
        self.inventory_saves
            .lock()
            .unwrap()
            .iter()
            .filter(|(c, _)| *c == CharId(char_id(n)))
            .count()
    }

    /// The look columns of character `n` (what the char list shows).
    pub fn stored_looks(&self, n: u32) -> Option<CharacterLooks> {
        self.chars
            .lock()
            .unwrap()
            .get(&CharId(char_id(n)))
            .map(|c| c.looks)
    }

    /// Stores `progress` for character `n`, as a save would.
    pub fn set_progress(&self, n: u32, progress: CharacterProgress) {
        let id = CharId(char_id(n));
        if let Some(c) = self.chars.lock().unwrap().get_mut(&id) {
            c.job_class = progress.job_class;
            c.base_level = progress.base_level;
            c.job_level = progress.status.job_level;
        }
        self.statuses.lock().unwrap().insert(id, progress.status);
        self.change_levels
            .lock()
            .unwrap()
            .insert(id, progress.change_level);
    }

    /// The stored progress of character `n` (what the next login loads).
    pub fn stored_progress(&self, n: u32) -> Option<CharacterProgress> {
        let id = CharId(char_id(n));
        let (job_class, base_level) = self
            .chars
            .lock()
            .unwrap()
            .get(&id)
            .map(|c| (c.job_class, c.base_level))?;
        let status = self
            .statuses
            .lock()
            .unwrap()
            .get(&id)
            .copied()
            .unwrap_or_default();
        Some(CharacterProgress {
            job_class,
            base_level,
            status,
            change_level: self
                .change_levels
                .lock()
                .unwrap()
                .get(&id)
                .copied()
                .unwrap_or_default(),
        })
    }

    /// The stored skill rows of character `n` (by raw char id).
    pub fn stored_skills(&self, raw_char_id: u32) -> Vec<LearnedSkill> {
        self.skills
            .lock()
            .unwrap()
            .get(&CharId(raw_char_id))
            .cloned()
            .unwrap_or_default()
    }

    /// Seeds character `n`'s skill rows.
    pub fn set_skills(&self, n: u32, rows: Vec<LearnedSkill>) {
        self.skills.lock().unwrap().insert(CharId(char_id(n)), rows);
    }

    pub fn stored(&self, n: u32) -> Option<(String, MapPosition)> {
        self.chars
            .lock()
            .unwrap()
            .get(&CharId(char_id(n)))
            .map(|c| (c.last_map.clone(), c.last_pos))
    }
}

#[async_trait]
impl CharacterRepository for InMemoryRepo {
    async fn list_by_account(
        &self,
        account_id: AccountId,
    ) -> Result<Vec<CharacterSummary>, RepositoryError> {
        Ok(self
            .chars
            .lock()
            .unwrap()
            .values()
            .filter(|c| c.account_id == account_id)
            .cloned()
            .collect())
    }
    async fn find_by_slot(
        &self,
        account_id: AccountId,
        slot: u8,
    ) -> Result<Option<CharacterSummary>, RepositoryError> {
        Ok(self
            .chars
            .lock()
            .unwrap()
            .values()
            .find(|c| c.account_id == account_id && c.slot == slot)
            .cloned())
    }
    async fn find_by_id(&self, id: CharId) -> Result<Option<CharacterSummary>, RepositoryError> {
        Ok(self.chars.lock().unwrap().get(&id).cloned())
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
        let delay = *self.persist_delay.lock().unwrap();
        if !delay.is_zero() {
            tokio::time::sleep(delay).await;
        }
        if let Some(c) = self.chars.lock().unwrap().get_mut(&id) {
            c.last_map = map_name.to_string();
            c.last_pos = pos;
        }
        Ok(())
    }
}

#[async_trait]
impl InventoryRepository for InMemoryRepo {
    async fn load_play_state(&self, id: CharId) -> Result<Option<PlayState>, RepositoryError> {
        if !self.chars.lock().unwrap().contains_key(&id) {
            return Ok(None);
        }
        Ok(Some(PlayState {
            status: self
                .statuses
                .lock()
                .unwrap()
                .get(&id)
                .copied()
                .unwrap_or_default(),
            group_id: self.groups.lock().unwrap().get(&id).copied().unwrap_or(0),
            inventory: self
                .inventories
                .lock()
                .unwrap()
                .get(&id)
                .cloned()
                .unwrap_or_default(),
            skills: self.stored_skills(id.0),
            change_level: self
                .change_levels
                .lock()
                .unwrap()
                .get(&id)
                .copied()
                .unwrap_or_default(),
        }))
    }

    async fn save_play_state(
        &self,
        id: CharId,
        inventory: &CharacterInventory,
        looks: &CharacterLooks,
        progress: &CharacterProgress,
        skills: &[LearnedSkill],
    ) -> Result<(), RepositoryError> {
        let delay = *self.persist_delay.lock().unwrap();
        if !delay.is_zero() {
            tokio::time::sleep(delay).await;
        }
        // Like the database: rows load back without holes.
        let compact = CharacterInventory {
            slots: inventory.items().cloned().map(Some).collect(),
            unique_item_counter: inventory.unique_item_counter,
        };
        self.inventory_saves
            .lock()
            .unwrap()
            .push((id, inventory.clone()));
        self.inventories.lock().unwrap().insert(id, compact);
        // Like the database: the look columns and the progress are written with the inventory.
        if let Some(c) = self.chars.lock().unwrap().get_mut(&id) {
            c.looks = *looks;
            c.job_class = progress.job_class;
            c.base_level = progress.base_level;
            c.job_level = progress.status.job_level;
            let status = &progress.status;
            c.select_stats = CharacterSelectStats {
                base_exp: status.base_exp,
                job_exp: status.job_exp,
                hp: status.hp,
                max_hp: status.max_hp,
                sp: status.sp,
                max_sp: status.max_sp,
                status_point: status.status_point,
                skill_point: status.skill_point,
                stats: [
                    status.str, status.agi, status.vit, status.int, status.dex, status.luk,
                ],
                zeny: status.zeny,
            };
        }
        self.statuses.lock().unwrap().insert(id, progress.status);
        // Like the database: only levels above 0 are rows.
        let rows = skills.iter().filter(|s| s.level > 0).copied().collect();
        self.skills.lock().unwrap().insert(id, rows);
        self.change_levels
            .lock()
            .unwrap()
            .insert(id, progress.change_level);
        Ok(())
    }
}

/// An inventory of `(item id, amount)` stacks, one per slot.
pub fn inventory(items: &[(u32, u16)]) -> CharacterInventory {
    CharacterInventory {
        slots: items
            .iter()
            .map(|&(id, n)| Some(InventoryItem::new(ItemId(id), n)))
            .collect(),
        unique_item_counter: 0,
    }
}

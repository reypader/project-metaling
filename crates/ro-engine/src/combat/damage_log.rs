//! Per-mob damage log: who hit a mob and for how much.

use ro_core::CharId;
use smallvec::SmallVec;

use crate::combat::state_dto::DamageEntry;

/// rAthena `DAMAGELOG_SIZE`: the oldest attacker drops out past this many.
const DAMAGE_LOG_SIZE: usize = 20;

/// A mob's damage log, oldest first, at most `DAMAGE_LOG_SIZE` attackers (spec §2.7).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DamageLog(SmallVec<[DamageEntry; 4]>);

impl DamageLog {
    /// Adds to the attacker's entry, or appends one, evicting the oldest when full.
    pub(crate) fn add(&mut self, char_id: CharId, damage: u64) {
        if let Some(entry) = self.0.iter_mut().find(|e| e.char_id == char_id) {
            entry.damage = entry.damage.saturating_add(damage);
            return;
        }
        if self.0.len() >= DAMAGE_LOG_SIZE {
            self.0.remove(0);
        }
        self.0.push(DamageEntry { char_id, damage });
    }

    pub(crate) fn entries(&self) -> &[DamageEntry] {
        &self.0
    }
}

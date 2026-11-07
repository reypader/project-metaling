//! Mob drops: the roll and the placement (spec `specs/items/spec.md` §2.6; catalog
//! `mechanics/mobs.md` §4). Not called at mob death yet: there is no combat. The combat feature
//! calls `drop_mob_items` in the death tick, after the exp packets (PF-15).

use ro_core::{CharId, ItemId};
use ro_db::{ItemDb, ItemType, MobData, MobId, MAX_DROP_RATE};

mod systems;

/// rAthena `drop_rate_cap`: bonuses can't push a rate past 90% unless the base rate already was.
const DROP_RATE_CAP: u16 = 9000;

/// A dead mob's drop: what died, where, and who has loot priority (top damage dealers).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MobDropRequest {
    pub mob_id: MobId,
    pub cell: (u16, u16),
    pub owners: [Option<CharId>; 3],
}

/// rAthena's drop roll without the deferred parts (level penalty, player bonuses): each `Drops`
/// entry in db order drops one item when `roll() % 10000 < rate`.
pub fn roll_mob_drops(mob: &MobData, mut roll: impl FnMut() -> u32) -> Vec<ItemId> {
    let mut dropped = Vec::new();
    for entry in &mob.drops {
        let base = entry.rate;
        // Bonuses would be applied here; with none the rate is the base rate.
        let mut rate = base;
        if rate > DROP_RATE_CAP && base <= DROP_RATE_CAP {
            rate = DROP_RATE_CAP;
        }
        let rate = rate.clamp(1, MAX_DROP_RATE);
        if roll() % u32::from(MAX_DROP_RATE) < u32::from(rate) {
            dropped.push(entry.item);
        }
    }
    dropped
}

/// rAthena `itemdb_isidentified`: equipment drops unidentified, everything else identified.
fn identified_drop(items: &ItemDb, id: ItemId) -> bool {
    !items.get(id).is_some_and(|d| {
        matches!(
            d.item_type,
            ItemType::Weapon | ItemType::Armor | ItemType::PetArmor | ItemType::ShadowGear
        )
    })
}

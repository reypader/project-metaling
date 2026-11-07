//! A worn set seen through the item db: weapon type, looks and the gear status totals.

use ro_core::{CharacterLooks, InventoryItem, ItemId, UnitLooks};
use ro_db::{ItemData, ItemDb, ItemType, RefineBonus, RefineDb, RefineGroup, WeaponType};

use crate::equipment::{
    GearStatus, WeaponGear, WornSlots, EQI_ORDER, EQP_AMMO, EQP_ARMS, EQP_COSTUME_GARMENT,
    EQP_COSTUME_HEAD_LOW, EQP_COSTUME_HEAD_MID, EQP_COSTUME_HEAD_TOP, EQP_GARMENT, EQP_HAND_L,
    EQP_HAND_R, EQP_HEAD_LOW, EQP_HEAD_MID, EQP_HEAD_TOP, W_DOUBLE_AA, W_DOUBLE_DA, W_DOUBLE_DD,
    W_DOUBLE_DS, W_DOUBLE_SA, W_DOUBLE_SS,
};

/// A worn item: its slot and the item.
type Held<'a> = Option<(usize, &'a InventoryItem)>;

/// A worn set with what it takes to read it: the worn-slot table, the inventory slots and the
/// item registry. Looks and gear values come from the table, not from the items' `equip`
/// masks, so they're right mid-displacement too (spec §2.2 step 10).
#[derive(Clone, Copy)]
pub(crate) struct Gear<'a> {
    worn: &'a WornSlots,
    slots: &'a [Option<InventoryItem>],
    items: &'a ItemDb,
}

impl<'a> Gear<'a> {
    pub(crate) fn new(
        worn: &'a WornSlots,
        slots: &'a [Option<InventoryItem>],
        items: &'a ItemDb,
    ) -> Self {
        Self { worn, slots, items }
    }

    pub(crate) fn worn(&self) -> &'a WornSlots {
        self.worn
    }

    /// The slot and item at a position.
    fn item_at(&self, position: u32) -> Held<'a> {
        let slot = self.worn.at(position)?;
        self.slots.get(slot)?.as_ref().map(|item| (slot, item))
    }

    pub(crate) fn data(&self, id: ItemId) -> Option<&'a ItemData> {
        self.items.get(id)
    }

    /// The right-hand slot, and the left-hand slot when it's a different item.
    fn hands(&self) -> (Held<'a>, Held<'a>) {
        let right = self.item_at(EQP_HAND_R);
        let left = self
            .item_at(EQP_HAND_L)
            .filter(|(l, _)| right.is_none_or(|(r, _)| r != *l));
        (right, left)
    }

    fn weapon_type_of(&self, hand: Held<'_>) -> WeaponType {
        hand.and_then(|(_, item)| self.data(item.item_id))
            .filter(|d| d.item_type == ItemType::Weapon)
            .map_or(WeaponType::Fist, |d| d.weapon_type)
    }

    /// rAthena `status.weapon`: one hand's type, or the dual-wield constant for a
    /// dagger/sword/axe pair; `W_FIST` (0) bare-handed.
    pub(crate) fn weapon_type(&self) -> u16 {
        let (right, left) = self.hands();
        let (r, l) = (self.weapon_type_of(right), self.weapon_type_of(left));
        use WeaponType::{Dagger, Fist, OneHandAxe as Axe, OneHandSword as Sword};
        match (r, l) {
            (Fist, l) => l.id().into(),
            (r, Fist) => r.id().into(),
            (Dagger, Dagger) => W_DOUBLE_DD,
            (Sword, Sword) => W_DOUBLE_SS,
            (Axe, Axe) => W_DOUBLE_AA,
            (Dagger, Sword) | (Sword, Dagger) => W_DOUBLE_DS,
            (Dagger, Axe) | (Axe, Dagger) => W_DOUBLE_DA,
            (Sword, Axe) | (Axe, Sword) => W_DOUBLE_SA,
            (r, _) => r.id().into(),
        }
    }

    /// The view of the item at `position`, or 0 when that item also covers a higher position
    /// in `higher` (a multi-slot headgear shows in its highest slot only, by its own
    /// `Locations`).
    fn view_at(&self, position: u32, higher: u32) -> Option<u16> {
        let (_, item) = self.item_at(position)?;
        Some(self.data(item.item_id).map_or(0, |d| {
            if d.locations & higher != 0 {
                0
            } else {
                saturate(d.view)
            }
        }))
    }

    /// Head bottom, top, mid and robe views after the costume override.
    fn head_views(&self) -> (u16, u16, u16, u16) {
        let low = self.view_at(EQP_HEAD_LOW, EQP_HEAD_MID | EQP_HEAD_TOP);
        let mid = self.view_at(EQP_HEAD_MID, EQP_HEAD_TOP);
        let top = self.view_at(EQP_HEAD_TOP, 0);
        let robe = self.view_at(EQP_GARMENT, 0);
        let c_low = self.view_at(
            EQP_COSTUME_HEAD_LOW,
            EQP_COSTUME_HEAD_MID | EQP_COSTUME_HEAD_TOP,
        );
        let c_mid = self.view_at(EQP_COSTUME_HEAD_MID, EQP_COSTUME_HEAD_TOP);
        let c_top = self.view_at(EQP_COSTUME_HEAD_TOP, 0);
        let c_robe = self.view_at(EQP_COSTUME_GARMENT, 0);
        (
            c_low.or(low).unwrap_or(0),
            c_top.or(top).unwrap_or(0),
            c_mid.or(mid).unwrap_or(0),
            c_robe.or(robe).unwrap_or(0),
        )
    }

    fn client_id(&self, item: &InventoryItem) -> u32 {
        self.data(item.item_id)
            .map_or(item.item_id.0, ItemData::client_id)
    }

    /// What other players see: hand item ids (a left-hand weapon or shield in `shield`, 0 for
    /// the second hand of a two-hander) and the head and robe views.
    pub(crate) fn unit_looks(&self) -> UnitLooks {
        let (right, left) = self.hands();
        let (head_bottom, head_top, head_mid, robe) = self.head_views();
        UnitLooks {
            weapon: right.map_or(0, |(_, i)| self.client_id(i)),
            shield: left.map_or(0, |(_, i)| self.client_id(i)),
            head_bottom,
            head_top,
            head_mid,
            robe,
        }
    }

    /// What char select shows: the weapon type, the left-hand armor's `View` (0 for a
    /// left-hand weapon), and the head and robe views.
    pub(crate) fn character_looks(&self) -> CharacterLooks {
        let (_, left) = self.hands();
        let shield = left
            .and_then(|(_, item)| self.data(item.item_id))
            .filter(|d| d.item_type == ItemType::Armor)
            .map_or(0, |d| saturate(d.view));
        let (head_bottom, head_top, head_mid, robe) = self.head_views();
        CharacterLooks {
            weapon_type: self.weapon_type(),
            shield,
            head_top,
            head_mid,
            head_bottom,
            robe,
        }
    }
}

/// Bare-handed attack range in cells.
const BARE_HAND_RANGE: u16 = 1;

impl Gear<'_> {
    /// The hands as the status reads them: like `hands`, but a two-hander holding only
    /// `HAND_L` is the right-hand weapon.
    fn status_hands(&self) -> (Held<'_>, Held<'_>) {
        let (right, left) = self.hands();
        let two_hander = left.is_some_and(|(_, item)| {
            self.data(item.item_id).is_some_and(|d| {
                d.item_type == ItemType::Weapon && d.locations & EQP_ARMS == EQP_ARMS
            })
        });
        if right.is_none() && two_hander {
            (left, None)
        } else {
            (right, left)
        }
    }

    fn weapon_gear(&self, hand: Held<'_>, refine: &RefineDb) -> Option<WeaponGear> {
        let (_, item) = hand?;
        let data = self
            .data(item.item_id)
            .filter(|d| d.item_type == ItemType::Weapon)?;
        Some(WeaponGear {
            weapon_type: data.weapon_type,
            atk: data.attack,
            matk: data.magic_attack,
            level: data.weapon_level,
            refine: item_refine(data, item, RefineGroup::Weapon, data.weapon_level, refine),
        })
    }

    /// Reads the worn-slot table. Each item counts once.
    pub(crate) fn gear_status(&self, refine: &RefineDb) -> GearStatus {
        let (right, left) = self.status_hands();
        let shield = left
            .and_then(|(_, item)| self.data(item.item_id))
            .is_some_and(|d| d.item_type == ItemType::Armor);
        let mut status = GearStatus {
            right: self.weapon_gear(right, refine),
            left: self.weapon_gear(left, refine),
            shield,
            range: BARE_HAND_RANGE,
            ..GearStatus::default()
        };
        for weapon in [right, left].into_iter().flatten() {
            if let Some(d) = self
                .data(weapon.1.item_id)
                .filter(|d| d.item_type == ItemType::Weapon)
            {
                status.range = status.range.max(d.range.into());
            }
        }
        // Slots are below 128 (`INVENTORY_SLOTS` is 100).
        let mut counted = 0u128;
        for (&position, slot) in EQI_ORDER.iter().zip(&self.worn.slots) {
            let Some(slot) = slot.filter(|s| *s < 128 && position != EQP_AMMO) else {
                continue;
            };
            if counted & (1 << slot) != 0 {
                continue;
            }
            counted |= 1 << slot;
            let Some((item, data)) = self
                .slots
                .get(usize::from(slot))
                .and_then(Option::as_ref)
                .and_then(|item| self.data(item.item_id).map(|d| (item, d)))
            else {
                continue;
            };
            status.hard_def = status.hard_def.saturating_add(data.defense);
            if data.item_type == ItemType::Armor {
                let bonus = item_refine(data, item, RefineGroup::Armor, data.armor_level, refine);
                status.armor_refine = status.armor_refine.saturating_add(bonus.bonus);
            }
        }
        status
    }
}

/// The refine bonus of a worn item: only `Refineable` items refined above 0 get one.
fn item_refine(
    data: &ItemData,
    item: &InventoryItem,
    group: RefineGroup,
    level: u8,
    refine: &RefineDb,
) -> RefineBonus {
    if !data.refineable || item.refine == 0 {
        return RefineBonus::default();
    }
    refine.bonus(group, level, item.refine)
}

pub(super) fn saturate(v: u32) -> u16 {
    u16::try_from(v).unwrap_or(u16::MAX)
}

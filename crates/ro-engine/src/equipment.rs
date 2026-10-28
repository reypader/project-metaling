//! Equipment rules, pure: where an item may go, whether a character may wear it, and the looks
//! and weapon type of a worn set (spec `specs/equipment/spec.md` §2.1, §2.2; catalog
//! `mechanics/equipment.md` §1, §3, §3a, §5).

use ro_core::{CharacterLooks, InventoryItem, ItemId, Sex, UnitLooks};
use ro_db::{
    AmmoType, Gender, ItemData, ItemDb, ItemType, RefineBonus, RefineDb, RefineGroup, WeaponType,
};

use crate::job::{class_can_use, dual_wields, is_genetic, is_mechanic, job_can_use};

/// `EQP_*` position bits (catalog §1).
pub(crate) const EQP_HEAD_LOW: u32 = 0x0001;
pub(crate) const EQP_HAND_R: u32 = 0x0002;
pub(crate) const EQP_GARMENT: u32 = 0x0004;
pub(crate) const EQP_ACC_R: u32 = 0x0008;
pub(crate) const EQP_ARMOR: u32 = 0x0010;
pub(crate) const EQP_HAND_L: u32 = 0x0020;
pub(crate) const EQP_SHOES: u32 = 0x0040;
pub(crate) const EQP_ACC_L: u32 = 0x0080;
pub(crate) const EQP_HEAD_TOP: u32 = 0x0100;
pub(crate) const EQP_HEAD_MID: u32 = 0x0200;
pub(crate) const EQP_COSTUME_HEAD_TOP: u32 = 0x0400;
pub(crate) const EQP_COSTUME_HEAD_MID: u32 = 0x0800;
pub(crate) const EQP_COSTUME_HEAD_LOW: u32 = 0x1000;
pub(crate) const EQP_COSTUME_GARMENT: u32 = 0x2000;
pub(crate) const EQP_AMMO: u32 = 0x8000;
pub(crate) const EQP_SHADOW_ARMOR: u32 = 0x1_0000;
pub(crate) const EQP_SHADOW_WEAPON: u32 = 0x2_0000;
pub(crate) const EQP_SHADOW_SHIELD: u32 = 0x4_0000;
pub(crate) const EQP_SHADOW_SHOES: u32 = 0x8_0000;
pub(crate) const EQP_SHADOW_ACC_R: u32 = 0x10_0000;
pub(crate) const EQP_SHADOW_ACC_L: u32 = 0x20_0000;
/// Both hands: a two-hander, or a dual-wield capable weapon's capability.
pub(crate) const EQP_ARMS: u32 = EQP_HAND_R | EQP_HAND_L;
pub(crate) const EQP_ACC: u32 = EQP_ACC_R | EQP_ACC_L;
const EQP_SHADOW_ARMS: u32 = EQP_SHADOW_WEAPON | EQP_SHADOW_SHIELD;
const EQP_SHADOW_ACC: u32 = EQP_SHADOW_ACC_R | EQP_SHADOW_ACC_L;
/// Positions with a sprite: heads, garment and costumes. `0x0999` carries `View` for these.
pub(crate) const EQP_VISIBLE: u32 = EQP_HEAD_LOW
    | EQP_HEAD_MID
    | EQP_HEAD_TOP
    | EQP_GARMENT
    | EQP_COSTUME_HEAD_TOP
    | EQP_COSTUME_HEAD_MID
    | EQP_COSTUME_HEAD_LOW
    | EQP_COSTUME_GARMENT;

/// Every position in rAthena `EQI` order, the order displacement, unequip-all and gear status
/// walk them in.
pub(crate) const EQI_ORDER: [u32; 21] = [
    EQP_ACC_L,
    EQP_ACC_R,
    EQP_SHOES,
    EQP_GARMENT,
    EQP_HEAD_LOW,
    EQP_HEAD_MID,
    EQP_HEAD_TOP,
    EQP_ARMOR,
    EQP_HAND_L,
    EQP_HAND_R,
    EQP_COSTUME_HEAD_TOP,
    EQP_COSTUME_HEAD_MID,
    EQP_COSTUME_HEAD_LOW,
    EQP_COSTUME_GARMENT,
    EQP_AMMO,
    EQP_SHADOW_ARMOR,
    EQP_SHADOW_WEAPON,
    EQP_SHADOW_SHIELD,
    EQP_SHADOW_SHOES,
    EQP_SHADOW_ACC_R,
    EQP_SHADOW_ACC_L,
];

/// `0x0999` results.
pub(crate) const EQUIP_FAIL_LEVEL: u8 = 1;
pub(crate) const EQUIP_FAIL: u8 = 2;
/// `0x0291` message ids for the ammo pairing (catalog `packets/equipment.md` "Messages").
pub(crate) const MSI_FAIL_NEED_EQUIPPED_BOW: u16 = 1691;
pub(crate) const MSI_WRONG_BULLET: u16 = 2493;
pub(crate) const MSI_USESKILL_FAIL_MADOGEAR: u16 = 1435;
pub(crate) const MSI_USESKILL_FAIL_CART: u16 = 1519;

/// Dual-wield weapon types (rAthena `W_DOUBLE_*`).
const W_DOUBLE_DD: u16 = 25;
const W_DOUBLE_SS: u16 = 26;
const W_DOUBLE_AA: u16 = 27;
const W_DOUBLE_DS: u16 = 28;
const W_DOUBLE_DA: u16 = 29;
const W_DOUBLE_SA: u16 = 30;

/// The inventory slot worn at each position (rAthena `equip_index`), indexed like `EQI_ORDER`.
/// Built from the items' `equip` masks at spawn; an equip fills it one position at a time.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct WornSlots {
    slots: [Option<u8>; EQI_ORDER.len()],
}

impl WornSlots {
    /// Every set `equip` bit points at its item's slot.
    pub(crate) fn from_slots(slots: &[Option<InventoryItem>]) -> Self {
        let mut worn = Self::default();
        for (slot, item) in slots.iter().enumerate() {
            if let Some(item) = item.as_ref().filter(|i| i.equip != 0) {
                worn.assign(item.equip, slot);
            }
        }
        worn
    }

    /// The slot at the first position (in `EQI` order) of `positions` that holds an item.
    pub(crate) fn at(&self, positions: u32) -> Option<usize> {
        EQI_ORDER
            .iter()
            .zip(&self.slots)
            .find_map(|(&p, &s)| s.filter(|_| p & positions != 0))
            .map(usize::from)
    }

    /// Points every position in `positions` at `slot`.
    pub(crate) fn assign(&mut self, positions: u32, slot: usize) {
        let Ok(slot) = u8::try_from(slot) else {
            return;
        };
        for (p, s) in EQI_ORDER.iter().zip(&mut self.slots) {
            if p & positions != 0 {
                *s = Some(slot);
            }
        }
    }

    /// Frees every position `slot` holds.
    pub(crate) fn clear_slot(&mut self, slot: usize) {
        for s in &mut self.slots {
            if s.is_some_and(|s| usize::from(s) == slot) {
                *s = None;
            }
        }
    }
}

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

    fn data(&self, id: ItemId) -> Option<&'a ItemData> {
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

/// One hand's weapon as the status reads it (spec `specs/stats/spec.md` §2.2).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct WeaponGear {
    pub(crate) weapon_type: WeaponType,
    /// Item ATK and MATK.
    pub(crate) atk: u32,
    pub(crate) matk: u32,
    /// Refine table entry (1/100 units); zero unrefined or not `Refineable`.
    pub(crate) refine: RefineBonus,
}

/// The gear part of the status: per-hand weapons, shield, hard DEF of every worn item once (ammo
/// skipped), the armor refine sum and the attack range. Rebuilt only on a gear change.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct GearStatus {
    /// The right-hand weapon. A two-hander that holds only the left hand so far counts here, as
    /// in rAthena (DEFERRED "Per-hand values").
    pub(crate) right: Option<WeaponGear>,
    /// A second, different weapon in the left hand (dual wield).
    pub(crate) left: Option<WeaponGear>,
    /// Armor in the left hand.
    pub(crate) shield: bool,
    /// Σ item `Defense`.
    pub(crate) hard_def: u32,
    /// Σ refine `Bonus` (1/100) over every worn, refined, `Refineable` armor.
    pub(crate) armor_refine: u32,
    pub(crate) range: u16,
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

fn saturate(v: u32) -> u16 {
    u16::try_from(v).unwrap_or(u16::MAX)
}

/// The character traits the equip checks read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Wearer {
    pub(crate) base_level: u16,
    pub(crate) sex: Sex,
    /// From [`crate::job::job_mapid`].
    pub(crate) mapid: u32,
}

/// Why an equip request is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Refusal {
    /// Nothing is sent back.
    NoAck,
    /// `0x0999` with this result.
    Result(u8),
    /// `0x0291` with this message, then `0x0999` result 2.
    Message { msg_id: u16 },
}

/// rAthena `pc_equippoint`: the item's `Locations`, widened to both hands for a dagger,
/// one-hand sword or one-hand axe on an Assassin or Kagerou/Oboro (shadow weapons likewise).
pub(crate) fn equip_point(item: &ItemData, mapid: u32) -> u32 {
    let dual = item.item_type == ItemType::Weapon
        && matches!(
            item.weapon_type,
            WeaponType::Dagger | WeaponType::OneHandSword | WeaponType::OneHandAxe
        )
        && dual_wields(mapid);
    match item.locations {
        EQP_HAND_R if dual => EQP_ARMS,
        EQP_SHADOW_WEAPON if dual => EQP_SHADOW_ARMS,
        locations => locations,
    }
}

/// The `pc_isequip` subset, in order (spec §2.2 step 7). `weapon_type` is the worn set's
/// [`Gear::weapon_type`], for the ammo pairing.
pub(crate) fn check_equip(
    item: &ItemData,
    entry: &InventoryItem,
    wearer: &Wearer,
    weapon_type: u16,
) -> Result<(), Refusal> {
    let fail = Err(Refusal::Result(EQUIP_FAIL));
    if (item.equip_level_min != 0 && wearer.base_level < item.equip_level_min)
        || (item.equip_level_max != 0 && wearer.base_level > item.equip_level_max)
    {
        return Err(Refusal::Result(EQUIP_FAIL_LEVEL));
    }
    let sex_ok = match item.gender {
        Gender::Both => true,
        Gender::Female => wearer.sex == Sex::Female,
        Gender::Male => wearer.sex == Sex::Male,
    };
    if !sex_ok || entry.attribute != 0 {
        return fail;
    }
    if let Some(ammo) = item.ammo_type.filter(|_| item.item_type == ItemType::Ammo) {
        ammo_check(ammo, weapon_type, wearer.mapid)?;
    }
    if !job_can_use(&item.jobs, wearer.mapid) || !class_can_use(item.classes, wearer.mapid) {
        return fail;
    }
    Ok(())
}

fn ammo_check(ammo: AmmoType, weapon_type: u16, mapid: u32) -> Result<(), Refusal> {
    let message = |msg_id| Err(Refusal::Message { msg_id });
    match ammo {
        AmmoType::Arrow if !ammo_fits_weapon(ammo, weapon_type) => {
            message(MSI_FAIL_NEED_EQUIPPED_BOW)
        }
        AmmoType::Bullet | AmmoType::Shell | AmmoType::Grenade
            if !ammo_fits_weapon(ammo, weapon_type) =>
        {
            message(MSI_WRONG_BULLET)
        }
        // Needs `AS_VENOMKNIFE`; no skills exist yet.
        AmmoType::Dagger => Err(Refusal::Result(EQUIP_FAIL)),
        // Needs a Mado Gear / a cart on those jobs; neither exists yet.
        AmmoType::Cannonball if is_mechanic(mapid) => message(MSI_USESKILL_FAIL_MADOGEAR),
        AmmoType::Cannonball if is_genetic(mapid) => message(MSI_USESKILL_FAIL_CART),
        _ => Ok(()),
    }
}

/// Whether worn ammo still pairs with the weapon type (the ammo cascade). Only arrows, bullets,
/// shells and grenades need a weapon.
pub(crate) fn ammo_fits_weapon(ammo: AmmoType, weapon_type: u16) -> bool {
    let is = |w: WeaponType| weapon_type == u16::from(w.id());
    match ammo {
        AmmoType::Arrow => is(WeaponType::Bow) || is(WeaponType::Musical) || is(WeaponType::Whip),
        AmmoType::Bullet | AmmoType::Shell => {
            is(WeaponType::Revolver)
                || is(WeaponType::Rifle)
                || is(WeaponType::Gatling)
                || is(WeaponType::Shotgun)
                || is(WeaponType::Grenade)
        }
        AmmoType::Grenade => is(WeaponType::Grenade),
        _ => true,
    }
}

/// Weapons whose removal takes the ammo off too.
pub(crate) fn is_ranged(weapon: WeaponType) -> bool {
    matches!(
        weapon,
        WeaponType::Bow
            | WeaponType::Musical
            | WeaponType::Whip
            | WeaponType::Revolver
            | WeaponType::Rifle
            | WeaponType::Gatling
            | WeaponType::Shotgun
            | WeaponType::Grenade
    )
}

/// The final mask (spec §2.2 step 9): one accessory slot, the dual-wield hand choice, or `pos`
/// unchanged. For a request naming both choices: the second (left) one only when the first is
/// taken and it is free, otherwise the first.
pub(crate) fn resolve_position(item: &ItemData, pos: u32, requested: u32, worn: &WornSlots) -> u32 {
    let pick = |first: u32, second: u32| {
        let asked = requested & (first | second);
        if asked != first | second {
            asked
        } else if worn.at(first).is_some() && worn.at(second).is_none() {
            second
        } else {
            first
        }
    };
    match pos {
        EQP_ACC => pick(EQP_ACC_R, EQP_ACC_L),
        EQP_SHADOW_ACC => pick(EQP_SHADOW_ACC_L, EQP_SHADOW_ACC_R),
        EQP_ARMS if item.locations == EQP_HAND_R => pick(EQP_HAND_R, EQP_HAND_L),
        EQP_SHADOW_ARMS if item.locations == EQP_SHADOW_WEAPON => {
            pick(EQP_SHADOW_WEAPON, EQP_SHADOW_SHIELD)
        }
        pos => pos,
    }
}

/// Spec §2.2 steps 3-9 for an item the character holds: the final mask, or why not. The dead
/// and slot checks (steps 1-2) are the caller's.
pub(crate) fn prepare_equip(
    entry: &InventoryItem,
    requested: u32,
    wearer: &Wearer,
    gear: &Gear<'_>,
) -> Result<u32, Refusal> {
    if !entry.identified {
        return Err(Refusal::Result(EQUIP_FAIL));
    }
    let item = gear.data(entry.item_id).ok_or(Refusal::NoAck)?;
    if !matches!(
        item.item_type,
        ItemType::Weapon | ItemType::Armor | ItemType::Ammo | ItemType::ShadowGear
    ) {
        return Err(Refusal::NoAck);
    }
    let requested = if item.item_type == ItemType::Ammo {
        EQP_AMMO
    } else {
        requested
    };
    let pos = equip_point(item, wearer.mapid);
    check_equip(item, entry, wearer, gear.weapon_type())?;
    if pos & requested == 0 || entry.equip != 0 {
        return Err(Refusal::Result(EQUIP_FAIL));
    }
    Ok(resolve_position(item, pos, requested, gear.worn()))
}

/// The `view` of a `0x0999` ack: the item's `View` on a visible position, else 0.
pub(crate) fn equip_view(item: &ItemData, mask: u32) -> u16 {
    if mask & EQP_VISIBLE != 0 {
        saturate(item.view)
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::job_mapid;
    use ro_core::{InventoryItem, ItemId, Sex};
    use ro_db::{
        AmmoType, Gender, ItemData, ItemDb, ItemType, RefineBonus, RefineDb, RefineGroup,
        WeaponType,
    };

    const KNIFE: u32 = 1201;
    const DAMASCUS: u32 = 1222;
    const GLADIUS: u32 = 1219;
    const TSURUGI: u32 = 1119;
    const MACE: u32 = 1501;
    const JUR: u32 = 1250;
    const BOW: u32 = 1701;
    const GUARD: u32 = 2101;
    const HAT: u32 = 2220;
    const GOGGLES: u32 = 2225;
    const BRAVERY_BAG: u32 = 2576;
    const COSTUME_HAT: u32 = 19500;
    const CLIP: u32 = 2607;
    const ARROW: u32 = 1750;
    const BULLET: u32 = 13200;
    const GRENADE: u32 = 13203;
    const VENOM_KNIFE: u32 = 13250;
    const SHURIKEN: u32 = 13251;
    const CANNONBALL: u32 = 18000;
    const JELLOPY: u32 = 909;
    const PET_ARMOR: u32 = 10001;
    const SWORD_ONLY: u32 = 1101;
    const KNIGHT_ONLY: u32 = 1102;
    const UPPER_ONLY: u32 = 2392;
    const LEVEL_20: u32 = 1714;
    const LEVEL_MAX_10: u32 = 2121;
    const FEMALE_ONLY: u32 = 1950;

    const ASSASSIN: u16 = 12;
    const KNIGHT: u16 = 7;
    const NOVICE: u16 = 0;

    fn weapon(id: u32, wt: WeaponType, locations: u32, attack: u32) -> ItemData {
        ItemData {
            id: ItemId(id),
            item_type: ItemType::Weapon,
            locations,
            weapon_type: wt,
            attack,
            range: 1,
            ..ItemData::default()
        }
    }

    fn armor(id: u32, locations: u32, view: u32) -> ItemData {
        ItemData {
            id: ItemId(id),
            item_type: ItemType::Armor,
            locations,
            view,
            ..ItemData::default()
        }
    }

    fn ammo(id: u32, ammo: AmmoType) -> ItemData {
        ItemData {
            id: ItemId(id),
            item_type: ItemType::Ammo,
            locations: EQP_AMMO,
            ammo_type: Some(ammo),
            ..ItemData::default()
        }
    }

    fn db() -> ItemDb {
        let mut sword_only = weapon(SWORD_ONLY, WeaponType::OneHandSword, EQP_HAND_R, 25);
        sword_only.jobs = [1 << 1, 0, 0];
        let mut knight_only = weapon(KNIGHT_ONLY, WeaponType::OneHandSword, EQP_HAND_R, 25);
        knight_only.jobs = [0, 1 << 1, 0];
        let mut upper_only = armor(UPPER_ONLY, EQP_ARMOR, 0);
        upper_only.classes = 0x02;
        let mut level_20 = weapon(LEVEL_20, WeaponType::Bow, EQP_ARMS, 50);
        level_20.equip_level_min = 20;
        let mut level_max_10 = armor(LEVEL_MAX_10, EQP_HAND_L, 1);
        level_max_10.equip_level_max = 10;
        let mut female_only = weapon(FEMALE_ONLY, WeaponType::Whip, EQP_HAND_R, 80);
        female_only.gender = Gender::Female;
        female_only.equip_level_min = 50;
        let items = vec![
            weapon(KNIFE, WeaponType::Dagger, EQP_HAND_R, 17),
            weapon(DAMASCUS, WeaponType::Dagger, EQP_HAND_R, 118),
            weapon(GLADIUS, WeaponType::Dagger, EQP_HAND_R, 105),
            weapon(TSURUGI, WeaponType::OneHandSword, EQP_HAND_R, 130),
            weapon(MACE, WeaponType::Mace, EQP_HAND_R, 37),
            weapon(JUR, WeaponType::Katar, EQP_ARMS, 125),
            weapon(BOW, WeaponType::Bow, EQP_ARMS, 15),
            armor(GUARD, EQP_HAND_L, 1),
            armor(HAT, EQP_HEAD_TOP, 16),
            armor(GOGGLES, EQP_HEAD_TOP | EQP_HEAD_MID, 1),
            armor(BRAVERY_BAG, EQP_GARMENT, 2),
            armor(COSTUME_HAT, EQP_COSTUME_HEAD_TOP, 900),
            armor(CLIP, EQP_ACC, 0),
            ammo(ARROW, AmmoType::Arrow),
            ammo(BULLET, AmmoType::Bullet),
            ammo(GRENADE, AmmoType::Grenade),
            ammo(VENOM_KNIFE, AmmoType::Dagger),
            ammo(SHURIKEN, AmmoType::Shuriken),
            ammo(CANNONBALL, AmmoType::Cannonball),
            ItemData {
                id: ItemId(JELLOPY),
                ..ItemData::default()
            },
            ItemData {
                id: ItemId(PET_ARMOR),
                item_type: ItemType::PetArmor,
                ..ItemData::default()
            },
            sword_only,
            knight_only,
            upper_only,
            level_20,
            level_max_10,
            female_only,
        ];
        let items = items
            .into_iter()
            .map(|mut d| {
                d.aegis_name = format!("Item{}", d.id.0);
                d.name.clone_from(&d.aegis_name);
                d
            })
            .collect();
        ItemDb::from_items("test", items).expect("test items load")
    }

    /// Inventory slots holding `items` (id, worn mask) in order.
    fn slots(items: &[(u32, u32)]) -> Vec<Option<InventoryItem>> {
        items
            .iter()
            .map(|&(id, equip)| {
                let mut item = InventoryItem::new(ItemId(id), 1);
                item.equip = equip;
                Some(item)
            })
            .collect()
    }

    fn wearer(job: u16) -> Wearer {
        Wearer {
            base_level: 99,
            sex: Sex::Male,
            mapid: job_mapid(job),
        }
    }

    fn data(db: &ItemDb, id: u32) -> &ItemData {
        db.get(ItemId(id)).expect("test item")
    }

    // ---- ENG-EQ-01: positions ----

    #[test]
    fn eng_eq_01_dual_wield_widening_for_assassins_only() {
        let db = db();
        let assassin = job_mapid(ASSASSIN);
        assert_eq!(equip_point(data(&db, DAMASCUS), assassin), EQP_ARMS);
        assert_eq!(equip_point(data(&db, TSURUGI), assassin), EQP_ARMS);
        assert_eq!(
            equip_point(data(&db, DAMASCUS), job_mapid(KNIGHT)),
            EQP_HAND_R
        );
        assert_eq!(equip_point(data(&db, MACE), assassin), EQP_HAND_R);
        assert_eq!(equip_point(data(&db, JUR), assassin), EQP_ARMS);
        assert_eq!(equip_point(data(&db, GUARD), assassin), EQP_HAND_L);
        assert_eq!(equip_point(data(&db, DAMASCUS), job_mapid(4059)), EQP_ARMS);
    }

    #[test]
    fn eng_eq_01_accessory_auto_slot() {
        let db = db();
        let clip = data(&db, CLIP);
        let none = WornSlots::from_slots(&[]);
        assert_eq!(resolve_position(clip, EQP_ACC, EQP_ACC, &none), EQP_ACC_R);
        let right = WornSlots::from_slots(&slots(&[(CLIP, EQP_ACC_R)]));
        assert_eq!(resolve_position(clip, EQP_ACC, EQP_ACC, &right), EQP_ACC_L);
        let both = WornSlots::from_slots(&slots(&[(CLIP, EQP_ACC_R), (CLIP, EQP_ACC_L)]));
        assert_eq!(resolve_position(clip, EQP_ACC, EQP_ACC, &both), EQP_ACC_R);
        assert_eq!(resolve_position(clip, EQP_ACC, EQP_ACC_L, &none), EQP_ACC_L);
    }

    /// The hand choice captured in `equip_sequence_3.txt`.
    #[test]
    fn eng_eq_01_dual_wield_hand_choice() {
        let db = db();
        let damascus = data(&db, DAMASCUS);
        let none = WornSlots::from_slots(&[]);
        assert_eq!(
            resolve_position(damascus, EQP_ARMS, EQP_ARMS, &none),
            EQP_HAND_R
        );
        let right = WornSlots::from_slots(&slots(&[(KNIFE, EQP_HAND_R)]));
        assert_eq!(
            resolve_position(damascus, EQP_ARMS, EQP_ARMS, &right),
            EQP_HAND_L
        );
        let both = WornSlots::from_slots(&slots(&[(KNIFE, EQP_HAND_R), (DAMASCUS, EQP_HAND_L)]));
        assert_eq!(
            resolve_position(data(&db, GLADIUS), EQP_ARMS, EQP_ARMS, &both),
            EQP_HAND_R
        );
        let left = WornSlots::from_slots(&slots(&[(DAMASCUS, EQP_HAND_L)]));
        assert_eq!(
            resolve_position(damascus, EQP_ARMS, EQP_ARMS, &left),
            EQP_HAND_R
        );
        // A one-hand request (a list from before `@job`) takes that hand.
        assert_eq!(
            resolve_position(damascus, EQP_ARMS, EQP_HAND_R, &right),
            EQP_HAND_R
        );
        // A two-hander keeps both hands.
        assert_eq!(
            resolve_position(data(&db, JUR), EQP_ARMS, EQP_ARMS, &right),
            EQP_ARMS
        );
    }

    #[test]
    fn eng_eq_01_weapon_type_resolution() {
        let db = db();
        let wt = |worn: &[(u32, u32)]| {
            let s = slots(worn);
            let w = WornSlots::from_slots(&s);
            Gear::new(&w, &s, &db).weapon_type()
        };
        assert_eq!(wt(&[]), 0);
        assert_eq!(wt(&[(TSURUGI, EQP_HAND_R), (DAMASCUS, EQP_HAND_L)]), 28);
        assert_eq!(wt(&[(DAMASCUS, EQP_HAND_R), (TSURUGI, EQP_HAND_L)]), 28);
        assert_eq!(wt(&[(KNIFE, EQP_HAND_R), (DAMASCUS, EQP_HAND_L)]), 25);
        assert_eq!(wt(&[(TSURUGI, EQP_HAND_R), (TSURUGI, EQP_HAND_L)]), 26);
        assert_eq!(wt(&[(JUR, EQP_ARMS)]), 16);
        assert_eq!(wt(&[(DAMASCUS, EQP_HAND_L)]), 1);
        assert_eq!(wt(&[(TSURUGI, EQP_HAND_R), (GUARD, EQP_HAND_L)]), 2);
        assert_eq!(wt(&[(MACE, EQP_HAND_R), (DAMASCUS, EQP_HAND_L)]), 8);
    }

    // ---- ENG-EQ-02: checks in order ----

    fn check(db: &ItemDb, id: u32, who: &Wearer, weapon_type: u16) -> Result<(), Refusal> {
        check_equip(
            data(db, id),
            &InventoryItem::new(ItemId(id), 1),
            who,
            weapon_type,
        )
    }

    #[test]
    fn eng_eq_02_level_gender_broken() {
        let db = db();
        let low = Wearer {
            base_level: 10,
            ..wearer(NOVICE)
        };
        assert_eq!(
            check(&db, LEVEL_20, &low, 0),
            Err(Refusal::Result(EQUIP_FAIL_LEVEL))
        );
        let high = Wearer {
            base_level: 11,
            ..wearer(NOVICE)
        };
        assert_eq!(
            check(&db, LEVEL_MAX_10, &high, 0),
            Err(Refusal::Result(EQUIP_FAIL_LEVEL))
        );
        assert_eq!(check(&db, LEVEL_MAX_10, &low, 0), Ok(()));
        // Female-only, level 50: the level fails first.
        assert_eq!(
            check(&db, FEMALE_ONLY, &low, 0),
            Err(Refusal::Result(EQUIP_FAIL_LEVEL))
        );
        assert_eq!(
            check(&db, FEMALE_ONLY, &wearer(NOVICE), 0),
            Err(Refusal::Result(EQUIP_FAIL))
        );
        let female = Wearer {
            sex: Sex::Female,
            ..wearer(NOVICE)
        };
        assert_eq!(check(&db, FEMALE_ONLY, &female, 0), Ok(()));
        let mut broken = InventoryItem::new(ItemId(KNIFE), 1);
        broken.attribute = 1;
        assert_eq!(
            check_equip(data(&db, KNIFE), &broken, &wearer(NOVICE), 0),
            Err(Refusal::Result(EQUIP_FAIL))
        );
    }

    #[test]
    fn eng_eq_02_job_and_class() {
        let db = db();
        assert_eq!(
            check(&db, SWORD_ONLY, &wearer(NOVICE), 0),
            Err(Refusal::Result(EQUIP_FAIL))
        );
        assert_eq!(check(&db, SWORD_ONLY, &wearer(1), 0), Ok(()));
        assert_eq!(check(&db, KNIGHT_ONLY, &wearer(KNIGHT), 0), Ok(()));
        assert_eq!(
            check(&db, KNIGHT_ONLY, &wearer(1), 0),
            Err(Refusal::Result(EQUIP_FAIL))
        );
        assert_eq!(
            check(&db, UPPER_ONLY, &wearer(NOVICE), 0),
            Err(Refusal::Result(EQUIP_FAIL))
        );
        assert_eq!(check(&db, UPPER_ONLY, &wearer(4001), 0), Ok(()));
    }

    #[test]
    fn eng_eq_02_ammo_pairing() {
        let db = db();
        let w = wearer(NOVICE);
        let bow = WeaponType::Bow.id().into();
        let dagger = WeaponType::Dagger.id().into();
        let msg = |msg_id| Err(Refusal::Message { msg_id });
        assert_eq!(
            check(&db, ARROW, &w, dagger),
            msg(MSI_FAIL_NEED_EQUIPPED_BOW)
        );
        assert_eq!(check(&db, ARROW, &w, 0), msg(MSI_FAIL_NEED_EQUIPPED_BOW));
        assert_eq!(check(&db, ARROW, &w, bow), Ok(()));
        assert_eq!(check(&db, ARROW, &w, WeaponType::Whip.id().into()), Ok(()));
        assert_eq!(check(&db, BULLET, &w, bow), msg(MSI_WRONG_BULLET));
        assert_eq!(
            check(&db, BULLET, &w, WeaponType::Rifle.id().into()),
            Ok(())
        );
        let revolver = WeaponType::Revolver.id().into();
        assert_eq!(check(&db, GRENADE, &w, revolver), msg(MSI_WRONG_BULLET));
        assert_eq!(
            check(&db, GRENADE, &w, WeaponType::Grenade.id().into()),
            Ok(())
        );
        assert_eq!(
            check(&db, VENOM_KNIFE, &w, dagger),
            Err(Refusal::Result(EQUIP_FAIL))
        );
        assert_eq!(check(&db, SHURIKEN, &w, 0), Ok(()));
        assert_eq!(
            check(&db, CANNONBALL, &wearer(4058), 0),
            msg(MSI_USESKILL_FAIL_MADOGEAR)
        );
        assert_eq!(
            check(&db, CANNONBALL, &wearer(4071), 0),
            msg(MSI_USESKILL_FAIL_CART)
        );
        assert_eq!(check(&db, CANNONBALL, &w, 0), Ok(()));
    }

    // ---- prepare_equip: spec §2.2 steps 3-9 ----

    fn prepare(
        db: &ItemDb,
        worn: &[(u32, u32)],
        slot: usize,
        requested: u32,
        who: &Wearer,
    ) -> Result<u32, Refusal> {
        let s = slots(worn);
        let w = WornSlots::from_slots(&s);
        let entry = s[slot].as_ref().expect("slot holds the item");
        prepare_equip(entry, requested, who, &Gear::new(&w, &s, db))
    }

    #[test]
    fn eng_eq_06_prepare_refusals() {
        let db = db();
        let w = wearer(ASSASSIN);
        let s = slots(&[(KNIFE, 0)]);
        let worn = WornSlots::from_slots(&s);
        let mut unidentified = InventoryItem::new(ItemId(KNIFE), 1);
        unidentified.identified = false;
        assert_eq!(
            prepare_equip(&unidentified, EQP_HAND_R, &w, &Gear::new(&worn, &s, &db)),
            Err(Refusal::Result(EQUIP_FAIL))
        );
        let unknown = InventoryItem::new(ItemId(4_000_000), 1);
        assert_eq!(
            prepare_equip(&unknown, EQP_HAND_R, &w, &Gear::new(&worn, &s, &db)),
            Err(Refusal::NoAck)
        );
        assert_eq!(
            prepare(&db, &[(JELLOPY, 0)], 0, EQP_HAND_R, &w),
            Err(Refusal::NoAck)
        );
        assert_eq!(
            prepare(&db, &[(PET_ARMOR, 0)], 0, EQP_HAND_R, &w),
            Err(Refusal::NoAck)
        );
        assert_eq!(
            prepare(&db, &[(KNIFE, EQP_HAND_R)], 0, EQP_HAND_R, &w),
            Err(Refusal::Result(EQUIP_FAIL)),
            "already worn"
        );
        assert_eq!(
            prepare(&db, &[(GUARD, 0)], 0, EQP_HAND_R, &w),
            Err(Refusal::Result(EQUIP_FAIL)),
            "wrong position"
        );
        assert_eq!(
            prepare(&db, &[(DAMASCUS, 0)], 0, EQP_HAND_L, &wearer(KNIGHT)),
            Err(Refusal::Result(EQUIP_FAIL)),
            "a knight can't take a dagger in the left hand"
        );
    }

    #[test]
    fn eng_eq_01_prepare_positions() {
        let db = db();
        let w = wearer(ASSASSIN);
        assert_eq!(
            prepare(&db, &[(KNIFE, EQP_HAND_R), (DAMASCUS, 0)], 1, EQP_ARMS, &w),
            Ok(EQP_HAND_L)
        );
        assert_eq!(prepare(&db, &[(JUR, 0)], 0, EQP_ARMS, &w), Ok(EQP_ARMS));
        // Ammo goes to the ammo position whatever the client asked for.
        let bow = (BOW, EQP_ARMS);
        assert_eq!(prepare(&db, &[bow, (ARROW, 0)], 1, 0, &w), Ok(EQP_AMMO));
        assert_eq!(
            prepare(&db, &[bow, (ARROW, 0)], 1, EQP_AMMO, &w),
            Ok(EQP_AMMO)
        );
        assert_eq!(
            prepare(&db, &[(KNIFE, EQP_HAND_R), (ARROW, 0)], 1, 0, &w),
            Err(Refusal::Message {
                msg_id: MSI_FAIL_NEED_EQUIPPED_BOW
            })
        );
    }

    // ---- looks ----

    fn looks(db: &ItemDb, worn: &[(u32, u32)]) -> (UnitLooks, CharacterLooks) {
        let s = slots(worn);
        let w = WornSlots::from_slots(&s);
        let gear = Gear::new(&w, &s, db);
        (gear.unit_looks(), gear.character_looks())
    }

    #[test]
    fn eng_eq_13_dual_wield_looks() {
        let db = db();
        let (unit, chr) = looks(&db, &[(KNIFE, EQP_HAND_R), (DAMASCUS, EQP_HAND_L)]);
        assert_eq!((unit.weapon, unit.shield), (KNIFE, DAMASCUS));
        assert_eq!((chr.weapon_type, chr.shield), (25, 0));
        let (unit, chr) = looks(&db, &[(TSURUGI, EQP_HAND_R), (DAMASCUS, EQP_HAND_L)]);
        assert_eq!((unit.weapon, unit.shield), (TSURUGI, DAMASCUS));
        assert_eq!((chr.weapon_type, chr.shield), (28, 0));
        let (unit, chr) = looks(&db, &[(TSURUGI, EQP_HAND_R), (GUARD, EQP_HAND_L)]);
        assert_eq!((unit.weapon, unit.shield), (TSURUGI, GUARD));
        assert_eq!(
            (chr.weapon_type, chr.shield),
            (2, 1),
            "shield = the Guard's View"
        );
        let (unit, chr) = looks(&db, &[(JUR, EQP_ARMS)]);
        assert_eq!((unit.weapon, unit.shield), (JUR, 0));
        assert_eq!((chr.weapon_type, chr.shield), (16, 0));
        // Mid-displacement: a two-hander holding only the left hand so far.
        let (unit, _) = looks(&db, &[(JUR, EQP_HAND_L)]);
        assert_eq!((unit.weapon, unit.shield), (0, JUR));
        let (unit, chr) = looks(&db, &[]);
        assert_eq!(
            (unit, chr),
            (UnitLooks::default(), CharacterLooks::default())
        );
    }

    #[test]
    fn eng_eq_05_head_and_robe_looks() {
        let db = db();
        let (unit, chr) = looks(
            &db,
            &[
                (GOGGLES, EQP_HEAD_TOP | EQP_HEAD_MID),
                (BRAVERY_BAG, EQP_GARMENT),
            ],
        );
        assert_eq!((unit.head_top, unit.head_mid, unit.robe), (1, 0, 2));
        assert_eq!((chr.head_top, chr.head_mid, chr.robe), (1, 0, 2));
        // Goggles holding only the mid slot so far (over a Hat): still no mid view.
        let (unit, _) = looks(&db, &[(GOGGLES, EQP_HEAD_MID)]);
        assert_eq!((unit.head_top, unit.head_mid), (0, 0));
        // A costume top overrides the normal top; without it the normal view shows.
        let (unit, chr) = looks(
            &db,
            &[(HAT, EQP_HEAD_TOP), (COSTUME_HAT, EQP_COSTUME_HEAD_TOP)],
        );
        assert_eq!((unit.head_top, chr.head_top), (900, 900));
        let (unit, _) = looks(&db, &[(HAT, EQP_HEAD_TOP)]);
        assert_eq!(unit.head_top, 16);
    }

    fn atk(status: &GearStatus) -> u32 {
        [status.right, status.left]
            .iter()
            .flatten()
            .map(|w| w.atk)
            .sum()
    }

    /// Spec §2.10, with the capture-3 values: both hands' ATK, a two-hander once (also while it
    /// holds only the left hand, where it is the right-hand weapon), DEF per item once, ammo
    /// skipped.
    #[test]
    fn gear_status_counts_each_item_once() {
        let db = db();
        let refine = RefineDb::default();
        let status = |worn: &[(u32, u32)]| {
            let s = slots(worn);
            let w = WornSlots::from_slots(&s);
            Gear::new(&w, &s, &db).gear_status(&refine)
        };
        assert_eq!(status(&[]).range, 1);
        assert_eq!(status(&[]).right, None, "bare hands");
        let dual = status(&[(KNIFE, EQP_HAND_R), (DAMASCUS, EQP_HAND_L)]);
        assert_eq!(atk(&dual), 135);
        assert_eq!(dual.left.map(|w| w.atk), Some(118));
        assert!(!dual.shield);
        assert_eq!(
            atk(&status(&[(TSURUGI, EQP_HAND_R), (DAMASCUS, EQP_HAND_L)])),
            248
        );
        let jur = status(&[(JUR, EQP_ARMS)]);
        assert_eq!((atk(&jur), jur.left), (125, None));
        let mid = status(&[(JUR, EQP_HAND_L)]);
        assert_eq!(
            (mid.right.map(|w| w.weapon_type), mid.left),
            (Some(WeaponType::Katar), None),
            "a two-hander on HAND_L only is the right-hand weapon"
        );
        assert!(status(&[(KNIFE, EQP_HAND_R), (GUARD, EQP_HAND_L)]).shield);
        let mut db2 = db.get(ItemId(GOGGLES)).cloned().expect("goggles");
        db2.defense = 5;
        let mut guard = db.get(ItemId(GUARD)).cloned().expect("guard");
        guard.defense = 20;
        let mut arrow = db.get(ItemId(ARROW)).cloned().expect("arrow");
        arrow.defense = 99;
        let mut bow = db.get(ItemId(BOW)).cloned().expect("bow");
        bow.range = 5;
        let db = ItemDb::from_items("def", vec![db2, guard, arrow, bow]).expect("loads");
        let s = slots(&[
            (GOGGLES, EQP_HEAD_TOP | EQP_HEAD_MID),
            (GUARD, EQP_HAND_L),
            (ARROW, EQP_AMMO),
        ]);
        let w = WornSlots::from_slots(&s);
        assert_eq!(Gear::new(&w, &s, &db).gear_status(&refine).hard_def, 25);
        let s = slots(&[(BOW, EQP_ARMS)]);
        let w = WornSlots::from_slots(&s);
        let bow_status = Gear::new(&w, &s, &db).gear_status(&refine);
        assert_eq!((atk(&bow_status), bow_status.range), (15, 5));
    }

    /// Refine bonuses: only `Refineable` items refined above 0, weapons by weapon level, armor
    /// summed over every worn armor (spec §2.2 "Refine").
    #[test]
    fn gear_status_reads_the_refine_table() {
        let mut knife = weapon(KNIFE, WeaponType::Dagger, EQP_HAND_R, 17);
        knife.refineable = true;
        knife.weapon_level = 1;
        let mut hat = armor(HAT, EQP_HEAD_TOP, 16);
        hat.refineable = true;
        hat.armor_level = 1;
        let mut guard = armor(GUARD, EQP_HAND_L, 1);
        guard.refineable = true;
        guard.armor_level = 1;
        let mut goggles = armor(GOGGLES, EQP_HEAD_MID, 1);
        goggles.armor_level = 1;
        let mut items = vec![knife, hat, guard, goggles];
        for d in &mut items {
            d.aegis_name = format!("Item{}", d.id.0);
        }
        let db = ItemDb::from_items("refine", items).expect("loads");
        // Cumulative per level, 20 levels.
        let steps = |per: u32| (1..=20).map(|i| i * per).collect::<Vec<u32>>();
        let row = |group, level, bonus: Vec<u32>, random_bonus: Vec<u32>| ro_db::RefineRow {
            group,
            level,
            bonus,
            random_bonus,
        };
        let refine = RefineDb::from_rows(
            "refine",
            vec![
                row(RefineGroup::Armor, 1, steps(100), vec![]),
                row(
                    RefineGroup::Weapon,
                    1,
                    steps(200),
                    steps(300).into_iter().map(|v| v - 300).collect(),
                ),
            ],
        )
        .expect("table");
        let mut s = slots(&[
            (KNIFE, EQP_HAND_R),
            (HAT, EQP_HEAD_TOP),
            (GUARD, EQP_HAND_L),
            (GOGGLES, EQP_HEAD_MID),
        ]);
        for (slot, level) in [(0, 2), (1, 2), (2, 1), (3, 2)] {
            if let Some(item) = s[slot].as_mut() {
                item.refine = level;
            }
        }
        let w = WornSlots::from_slots(&s);
        let status = Gear::new(&w, &s, &db).gear_status(&refine);
        assert_eq!(
            status.right.map(|w| w.refine),
            Some(RefineBonus {
                bonus: 400,
                random_bonus: 300
            })
        );
        assert_eq!(
            status.armor_refine, 300,
            "hat +2 and guard +1; goggles aren't refineable"
        );
    }

    #[test]
    fn equip_view_only_for_visible_positions() {
        let db = db();
        assert_eq!(equip_view(data(&db, HAT), EQP_HEAD_TOP), 16);
        assert_eq!(equip_view(data(&db, GUARD), EQP_HAND_L), 0);
        assert_eq!(equip_view(data(&db, BRAVERY_BAG), EQP_GARMENT), 2);
    }

    #[test]
    fn worn_slots_track_each_position() {
        let s = slots(&[(JUR, EQP_ARMS), (HAT, EQP_HEAD_TOP)]);
        let mut worn = WornSlots::from_slots(&s);
        assert_eq!(worn.at(EQP_HAND_R), Some(0));
        assert_eq!(worn.at(EQP_HAND_L), Some(0));
        assert_eq!(worn.at(EQP_HEAD_TOP), Some(1));
        assert_eq!(worn.at(EQP_SHOES), None);
        worn.clear_slot(0);
        assert_eq!(worn.at(EQP_ARMS), None);
        worn.assign(EQP_HAND_L, 0);
        assert_eq!((worn.at(EQP_HAND_L), worn.at(EQP_HAND_R)), (Some(0), None));
    }

    #[test]
    fn ammo_cascade_rules() {
        let bow = WeaponType::Bow.id().into();
        assert!(ammo_fits_weapon(AmmoType::Arrow, bow));
        assert!(!ammo_fits_weapon(
            AmmoType::Arrow,
            WeaponType::Dagger.id().into()
        ));
        assert!(!ammo_fits_weapon(AmmoType::Bullet, bow));
        assert!(ammo_fits_weapon(AmmoType::Shuriken, 0));
        assert!(is_ranged(WeaponType::Bow) && is_ranged(WeaponType::Whip));
        assert!(is_ranged(WeaponType::Grenade) && !is_ranged(WeaponType::Katar));
    }
}

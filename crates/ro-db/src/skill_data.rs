//! One skill as stored in `skill_db.yml`, with its per-level getters.

use serde::{Deserialize, Serialize};

use crate::yaml::{flow_list, is_zero};
use crate::{CastTimeFlags, DamageFlags, SkillElement, SkillFlags, SkillItemCost, WeaponType};

/// One skill as stored in `skill_db.yml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct SkillData {
    pub id: u16,
    /// Aegis name (`SM_BASH`), sent in the skill list.
    pub name: String,
    pub max_level: u8,
    /// rAthena's target type code, sent as-is: Passive 0, Attack 1, Ground 2, Self 4,
    /// Support 16, Trap 32. Stored as `TargetType` (a bare `Inf` key reads as infinity).
    #[serde(default, rename = "TargetType", skip_serializing_if = "is_zero")]
    pub inf: u8,
    /// Entry `i` is level `i + 1`; `max_level` entries, or none when 0 at every level.
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "flow_list"
    )]
    pub sp_cost: Vec<u16>,
    /// Entry `i` is level `i + 1`; `max_level` entries, or none when 0 at every level. A
    /// negative range means "weapon range"; the skill list sends its absolute value.
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "flow_list"
    )]
    pub range: Vec<i16>,
    #[serde(default, skip_serializing_if = "SkillFlags::is_empty")]
    pub flags: SkillFlags,
    // The per-level lists below follow the `sp_cost` rule: `max_level` entries, or none when
    // the value is 0 (the default) at every level.
    /// Cast time, ms.
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "flow_list"
    )]
    pub cast_time: Vec<u32>,
    /// Renewal fixed cast time, ms. −1 splits `cast_time` by `default_fixed_castrate`.
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "flow_list"
    )]
    pub fixed_cast_time: Vec<i32>,
    /// ms before the caster can act again; negative means `−delay + amotion`.
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "flow_list"
    )]
    pub after_cast_act_delay: Vec<i32>,
    /// ms before the caster can walk again.
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "flow_list"
    )]
    pub after_cast_walk_delay: Vec<u32>,
    /// Per-skill reuse delay, ms.
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "flow_list"
    )]
    pub cooldown: Vec<u32>,
    /// Hits shown; negative means the damage is the total, split over `|n|` hits.
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "flow_list"
    )]
    pub hit_count: Vec<i8>,
    /// Neutral at every level when empty.
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "flow_list"
    )]
    pub element: Vec<SkillElement>,
    #[serde(default, skip_serializing_if = "DamageFlags::is_empty")]
    pub damage_flags: DamageFlags,
    #[serde(default, skip_serializing_if = "CastTimeFlags::is_empty")]
    pub cast_time_flags: CastTimeFlags,
    /// `Requires.HpCost`.
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "flow_list"
    )]
    pub hp_cost: Vec<u32>,
    /// `Requires.ZenyCost`.
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "flow_list"
    )]
    pub zeny_cost: Vec<u32>,
    /// `Requires.Weapon`: the weapon types allowed, sorted; empty = any weapon.
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "flow_list"
    )]
    pub weapons: Vec<WeaponType>,
    /// `Requires.ItemCost`, in file order.
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "flow_list"
    )]
    pub item_cost: Vec<SkillItemCost>,
    /// `Requires.State` name (`Shield`, `Cart`, ...); `None` = no state needed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
}

impl SkillData {
    /// A passive skill (target type 0).
    pub fn is_passive(&self) -> bool {
        self.inf == 0
    }

    /// SP cost at `level` (clamped to `max_level`); 0 at level 0.
    pub fn sp(&self, level: u8) -> u16 {
        per_level(&self.sp_cost, level)
    }

    /// Range at `level` (clamped to `max_level`); 0 at level 0.
    pub fn range(&self, level: u8) -> i16 {
        per_level(&self.range, level)
    }

    // The accessors below also clamp to `max_level` and return the default at level 0.

    pub fn cast_time(&self, level: u8) -> u32 {
        per_level(&self.cast_time, level)
    }

    pub fn fixed_cast_time(&self, level: u8) -> i32 {
        per_level(&self.fixed_cast_time, level)
    }

    pub fn after_cast_act_delay(&self, level: u8) -> i32 {
        per_level(&self.after_cast_act_delay, level)
    }

    pub fn after_cast_walk_delay(&self, level: u8) -> u32 {
        per_level(&self.after_cast_walk_delay, level)
    }

    pub fn cooldown(&self, level: u8) -> u32 {
        per_level(&self.cooldown, level)
    }

    pub fn hit_count(&self, level: u8) -> i8 {
        per_level(&self.hit_count, level)
    }

    pub fn element(&self, level: u8) -> SkillElement {
        per_level(&self.element, level)
    }

    pub fn hp_cost(&self, level: u8) -> u32 {
        per_level(&self.hp_cost, level)
    }

    pub fn zeny_cost(&self, level: u8) -> u32 {
        per_level(&self.zeny_cost, level)
    }

    /// Whether `weapon` meets `Requires.Weapon`.
    pub fn allows_weapon(&self, weapon: WeaponType) -> bool {
        self.weapons.is_empty() || self.weapons.contains(&weapon)
    }

    /// The items a cast at `level` consumes. When any entry names a level, only the entries for
    /// that level count (rAthena's level-dependent item costs); otherwise every entry does.
    pub fn item_cost(&self, level: u8) -> impl Iterator<Item = &SkillItemCost> {
        let by_level = self.item_cost.iter().any(|cost| cost.level.is_some());
        self.item_cost
            .iter()
            .filter(move |cost| !by_level || cost.level == Some(level))
    }

    /// `(YAML key, length)` of every per-level list.
    pub(crate) fn list_lengths(&self) -> [(&'static str, usize); 11] {
        [
            ("SpCost", self.sp_cost.len()),
            ("Range", self.range.len()),
            ("CastTime", self.cast_time.len()),
            ("FixedCastTime", self.fixed_cast_time.len()),
            ("AfterCastActDelay", self.after_cast_act_delay.len()),
            ("AfterCastWalkDelay", self.after_cast_walk_delay.len()),
            ("Cooldown", self.cooldown.len()),
            ("HitCount", self.hit_count.len()),
            ("Element", self.element.len()),
            ("HpCost", self.hp_cost.len()),
            ("ZenyCost", self.zeny_cost.len()),
        ]
    }
}

fn per_level<T: Copy + Default>(values: &[T], level: u8) -> T {
    let Some(index) = usize::from(level).checked_sub(1) else {
        return T::default();
    };
    values
        .get(index)
        .or_else(|| values.last())
        .copied()
        .unwrap_or_default()
}

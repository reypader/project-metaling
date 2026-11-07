//! One mob database entry as stored in `mob_db.yml`, with rAthena's defaults.

use serde::{Deserialize, Serialize};

use crate::{Element, MobClass, MobDrop, MobId, MobMode, MobModes, MobRace, MobSize};

/// One mob database entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct MobData {
    pub id: MobId,
    pub aegis_name: String,
    pub name: String,
    pub level: u16,
    pub hp: u32,
    /// Full-share exp (rates 100: the db value).
    #[serde(default)]
    pub base_exp: u64,
    #[serde(default)]
    pub job_exp: u64,
    /// Base ATK (renewal `rhw.atk`); read by mob attacks later.
    #[serde(default)]
    pub attack: u16,
    /// Hard DEF.
    #[serde(default)]
    pub defense: i16,
    /// Renewal physical resistance.
    #[serde(default)]
    pub resistance: u16,
    #[serde(default = "one")]
    pub str: u16,
    #[serde(default = "one")]
    pub agi: u16,
    #[serde(default = "one")]
    pub vit: u16,
    #[serde(default = "one")]
    pub int: u16,
    #[serde(default = "one")]
    pub dex: u16,
    #[serde(default = "one")]
    pub luk: u16,
    /// Cells.
    #[serde(default)]
    pub attack_range: u16,
    #[serde(default)]
    pub size: MobSize,
    #[serde(default)]
    pub race: MobRace,
    #[serde(default)]
    pub element: Element,
    /// `1..=MAX_ELEMENT_LEVEL`.
    #[serde(default = "one_u8")]
    pub element_level: u8,
    /// ms per cell.
    pub walk_speed: u16,
    /// ms between swings (`adelay`), at least `attack_motion`.
    #[serde(default = "default_motion")]
    pub attack_delay: u16,
    /// `amotion`, at least 1.
    #[serde(default = "default_motion")]
    pub attack_motion: u16,
    /// `dmotion`: the flinch time sent as `dmg_speed`.
    #[serde(default)]
    pub damage_motion: u16,
    pub class: MobClass,
    /// From `Ai`, `Modes` and the `Class` additions.
    #[serde(default)]
    pub modes: MobModes,
    /// At most `MAX_MOB_DROPS`, in db order.
    pub drops: Vec<MobDrop>,
    /// At most `MAX_MVP_DROPS`. Unused until MVP rewards.
    pub mvp_drops: Vec<MobDrop>,
}

/// rAthena's `MAX_ASPD_NOPC`: the default (and lowest) `AttackDelay`, and the default
/// `AttackMotion`.
pub const DEFAULT_MOB_MOTION: u16 = 100;

fn one() -> u16 {
    1
}

fn one_u8() -> u8 {
    1
}

fn default_motion() -> u16 {
    DEFAULT_MOB_MOTION
}

/// rAthena's `DEFAULT_WALK_SPEED`.
pub const DEFAULT_MOB_WALK_SPEED: u16 = 150;

/// rAthena's struct defaults (id 0 is not a valid mob id; set it).
impl Default for MobData {
    fn default() -> Self {
        Self {
            id: MobId(0),
            aegis_name: String::new(),
            name: String::new(),
            level: 1,
            hp: 1,
            base_exp: 0,
            job_exp: 0,
            attack: 0,
            defense: 0,
            resistance: 0,
            str: 1,
            agi: 1,
            vit: 1,
            int: 1,
            dex: 1,
            luk: 1,
            attack_range: 0,
            size: MobSize::default(),
            race: MobRace::default(),
            element: Element::default(),
            element_level: 1,
            walk_speed: DEFAULT_MOB_WALK_SPEED,
            attack_delay: DEFAULT_MOB_MOTION,
            attack_motion: DEFAULT_MOB_MOTION,
            damage_motion: 0,
            class: MobClass::Normal,
            modes: MobModes::default(),
            drops: Vec::new(),
            mvp_drops: Vec::new(),
        }
    }
}

impl MobData {
    pub fn has_mode(&self, mode: MobMode) -> bool {
        self.modes.contains(mode)
    }

    pub fn is_mvp(&self) -> bool {
        self.has_mode(MobMode::Mvp)
    }

    /// rAthena `bosstype`: 2 for an MVP, 1 for `Class: Boss`, else 0.
    pub fn boss_type(&self) -> u8 {
        if self.is_mvp() {
            2
        } else if self.class == MobClass::Boss {
            1
        } else {
            0
        }
    }
}

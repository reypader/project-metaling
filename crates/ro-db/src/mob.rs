//! The mob registry (`MOB_DATA` YAML), produced offline by `tools/mob-import` from rAthena's mob
//! database. Drops are resolved to item ids. Combat fields (stats, motions, size, race, element,
//! modes) are for the damage formulas and mob status.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use ro_core::ItemId;
use serde::{Deserialize, Serialize};

use crate::yaml::Schema;
use crate::{DbLoadError, MobId};

/// `Header.Type` of the mob file.
pub const MOB_DATA_TYPE: &str = "MOB_DATA";
/// `Header.Version` this crate reads and writes.
pub const MOB_DATA_VERSION: u32 = 2;
/// The file of a mob data directory.
pub const MOB_DATA_FILE: &str = "mob_db.yml";
/// rAthena `MAX_MOB_DROP`.
pub const MAX_MOB_DROPS: usize = 10;
/// rAthena `MAX_MVP_DROP`.
pub const MAX_MVP_DROPS: usize = 3;
/// Drop rates are per 10000.
pub const MAX_DROP_RATE: u16 = 10_000;

const SCHEMA: Schema = Schema {
    db_type: MOB_DATA_TYPE,
    version: MOB_DATA_VERSION,
};

/// rAthena mob `Class`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MobClass {
    Normal,
    Boss,
    Guardian,
    Battlefield,
    Event,
}

/// rAthena mob `Size` (damage tables only; not what the client renders).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MobSize {
    #[default]
    Small,
    Medium,
    Large,
}

/// rAthena `Race`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MobRace {
    #[default]
    Formless,
    Undead,
    Brute,
    Plant,
    Insect,
    Fish,
    Demon,
    Demihuman,
    Angel,
    Dragon,
    PlayerHuman,
    PlayerDoram,
}

/// rAthena element (a mob's defense element; later also weapon and ammo elements).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Element {
    #[default]
    Neutral,
    Water,
    Earth,
    Fire,
    Wind,
    Poison,
    Holy,
    Dark,
    Ghost,
    Undead,
}

/// The highest `ElementLevel`.
pub const MAX_ELEMENT_LEVEL: u8 = 4;

/// One rAthena `MD_*` mode bit (catalog `db/mob_db.md` "Mode bits and Ai table").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MobMode {
    CanMove,
    Looter,
    Aggressive,
    Assist,
    CastSensorIdle,
    NoRandomWalk,
    NoCast,
    CanAttack,
    CastSensorChase,
    ChangeChase,
    Angry,
    ChangeTargetMelee,
    ChangeTargetChase,
    TargetWeak,
    RandomTarget,
    IgnoreMelee,
    IgnoreMagic,
    IgnoreRanged,
    Mvp,
    IgnoreMisc,
    KnockbackImmune,
    TeleportBlock,
    FixedItemDrop,
    Detector,
    StatusImmune,
    SkillImmune,
}

impl MobMode {
    /// Every mode, in bit order.
    pub const ALL: [MobMode; 26] = [
        Self::CanMove,
        Self::Looter,
        Self::Aggressive,
        Self::Assist,
        Self::CastSensorIdle,
        Self::NoRandomWalk,
        Self::NoCast,
        Self::CanAttack,
        Self::CastSensorChase,
        Self::ChangeChase,
        Self::Angry,
        Self::ChangeTargetMelee,
        Self::ChangeTargetChase,
        Self::TargetWeak,
        Self::RandomTarget,
        Self::IgnoreMelee,
        Self::IgnoreMagic,
        Self::IgnoreRanged,
        Self::Mvp,
        Self::IgnoreMisc,
        Self::KnockbackImmune,
        Self::TeleportBlock,
        Self::FixedItemDrop,
        Self::Detector,
        Self::StatusImmune,
        Self::SkillImmune,
    ];

    /// rAthena's `MD_*` value.
    pub const fn bit(self) -> u32 {
        match self {
            Self::CanMove => 0x1,
            Self::Looter => 0x2,
            Self::Aggressive => 0x4,
            Self::Assist => 0x8,
            Self::CastSensorIdle => 0x10,
            Self::NoRandomWalk => 0x20,
            Self::NoCast => 0x40,
            Self::CanAttack => 0x80,
            Self::CastSensorChase => 0x200,
            Self::ChangeChase => 0x400,
            Self::Angry => 0x800,
            Self::ChangeTargetMelee => 0x1000,
            Self::ChangeTargetChase => 0x2000,
            Self::TargetWeak => 0x4000,
            Self::RandomTarget => 0x8000,
            Self::IgnoreMelee => 0x10000,
            Self::IgnoreMagic => 0x20000,
            Self::IgnoreRanged => 0x40000,
            Self::Mvp => 0x80000,
            Self::IgnoreMisc => 0x100000,
            Self::KnockbackImmune => 0x200000,
            Self::TeleportBlock => 0x400000,
            Self::FixedItemDrop => 0x1000000,
            Self::Detector => 0x2000000,
            Self::StatusImmune => 0x4000000,
            Self::SkillImmune => 0x8000000,
        }
    }

    /// Case-insensitive lookup of a mode name as rAthena's `Modes:` keys spell it.
    pub fn from_name(name: &str) -> Option<MobMode> {
        Self::ALL
            .into_iter()
            .find(|mode| format!("{mode:?}").eq_ignore_ascii_case(name))
    }
}

/// A set of `MobMode`s, stored as rAthena's mode bits. YAML: a list of mode names.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct MobModes(u32);

impl MobModes {
    /// From raw bits; unknown bits are dropped.
    pub fn from_bits(bits: u32) -> Self {
        let known = MobMode::ALL.iter().fold(0, |acc, mode| acc | mode.bit());
        Self(bits & known)
    }

    pub fn bits(self) -> u32 {
        self.0
    }

    pub fn contains(self, mode: MobMode) -> bool {
        self.0 & mode.bit() != 0
    }

    pub fn set(&mut self, mode: MobMode, on: bool) {
        if on {
            self.0 |= mode.bit();
        } else {
            self.0 &= !mode.bit();
        }
    }

    /// The modes in bit order.
    pub fn iter(self) -> impl Iterator<Item = MobMode> {
        MobMode::ALL
            .into_iter()
            .filter(move |mode| self.contains(*mode))
    }
}

impl FromIterator<MobMode> for MobModes {
    fn from_iter<I: IntoIterator<Item = MobMode>>(iter: I) -> Self {
        let mut modes = Self::default();
        for mode in iter {
            modes.set(mode, true);
        }
        modes
    }
}

impl Serialize for MobModes {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let modes: Vec<MobMode> = self.iter().collect();
        serde_saphyr::FlowSeq(&modes).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for MobModes {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Vec::<MobMode>::deserialize(deserializer)?
            .into_iter()
            .collect())
    }
}

/// One drop entry: `item` drops when `roll % 10000 < rate`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct MobDrop {
    pub item: ItemId,
    /// `1..=10000`.
    pub rate: u16,
}

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

/// The global mob registry.
#[derive(Debug, Clone, Default)]
pub struct MobDb {
    /// Sorted by id.
    mobs: Vec<MobData>,
    by_id: HashMap<MobId, usize>,
}

impl MobDb {
    /// Parses one `MOB_DATA` file. `path` is only a label for errors.
    pub fn parse_file(path: &str, text: &str) -> Result<Vec<MobData>, DbLoadError> {
        SCHEMA.parse(path, text)
    }

    /// Serializes with the current header (no leading comments; the importer adds those).
    pub fn file_to_yaml(mobs: &[MobData]) -> Result<String, DbLoadError> {
        SCHEMA.to_yaml(mobs)
    }

    /// Builds the registry. Ids must be valid mob ids and unique, aegis names unique
    /// (case-insensitive), drop lists within their limits and rates in `1..=10000`.
    pub fn from_mobs(path: &str, mut mobs: Vec<MobData>) -> Result<Self, DbLoadError> {
        mobs.sort_by_key(|mob| mob.id);
        let mut by_id = HashMap::with_capacity(mobs.len());
        let mut aegis = HashSet::with_capacity(mobs.len());
        for (i, mob) in mobs.iter().enumerate() {
            if by_id.insert(mob.id, i).is_some() {
                return Err(DbLoadError::DuplicateId {
                    path: path.to_string(),
                    id: mob.id.0,
                });
            }
            check_mob(mob).map_err(|message| DbLoadError::Schema {
                path: path.to_string(),
                message: format!("mob {}: {message}", mob.id.0),
            })?;
            if !aegis.insert(mob.aegis_name.to_lowercase()) {
                return Err(DbLoadError::Schema {
                    path: path.to_string(),
                    message: format!("duplicate AegisName '{}'", mob.aegis_name),
                });
            }
        }
        Ok(Self { mobs, by_id })
    }

    /// Loads `MOB_DATA_FILE` from `dir`. A missing file is an error.
    pub fn load_dir(dir: &Path) -> Result<Self, DbLoadError> {
        let path = dir.join(MOB_DATA_FILE);
        let label = path.display().to_string();
        let Some(mobs) = SCHEMA.load(&path)? else {
            return Err(DbLoadError::Io {
                path: label,
                message: "file not found".into(),
            });
        };
        Self::from_mobs(&label, mobs)
    }

    pub fn get(&self, id: MobId) -> Option<&MobData> {
        self.by_id.get(&id).map(|&i| &self.mobs[i])
    }

    /// Mobs in id order.
    pub fn iter(&self) -> impl Iterator<Item = &MobData> {
        self.mobs.iter()
    }

    pub fn len(&self) -> usize {
        self.mobs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.mobs.is_empty()
    }
}

fn check_mob(mob: &MobData) -> Result<(), String> {
    if !mob.id.is_valid() {
        return Err("id outside the mob id ranges".into());
    }
    if mob.attack_motion == 0 {
        return Err("AttackMotion must be at least 1".into());
    }
    if mob.attack_delay < mob.attack_motion {
        return Err(format!(
            "AttackDelay {} is below AttackMotion {}",
            mob.attack_delay, mob.attack_motion
        ));
    }
    if !(1..=MAX_ELEMENT_LEVEL).contains(&mob.element_level) {
        return Err(format!(
            "ElementLevel {} outside 1..={MAX_ELEMENT_LEVEL}",
            mob.element_level
        ));
    }
    if mob.drops.len() > MAX_MOB_DROPS {
        return Err(format!("{} drops (max {MAX_MOB_DROPS})", mob.drops.len()));
    }
    if mob.mvp_drops.len() > MAX_MVP_DROPS {
        return Err(format!(
            "{} MVP drops (max {MAX_MVP_DROPS})",
            mob.mvp_drops.len()
        ));
    }
    if let Some(bad) = mob
        .drops
        .iter()
        .chain(&mob.mvp_drops)
        .find(|d| d.rate == 0 || d.rate > MAX_DROP_RATE)
    {
        return Err(format!("drop rate {} outside 1..=10000", bad.rate));
    }
    Ok(())
}

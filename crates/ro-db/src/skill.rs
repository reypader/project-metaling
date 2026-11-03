//! Skill registry (`SKILL_DATA`) and per-job skill trees (`SKILL_TREE_DATA`), from
//! `tools/skill-import` (specs `specs/skills/spec.md` §0-§1, `specs/effects/spec.md` §0.1, §1).
//! Per skill: the target type, the flags that decide learnability and range bonuses, and the
//! per-level casting data (costs, range, cast and delay times, element, requirements). The
//! trees come with rAthena's inheritance already resolved, so each job lists every skill it can
//! learn.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize, Serializer};

use crate::yaml::{is_false, Schema};
use crate::{DbLoadError, Element, WeaponType};

pub const SKILL_DATA_TYPE: &str = "SKILL_DATA";
/// 2 added the casting fields (`specs/effects`).
pub const SKILL_DATA_VERSION: u32 = 2;
pub const SKILL_DATA_FILE: &str = "skill_db.yml";
pub const SKILL_TREE_DATA_TYPE: &str = "SKILL_TREE_DATA";
pub const SKILL_TREE_DATA_VERSION: u32 = 1;
pub const SKILL_TREE_DATA_FILE: &str = "skill_tree.yml";
/// rAthena `MAX_SKILL_LEVEL`.
pub const MAX_SKILL_LEVEL: u8 = 13;

const SKILL_SCHEMA: Schema = Schema {
    db_type: SKILL_DATA_TYPE,
    version: SKILL_DATA_VERSION,
};
const TREE_SCHEMA: Schema = Schema {
    db_type: SKILL_TREE_DATA_TYPE,
    version: SKILL_TREE_DATA_VERSION,
};

/// The skill flags that matter before casting exists (catalog `db/skills.md` "Flags").
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct SkillFlags {
    /// A quest ("platinum") skill: not learnable with points, not counted, not reset.
    #[serde(default, skip_serializing_if = "is_false")]
    pub quest: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub wedding: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub spirit: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub npc: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub guild: bool,
    /// `range2` adds the player's Vulture's Eye level.
    #[serde(default, skip_serializing_if = "is_false")]
    pub alter_range_vulture: bool,
    /// `range2` adds the player's Snake Eye level.
    #[serde(default, skip_serializing_if = "is_false")]
    pub alter_range_snake_eye: bool,
    /// `range2` adds the player's Radius level.
    #[serde(default, skip_serializing_if = "is_false")]
    pub alter_range_radius: bool,
    /// `range2` adds a bonus from the player's Research Trap level.
    #[serde(default, skip_serializing_if = "is_false")]
    pub alter_range_research_trap: bool,
    /// `range2` is Shadow Jump's range at the player's Shadow Jump level.
    #[serde(default, skip_serializing_if = "is_false")]
    pub alter_range_shadow_jump: bool,
}

impl SkillFlags {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// rAthena `DamageFlags` (`NK_*`; catalog `db/skills.md` "DamageFlags").
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct DamageFlags {
    /// A support skill: no damage step.
    #[serde(default, skip_serializing_if = "is_false")]
    pub no_damage: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub splash: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub splash_split: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub ignore_atk_card: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub ignore_element: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub ignore_defense: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub ignore_flee: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub ignore_def_card: bool,
    /// The skill can crit.
    #[serde(default, skip_serializing_if = "is_false")]
    pub critical: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub ignore_long_card: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub simple_defense: bool,
}

impl DamageFlags {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// rAthena `CastTimeFlags` (`SKILL_CAST_*`; catalog `scripting/skills.md` §4.1).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct CastTimeFlags {
    /// DEX and INT don't shorten the cast.
    #[serde(default, skip_serializing_if = "is_false")]
    pub ignore_dex: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub ignore_status: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub ignore_item_bonus: bool,
}

impl CastTimeFlags {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// A skill's element: a plain element, or one picked when the skill hits.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SkillElement {
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
    /// The attacker's weapon element (Bash, Mammonite).
    Weapon,
    /// The attacker's endow status element.
    Endowed,
    /// A random element per hit.
    Random,
}

impl SkillElement {
    /// rAthena's `ELE_*` number: 0-9 for the plain elements, then `Weapon` 12, `Endowed` 13,
    /// `Random` 14.
    pub fn code(self) -> u8 {
        match self.element() {
            Some(element) => element as u8,
            None => match self {
                Self::Weapon => 12,
                Self::Endowed => 13,
                _ => 14,
            },
        }
    }

    /// The plain element; `None` for the ones picked at hit time.
    pub fn element(self) -> Option<Element> {
        Some(match self {
            Self::Neutral => Element::Neutral,
            Self::Water => Element::Water,
            Self::Earth => Element::Earth,
            Self::Fire => Element::Fire,
            Self::Wind => Element::Wind,
            Self::Poison => Element::Poison,
            Self::Holy => Element::Holy,
            Self::Dark => Element::Dark,
            Self::Ghost => Element::Ghost,
            Self::Undead => Element::Undead,
            Self::Weapon | Self::Endowed | Self::Random => return None,
        })
    }
}

/// An item a cast consumes (`Requires.ItemCost`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct SkillItemCost {
    /// Item aegis name (resolved against the item registry by the user).
    pub item: String,
    pub amount: u32,
    /// Only this level needs the item; `None` = every level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<u8>,
}

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
    fn list_lengths(&self) -> [(&'static str, usize); 11] {
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

/// The skill registry, sorted by id.
#[derive(Debug, Clone, Default)]
pub struct SkillDb {
    skills: Vec<SkillData>,
    by_name: HashMap<String, usize>,
}

impl SkillDb {
    /// Parses `skill_db.yml`. `path` is only a label for errors.
    pub fn parse(path: &str, text: &str) -> Result<Vec<SkillData>, DbLoadError> {
        SKILL_SCHEMA.parse(path, text)
    }

    /// Serializes skills with the current header (the importer adds the leading comments).
    pub fn to_yaml(skills: &[SkillData]) -> Result<String, DbLoadError> {
        SKILL_SCHEMA.to_yaml(skills)
    }

    /// Builds the registry. Ids and names (case-insensitive) are unique, `max_level` is in
    /// `1..=MAX_SKILL_LEVEL`, and the per-level lists are empty or have `max_level` entries.
    pub fn from_rows(path: &str, mut skills: Vec<SkillData>) -> Result<Self, DbLoadError> {
        skills.sort_by_key(|skill| skill.id);
        let mut by_name = HashMap::with_capacity(skills.len());
        for (index, skill) in skills.iter().enumerate() {
            let label = format!("skill {} ({})", skill.id, skill.name);
            if index > 0 && skills[index - 1].id == skill.id {
                return Err(DbLoadError::DuplicateId {
                    path: path.to_string(),
                    id: u32::from(skill.id),
                });
            }
            if by_name.insert(skill.name.to_lowercase(), index).is_some() {
                return Err(schema_error(path, format!("{label}: duplicate Name")));
            }
            if !(1..=MAX_SKILL_LEVEL).contains(&skill.max_level) {
                return Err(schema_error(
                    path,
                    format!("{label}: MaxLevel must be 1..={MAX_SKILL_LEVEL}"),
                ));
            }
            let levels = usize::from(skill.max_level);
            if let Some(key) = skill
                .list_lengths()
                .into_iter()
                .find_map(|(key, len)| (len != 0 && len != levels).then_some(key))
            {
                return Err(schema_error(
                    path,
                    format!("{label}: {key} needs {levels} entries or none"),
                ));
            }
            if let Some(cost) = skill
                .item_cost
                .iter()
                .find(|cost| cost.level.is_some_and(|l| l == 0 || l > skill.max_level))
            {
                return Err(schema_error(
                    path,
                    format!(
                        "{label}: ItemCost {} level out of 1..={}",
                        cost.item, skill.max_level
                    ),
                ));
            }
        }
        Ok(Self { skills, by_name })
    }

    /// Loads `SKILL_DATA_FILE` from `dir`. A missing file is an error.
    pub fn load_dir(dir: &Path) -> Result<Self, DbLoadError> {
        let path = dir.join(SKILL_DATA_FILE);
        let rows = load_required(&SKILL_SCHEMA, &path)?;
        Self::from_rows(&path.display().to_string(), rows)
    }

    pub fn get(&self, id: u16) -> Option<&SkillData> {
        self.skills
            .binary_search_by_key(&id, |skill| skill.id)
            .ok()
            .map(|index| &self.skills[index])
    }

    /// Case-insensitive aegis name lookup (`sm_bash`).
    pub fn by_name(&self, name: &str) -> Option<&SkillData> {
        self.by_name
            .get(&name.to_lowercase())
            .map(|&index| &self.skills[index])
    }

    /// Skills in id order.
    pub fn iter(&self) -> impl Iterator<Item = &SkillData> {
        self.skills.iter()
    }

    pub fn len(&self) -> usize {
        self.skills.len()
    }

    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }
}

/// A prerequisite: `skill` learned at `level` or higher.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct SkillRequirement {
    pub skill: u16,
    pub level: u8,
}

/// One skill in a job's tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct TreeEntry {
    pub skill: u16,
    /// The most this job can learn (at most the skill's `max_level`).
    pub max_level: u8,
    /// Base level needed to see the skill (0 = none).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub base_level: u16,
    /// Job level needed to see the skill (0 = none).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub job_level: u16,
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "flow_list"
    )]
    pub requires: Vec<SkillRequirement>,
}

/// One job's resolved tree as stored in `skill_tree.yml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct JobTree {
    /// rAthena `e_job` id.
    pub job: u16,
    /// The job's rAthena name, for readers of the file.
    pub name: String,
    /// Sorted by skill id.
    #[serde(default)]
    pub tree: Vec<TreeEntry>,
}

/// Every job's skill tree, keyed by job id.
#[derive(Debug, Clone, Default)]
pub struct SkillTreeDb {
    trees: HashMap<u16, Vec<TreeEntry>>,
}

impl SkillTreeDb {
    /// Parses `skill_tree.yml`. `path` is only a label for errors.
    pub fn parse(path: &str, text: &str) -> Result<Vec<JobTree>, DbLoadError> {
        TREE_SCHEMA.parse(path, text)
    }

    /// Serializes trees with the current header (the importer adds the leading comments).
    pub fn to_yaml(trees: &[JobTree]) -> Result<String, DbLoadError> {
        TREE_SCHEMA.to_yaml(trees)
    }

    /// Builds the registry. Job ids are unique; each tree is sorted by skill id with no
    /// repeats; every skill and requirement names a skill in `skills`, `MaxLevel` is in
    /// `1..=` the skill's, and a requirement level is in `1..=` the required skill's.
    pub fn from_rows(
        path: &str,
        rows: Vec<JobTree>,
        skills: &SkillDb,
    ) -> Result<Self, DbLoadError> {
        let mut trees = HashMap::with_capacity(rows.len());
        for row in rows {
            let label = format!("job {} ({})", row.job, row.name);
            for (index, entry) in row.tree.iter().enumerate() {
                if index > 0 && row.tree[index - 1].skill >= entry.skill {
                    return Err(schema_error(
                        path,
                        format!(
                            "{label}: skills not in ascending id order at {}",
                            entry.skill
                        ),
                    ));
                }
                let max = known(path, &label, skills, entry.skill)?.max_level;
                if !(1..=max).contains(&entry.max_level) {
                    return Err(schema_error(
                        path,
                        format!("{label}: skill {} MaxLevel must be 1..={max}", entry.skill),
                    ));
                }
                for need in &entry.requires {
                    let max = known(path, &label, skills, need.skill)?.max_level;
                    if !(1..=max).contains(&need.level) {
                        return Err(schema_error(
                            path,
                            format!(
                                "{label}: skill {} requires skill {} at level {} (1..={max})",
                                entry.skill, need.skill, need.level
                            ),
                        ));
                    }
                }
            }
            if trees.insert(row.job, row.tree).is_some() {
                return Err(DbLoadError::DuplicateId {
                    path: path.to_string(),
                    id: u32::from(row.job),
                });
            }
        }
        Ok(Self { trees })
    }

    /// Loads `SKILL_TREE_DATA_FILE` from `dir`, checked against `skills`. A missing file is an
    /// error.
    pub fn load_dir(dir: &Path, skills: &SkillDb) -> Result<Self, DbLoadError> {
        let path = dir.join(SKILL_TREE_DATA_FILE);
        let rows = load_required(&TREE_SCHEMA, &path)?;
        Self::from_rows(&path.display().to_string(), rows, skills)
    }

    /// `job`'s tree, sorted by skill id; empty for a job with no tree.
    pub fn tree(&self, job: u16) -> &[TreeEntry] {
        self.trees.get(&job).map_or(&[], Vec::as_slice)
    }

    /// `skill`'s entry in `job`'s tree.
    pub fn entry(&self, job: u16, skill: u16) -> Option<&TreeEntry> {
        let tree = self.tree(job);
        tree.binary_search_by_key(&skill, |entry| entry.skill)
            .ok()
            .map(|index| &tree[index])
    }

    /// The most `job` can learn of `skill` (rAthena `skill_tree_get_max`): the tree entry's
    /// max, else the skill's own, else 0 for an unknown skill.
    pub fn max_level(&self, job: u16, skill: u16, skills: &SkillDb) -> u8 {
        match self.entry(job, skill) {
            Some(entry) => entry.max_level,
            None => skills.get(skill).map_or(0, |data| data.max_level),
        }
    }

    /// Number of jobs with a tree.
    pub fn len(&self) -> usize {
        self.trees.len()
    }

    pub fn is_empty(&self) -> bool {
        self.trees.is_empty()
    }
}

fn known<'a>(
    path: &str,
    label: &str,
    skills: &'a SkillDb,
    id: u16,
) -> Result<&'a SkillData, DbLoadError> {
    skills
        .get(id)
        .ok_or_else(|| schema_error(path, format!("{label}: unknown skill {id}")))
}

fn load_required<T: serde::de::DeserializeOwned>(
    schema: &Schema,
    path: &Path,
) -> Result<Vec<T>, DbLoadError> {
    schema.load(path)?.ok_or_else(|| DbLoadError::Io {
        path: path.display().to_string(),
        message: "file not found".into(),
    })
}

fn schema_error(path: &str, message: String) -> DbLoadError {
    DbLoadError::Schema {
        path: path.to_string(),
        message,
    }
}

fn is_zero<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}

fn flow_list<S: Serializer, T: Serialize>(values: &[T], serializer: S) -> Result<S::Ok, S::Error> {
    serde_saphyr::FlowSeq(values).serialize(serializer)
}

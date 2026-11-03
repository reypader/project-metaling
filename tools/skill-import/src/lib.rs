//! Offline converter from rAthena's renewal skill files (`db/re/skill_db.yml`,
//! `db/re/skill_tree.yml`) to Metaling's `SKILL_DATA` and `SKILL_TREE_DATA` YAML
//! (`crates/ro-db/data/skill/`). Dev-only; nothing in `crates/` or `bins/` uses it.
//!
//! Written from `reference/catalog/db/skills.md` (clean-room), not ported from rAthena. The
//! importer does the work rAthena does at load time, so `ro-db` stays a plain reader:
//! - per-level lists are expanded ("List form expansion") and cut to the skill's max level;
//! - tree inheritance is resolved ("Load-time inheritance");
//! - job and skill names become ids.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::path::Path;

use anyhow::Context;
use ro_db::{
    CastTimeFlags, DamageFlags, JobTree, SkillData, SkillDb, SkillElement, SkillFlags,
    SkillItemCost, SkillRequirement, SkillTreeDb, TreeEntry, WeaponType, MAX_SKILL_LEVEL,
};
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};

pub const SKILL_DB_SOURCE: &str = "db/re/skill_db.yml";
pub const SKILL_TREE_SOURCE: &str = "db/re/skill_tree.yml";

/// Per-level slots rAthena keeps for every skill (`MAX_SKILL_LEVEL`).
const SLOTS: usize = MAX_SKILL_LEVEL as usize;

/// `skill_tree.yml` job names that differ from the `job_import::JOB_IDS` names.
const JOB_NAME_ALIASES: [(&str, &str); 1] = [("SUPERNOVICE", "SUPER_NOVICE")];

/// `(max base level, max job level)` per job id: tree `BaseLevel`/`JobLevel` are capped to
/// them.
pub type MaxLevels = HashMap<u16, (u16, u16)>;

#[derive(Debug, Default)]
pub struct Converted {
    /// Every skill a tree lists or requires, sorted by id.
    pub skills: Vec<SkillData>,
    /// Sorted by job id.
    pub trees: Vec<JobTree>,
    pub report: Report,
}

#[derive(Debug, Default)]
pub struct Report {
    pub skills_read: usize,
    pub jobs_read: usize,
    /// Tree job names with no id in `job_import::JOB_IDS` (mounted and dummy variants).
    pub skipped_jobs: BTreeSet<String>,
    pub warnings: Vec<String>,
}

/// Reads the skill files (and, for the level caps, the job files) under `rathena`.
pub fn import(rathena: &Path) -> anyhow::Result<Converted> {
    let read = |source: &str| {
        let path = rathena.join(source);
        std::fs::read_to_string(&path).with_context(|| path.display().to_string())
    };
    let jobs = job_import::import(rathena).context("job files (for the level caps)")?;
    let max_levels = jobs
        .jobs
        .iter()
        .map(|job| (job.id, (job.max_base_level, job.max_job_level)))
        .collect();
    convert(
        &read(SKILL_DB_SOURCE)?,
        &read(SKILL_TREE_SOURCE)?,
        &max_levels,
    )
}

/// Converts the `skill_db.yml` and `skill_tree.yml` texts.
pub fn convert(
    skill_db: &str,
    skill_tree: &str,
    max_levels: &MaxLevels,
) -> anyhow::Result<Converted> {
    let mut report = Report::default();
    let raw_skills: Vec<RawSkill> = parse_body(skill_db).context(SKILL_DB_SOURCE)?;
    report.skills_read = raw_skills.len();
    let mut skills: HashMap<String, SkillData> = HashMap::new();
    for raw in raw_skills {
        let Some(skill) = convert_skill(raw, &mut report.warnings) else {
            continue;
        };
        if let Some(old) = skills.insert(skill.name.to_uppercase(), skill) {
            report.warnings.push(format!(
                "skill {} ({}) defined twice; the later one wins",
                old.id, old.name
            ));
        }
    }

    let raw_jobs: Vec<RawJob> = parse_body(skill_tree).context(SKILL_TREE_SOURCE)?;
    report.jobs_read = raw_jobs.len();
    let jobs = merge_jobs(raw_jobs);
    let mut job_ids: HashMap<String, u16> = job_import::JOB_IDS
        .iter()
        .map(|(name, id)| (name.to_string(), *id))
        .collect();
    for (alias, name) in JOB_NAME_ALIASES {
        if let Some(&id) = job_ids.get(name) {
            job_ids.insert(alias.to_string(), id);
        }
    }
    let mut trees = Vec::new();
    for job in &jobs.list {
        let Some(&id) = job_ids.get(&job.name.to_uppercase()) else {
            report.skipped_jobs.insert(job.name.clone());
            continue;
        };
        let caps = max_levels.get(&id).copied();
        let tree = resolve_tree(job, &jobs, caps, &skills, &mut report.warnings);
        trees.push(JobTree {
            job: id,
            name: job.name.clone(),
            tree,
        });
    }
    trees.sort_by_key(|tree| tree.job);

    let used: BTreeSet<u16> = trees
        .iter()
        .flat_map(|tree| &tree.tree)
        .flat_map(|entry| {
            std::iter::once(entry.skill).chain(entry.requires.iter().map(|need| need.skill))
        })
        .collect();
    let mut skills: Vec<SkillData> = skills
        .into_values()
        .filter(|skill| used.contains(&skill.id))
        .collect();
    skills.sort_by_key(|skill| skill.id);
    Ok(Converted {
        skills,
        trees,
        report,
    })
}

/// `skill_db.yml`: license/attribution comment, then the YAML.
pub fn render_skills(skills: &[SkillData], rathena_commit: &str) -> anyhow::Result<String> {
    Ok(format!(
        "{}{}",
        attribution(rathena_commit),
        SkillDb::to_yaml(skills)?
    ))
}

/// `skill_tree.yml`: license/attribution comment, then the YAML.
pub fn render_trees(trees: &[JobTree], rathena_commit: &str) -> anyhow::Result<String> {
    Ok(format!(
        "{}{}",
        attribution(rathena_commit),
        SkillTreeDb::to_yaml(trees)?
    ))
}

/// Writes `skill_db.yml` and `skill_tree.yml` into `out_dir` (created if missing).
pub fn write_output(
    out_dir: &Path,
    converted: &Converted,
    rathena_commit: &str,
) -> anyhow::Result<()> {
    std::fs::create_dir_all(out_dir).with_context(|| out_dir.display().to_string())?;
    for (file, text) in [
        (
            ro_db::SKILL_DATA_FILE,
            render_skills(&converted.skills, rathena_commit)?,
        ),
        (
            ro_db::SKILL_TREE_DATA_FILE,
            render_trees(&converted.trees, rathena_commit)?,
        ),
    ] {
        let path = out_dir.join(file);
        std::fs::write(&path, text).with_context(|| path.display().to_string())?;
    }
    Ok(())
}

fn attribution(commit: &str) -> String {
    format!(
        "# SPDX-License-Identifier: GPL-3.0-only\n\
         # Derived from rAthena (https://github.com/rathena/rathena) commit {commit}, \
         {SKILL_DB_SOURCE}, {SKILL_TREE_SOURCE}.\n\
         # Modified: trimmed to the fields Metaling uses, per-level arrays expanded, tree \
         inheritance resolved, names resolved to ids, by tools/skill-import. Do not edit by \
         hand.\n\
         # See crates/ro-db/data/LICENSE and crates/ro-db/data/NOTICE.md.\n"
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RathenaFile<T> {
    #[serde(default = "Vec::new")]
    body: Vec<T>,
}

fn parse_body<T: serde::de::DeserializeOwned>(text: &str) -> anyhow::Result<Vec<T>> {
    let mut options = serde_saphyr::Options::default();
    // Trusted offline input: skill_db.yml is larger than the default budget.
    options.budget = None;
    let file: RathenaFile<T> = serde_saphyr::from_str_with_options(text, options)?;
    Ok(file.body)
}

// ---------- skill_db.yml ----------

/// One `skill_db.yml` entry. Keys this importer doesn't use are ignored. Shapes: catalog
/// `db/skills.md` "skill_db.yml cast/requirement fields".
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawSkill {
    id: u16,
    name: String,
    max_level: u8,
    target_type: Option<String>,
    range: Option<PerLevel<RawRange>>,
    requires: Option<RawRequires>,
    #[serde(default)]
    flags: BTreeMap<String, bool>,
    cast_time: Option<PerLevel<RawTime>>,
    fixed_cast_time: Option<PerLevel<RawTime>>,
    after_cast_act_delay: Option<PerLevel<RawTime>>,
    after_cast_walk_delay: Option<PerLevel<RawTime>>,
    cooldown: Option<PerLevel<RawTime>>,
    hit_count: Option<PerLevel<RawCount>>,
    element: Option<RawElement>,
    #[serde(default)]
    damage_flags: BTreeMap<String, bool>,
    #[serde(default)]
    cast_time_flags: BTreeMap<String, bool>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawRequires {
    sp_cost: Option<PerLevel<RawAmount>>,
    hp_cost: Option<PerLevel<RawAmount>>,
    zeny_cost: Option<PerLevel<RawAmount>>,
    weapon: Option<BTreeMap<String, bool>>,
    #[serde(default)]
    item_cost: Vec<RawItemCost>,
    state: Option<String>,
}

/// rAthena's per-level keys: one value for every level, or a `{Level, value}` list.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum PerLevel<T> {
    Scalar(i32),
    List(Vec<T>),
}

/// One `{Level, value}` entry of a per-level list.
trait Entry {
    fn pair(&self) -> (u16, i32);
}

macro_rules! entry {
    ($name:ident, $field:ident) => {
        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "PascalCase")]
        struct $name {
            level: u16,
            $field: i32,
        }

        impl Entry for $name {
            fn pair(&self) -> (u16, i32) {
                (self.level, self.$field)
            }
        }
    };
}

entry!(RawRange, size);
entry!(RawAmount, amount);
entry!(RawTime, time);
entry!(RawCount, count);

/// `Element`: one name for every level, or a `{Level, Element}` list.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawElement {
    Scalar(String),
    List(Vec<RawElementEntry>),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawElementEntry {
    level: u16,
    element: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawItemCost {
    item: String,
    amount: i32,
    level: Option<u16>,
}

fn convert_skill(raw: RawSkill, warnings: &mut Vec<String>) -> Option<SkillData> {
    let label = format!("skill {} ({})", raw.id, raw.name);
    if !(1..=MAX_SKILL_LEVEL).contains(&raw.max_level) {
        warnings.push(format!(
            "{label}: MaxLevel {} out of range, skipped",
            raw.max_level
        ));
        return None;
    }
    let inf = match raw.target_type.as_deref() {
        None | Some("Passive") => 0,
        Some("Attack") => 1,
        Some("Ground") => 2,
        Some("Self") => 4,
        Some("Support") => 16,
        Some("Trap") => 32,
        Some(other) => {
            warnings.push(format!(
                "{label}: unknown TargetType '{other}', using Passive"
            ));
            0
        }
    };
    let levels = usize::from(raw.max_level);
    let requires = raw.requires.unwrap_or_default();
    // Expands each per-level key; a bad level drops the whole skill, as in rAthena.
    let mut bad_level = None;
    let sp_cost = slots_of(requires.sp_cost.as_ref(), &mut bad_level);
    let hp_cost = slots_of(requires.hp_cost.as_ref(), &mut bad_level);
    let zeny_cost = slots_of(requires.zeny_cost.as_ref(), &mut bad_level);
    let range = slots_of(raw.range.as_ref(), &mut bad_level);
    let cast_time = slots_of(raw.cast_time.as_ref(), &mut bad_level);
    let fixed_cast_time = slots_of(raw.fixed_cast_time.as_ref(), &mut bad_level);
    let act_delay = slots_of(raw.after_cast_act_delay.as_ref(), &mut bad_level);
    let walk_delay = slots_of(raw.after_cast_walk_delay.as_ref(), &mut bad_level);
    let cooldown = slots_of(raw.cooldown.as_ref(), &mut bad_level);
    let hit_count = slots_of(raw.hit_count.as_ref(), &mut bad_level);
    let element = match raw.element.as_ref().map(|e| elements(e, &label, warnings)) {
        Some(Err(e)) => {
            bad_level.get_or_insert(e);
            None
        }
        Some(Ok(slots)) => Some(slots),
        None => None,
    };
    if let Some(e) = bad_level {
        warnings.push(format!("{label}: {e}, skipped"));
        return None;
    }
    let element = element
        .map(|slots| slots[..levels].to_vec())
        .filter(|list| list.iter().any(|e| *e != SkillElement::Neutral))
        .unwrap_or_default();
    let item_cost = requires
        .item_cost
        .into_iter()
        .filter_map(|cost| item_cost(cost, raw.max_level, &label, warnings))
        .collect();
    let state = requires
        .state
        .filter(|state| !state.eq_ignore_ascii_case("None"));
    let mut flags = SkillFlags::default();
    for (name, &on) in &raw.flags {
        let slot = match name.as_str() {
            "IsQuest" => &mut flags.quest,
            "IsWedding" => &mut flags.wedding,
            "IsSpirit" => &mut flags.spirit,
            "IsNpc" => &mut flags.npc,
            "IsGuild" => &mut flags.guild,
            "AlterRangeVulture" => &mut flags.alter_range_vulture,
            "AlterRangeSnakeEye" => &mut flags.alter_range_snake_eye,
            "AlterRangeRadius" => &mut flags.alter_range_radius,
            "AlterRangeResearchTrap" => &mut flags.alter_range_research_trap,
            "AlterRangeShadowJump" => &mut flags.alter_range_shadow_jump,
            _ => continue,
        };
        *slot = on;
    }
    let mut damage_flags = DamageFlags::default();
    for (name, &on) in &raw.damage_flags {
        let slot = match name.as_str() {
            "NoDamage" => &mut damage_flags.no_damage,
            "Splash" => &mut damage_flags.splash,
            "SplashSplit" => &mut damage_flags.splash_split,
            "IgnoreAtkCard" => &mut damage_flags.ignore_atk_card,
            "IgnoreElement" => &mut damage_flags.ignore_element,
            "IgnoreDefense" => &mut damage_flags.ignore_defense,
            "IgnoreFlee" => &mut damage_flags.ignore_flee,
            "IgnoreDefCard" => &mut damage_flags.ignore_def_card,
            "Critical" => &mut damage_flags.critical,
            "IgnoreLongCard" => &mut damage_flags.ignore_long_card,
            "SimpleDefense" => &mut damage_flags.simple_defense,
            other => {
                warnings.push(format!("{label}: unknown DamageFlags '{other}', ignored"));
                continue;
            }
        };
        *slot = on;
    }
    let mut cast_time_flags = CastTimeFlags::default();
    for (name, &on) in &raw.cast_time_flags {
        let slot = match name.as_str() {
            "IgnoreDex" => &mut cast_time_flags.ignore_dex,
            "IgnoreStatus" => &mut cast_time_flags.ignore_status,
            "IgnoreItemBonus" => &mut cast_time_flags.ignore_item_bonus,
            other => {
                warnings.push(format!("{label}: unknown CastTimeFlags '{other}', ignored"));
                continue;
            }
        };
        *slot = on;
    }
    let weapons = requires
        .weapon
        .map(|map| weapons(&map, &label, warnings))
        .unwrap_or_default();
    Some(SkillData {
        id: raw.id,
        name: raw.name,
        max_level: raw.max_level,
        inf,
        sp_cost: narrow(sp_cost, levels, &label, "SpCost", warnings),
        range: narrow(range, levels, &label, "Range", warnings),
        flags,
        cast_time: narrow(cast_time, levels, &label, "CastTime", warnings),
        fixed_cast_time: narrow(fixed_cast_time, levels, &label, "FixedCastTime", warnings),
        after_cast_act_delay: narrow(act_delay, levels, &label, "AfterCastActDelay", warnings),
        after_cast_walk_delay: narrow(walk_delay, levels, &label, "AfterCastWalkDelay", warnings),
        cooldown: narrow(cooldown, levels, &label, "Cooldown", warnings),
        hit_count: narrow(hit_count, levels, &label, "HitCount", warnings),
        element,
        damage_flags,
        cast_time_flags,
        hp_cost: narrow(hp_cost, levels, &label, "HpCost", warnings),
        zeny_cost: narrow(zeny_cost, levels, &label, "ZenyCost", warnings),
        weapons,
        item_cost,
        state,
    })
}

/// A per-level key's 13 slots; `None` when absent. A bad level is kept in `bad` (first wins).
fn slots_of<T: Entry>(
    key: Option<&PerLevel<T>>,
    bad: &mut Option<BadLevel>,
) -> Option<[i32; SLOTS]> {
    let result = match key? {
        PerLevel::Scalar(value) => Ok([*value; SLOTS]),
        PerLevel::List(list) => expand(&list.iter().map(Entry::pair).collect::<Vec<_>>()),
    };
    match result {
        Ok(slots) => Some(slots),
        Err(e) => {
            bad.get_or_insert(e);
            None
        }
    }
}

/// `Element` slots. A list sets the listed levels; unlisted levels below the highest listed one
/// stay Neutral (rAthena's zeroed slot), and the levels above it repeat it. Element codes aren't
/// numbers to extrapolate, so the `expand` series rule doesn't apply.
fn elements(
    raw: &RawElement,
    label: &str,
    warnings: &mut Vec<String>,
) -> Result<[SkillElement; SLOTS], BadLevel> {
    let mut named = |name: &str| {
        skill_element(name).unwrap_or_else(|| {
            warnings.push(format!("{label}: unknown Element '{name}', using Neutral"));
            SkillElement::Neutral
        })
    };
    let mut out = [SkillElement::Neutral; SLOTS];
    match raw {
        RawElement::Scalar(name) => out = [named(name); SLOTS],
        RawElement::List(list) => {
            let mut top = 0;
            for entry in list {
                let slot = usize::from(entry.level);
                if !(1..=SLOTS).contains(&slot) {
                    return Err(BadLevel(entry.level));
                }
                out[slot - 1] = named(&entry.element);
                top = top.max(slot);
            }
            if top > 0 {
                let last = out[top - 1];
                out[top..].fill(last);
            }
        }
    }
    Ok(out)
}

fn skill_element(name: &str) -> Option<SkillElement> {
    const NAMES: [(&str, SkillElement); 13] = [
        ("Neutral", SkillElement::Neutral),
        ("Water", SkillElement::Water),
        ("Earth", SkillElement::Earth),
        ("Fire", SkillElement::Fire),
        ("Wind", SkillElement::Wind),
        ("Poison", SkillElement::Poison),
        ("Holy", SkillElement::Holy),
        ("Dark", SkillElement::Dark),
        ("Ghost", SkillElement::Ghost),
        ("Undead", SkillElement::Undead),
        ("Weapon", SkillElement::Weapon),
        ("Endowed", SkillElement::Endowed),
        ("Random", SkillElement::Random),
    ];
    NAMES
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, e)| *e)
}

/// `Requires.Weapon` as a sorted set; empty = any weapon (`All: true`, or every key false).
fn weapons(
    map: &BTreeMap<String, bool>,
    label: &str,
    warnings: &mut Vec<String>,
) -> Vec<WeaponType> {
    const NAMES: [(&str, WeaponType); 24] = [
        ("Fist", WeaponType::Fist),
        ("Dagger", WeaponType::Dagger),
        ("1hSword", WeaponType::OneHandSword),
        ("2hSword", WeaponType::TwoHandSword),
        ("1hSpear", WeaponType::OneHandSpear),
        ("2hSpear", WeaponType::TwoHandSpear),
        ("1hAxe", WeaponType::OneHandAxe),
        ("2hAxe", WeaponType::TwoHandAxe),
        ("Mace", WeaponType::Mace),
        ("2hMace", WeaponType::TwoHandMace),
        ("Staff", WeaponType::Staff),
        ("Bow", WeaponType::Bow),
        ("Knuckle", WeaponType::Knuckle),
        ("Musical", WeaponType::Musical),
        ("Whip", WeaponType::Whip),
        ("Book", WeaponType::Book),
        ("Katar", WeaponType::Katar),
        ("Revolver", WeaponType::Revolver),
        ("Rifle", WeaponType::Rifle),
        ("Gatling", WeaponType::Gatling),
        ("Shotgun", WeaponType::Shotgun),
        ("Grenade", WeaponType::Grenade),
        ("Huuma", WeaponType::Huuma),
        ("2hStaff", WeaponType::TwoHandStaff),
    ];
    if map.get("All").copied().unwrap_or(false) {
        return Vec::new();
    }
    let mut set = BTreeSet::new();
    for (name, &on) in map {
        if name == "All" {
            continue;
        }
        let Some(&(_, weapon)) = NAMES.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)) else {
            warnings.push(format!("{label}: Weapon '{name}' not supported, ignored"));
            continue;
        };
        if on {
            set.insert(weapon);
        } else {
            set.remove(&weapon);
        }
    }
    set.into_iter().collect()
}

fn item_cost(
    raw: RawItemCost,
    max_level: u8,
    label: &str,
    warnings: &mut Vec<String>,
) -> Option<SkillItemCost> {
    let Ok(amount) = u32::try_from(raw.amount) else {
        warnings.push(format!(
            "{label}: ItemCost {} amount {}, dropped",
            raw.item, raw.amount
        ));
        return None;
    };
    let level = match raw.level {
        None => None,
        Some(level) => match u8::try_from(level) {
            Ok(level) if (1..=max_level).contains(&level) => Some(level),
            _ => {
                warnings.push(format!(
                    "{label}: ItemCost {} level {level}, dropped",
                    raw.item
                ));
                return None;
            }
        },
    };
    Some(SkillItemCost {
        item: raw.item,
        amount,
        level,
    })
}

/// The first `levels` slots in the target type; empty when they are all 0.
fn narrow<T: TryFrom<i32> + Copy + Default + PartialEq>(
    slots: Option<[i32; SLOTS]>,
    levels: usize,
    label: &str,
    key: &str,
    warnings: &mut Vec<String>,
) -> Vec<T> {
    let Some(slots) = slots else {
        return Vec::new();
    };
    let values: Vec<T> = slots[..levels]
        .iter()
        .map(|&value| {
            T::try_from(value).unwrap_or_else(|_| {
                warnings.push(format!("{label}: {key} {value} out of range, using 0"));
                T::default()
            })
        })
        .collect();
    if values.iter().all(|value| *value == T::default()) {
        Vec::new()
    } else {
        values
    }
}

/// Expansion error: a listed level outside `1..=MAX_SKILL_LEVEL` (rAthena drops the skill).
#[derive(Debug)]
pub struct BadLevel(pub u16);

impl fmt::Display for BadLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "per-level entry at Level {} (1..={MAX_SKILL_LEVEL})",
            self.0
        )
    }
}

/// rAthena's per-level list expansion (catalog `db/skills.md` "List form expansion"). Each
/// `(level, value)` sets that slot; `m` is the highest listed level. Slots above `m` continue
/// the list: the first period `step` in `1..=m/2` for which every `v[k] - v[k-step]` over
/// `step+1..=m` is the same difference `d` gives `v[k] = v[k-step] + d` (a value that drops
/// below 1 after a value of 0 or more becomes 1, and so does every later one); with no such
/// period the value at `m` repeats. Unlisted slots up to `m` stay 0.
pub fn expand(listed: &[(u16, i32)]) -> Result<[i32; SLOTS], BadLevel> {
    // 1-based: v[0] is unused.
    let mut v = [0i32; SLOTS + 1];
    let mut m = 0;
    for &(level, value) in listed {
        let slot = usize::from(level);
        if !(1..=SLOTS).contains(&slot) {
            return Err(BadLevel(level));
        }
        v[slot] = value;
        m = m.max(slot);
    }
    if m == 0 {
        return Ok([0; SLOTS]);
    }
    let period = (1..=m / 2).find_map(|step| {
        let d = v[m] - v[m - step];
        ((step + 1)..=m)
            .all(|k| v[k] - v[k - step] == d)
            .then_some((step, d))
    });
    match period {
        Some((mut step, mut d)) => {
            for k in (m + 1)..=SLOTS {
                v[k] = v[k - step] + d;
                if v[k] < 1 && v[k - 1] >= 0 {
                    v[k] = 1;
                    step = 1;
                    d = 0;
                }
            }
        }
        None => {
            for k in (m + 1)..=SLOTS {
                v[k] = v[m];
            }
        }
    }
    let mut out = [0; SLOTS];
    out.copy_from_slice(&v[1..]);
    Ok(out)
}

// ---------- skill_tree.yml ----------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawJob {
    job: String,
    #[serde(default)]
    inherit: Ordered,
    #[serde(default)]
    tree: Vec<RawTreeEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawTreeEntry {
    name: String,
    max_level: Option<u8>,
    exclude: Option<bool>,
    base_level: Option<u16>,
    job_level: Option<u16>,
    requires: Option<Vec<RawRequirement>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawRequirement {
    name: String,
    level: u8,
}

/// `Inherit`, in file order: a later parent wins a skill both parents define.
#[derive(Debug, Default)]
struct Ordered(Vec<(String, bool)>);

impl<'de> Deserialize<'de> for Ordered {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct OrderedVisitor;
        impl<'de> Visitor<'de> for OrderedVisitor {
            type Value = Ordered;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a map of job names to booleans")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Ordered, A::Error> {
                let mut out = Vec::new();
                while let Some((key, value)) = map.next_entry::<String, bool>()? {
                    out.push((key, value));
                }
                Ok(Ordered(out))
            }
        }
        deserializer.deserialize_map(OrderedVisitor)
    }
}

/// One job's own tree, with repeated entries for the same job merged.
#[derive(Debug, Default)]
struct JobAcc {
    name: String,
    /// Parent names (upper case) in first-listed order; a later listing changes the flag.
    inherit: Vec<(String, bool)>,
    own: Vec<EntryAcc>,
}

#[derive(Debug, Default, Clone)]
struct EntryAcc {
    name: String,
    max_level: Option<u8>,
    exclude: bool,
    base_level: u16,
    job_level: u16,
    /// Level 0 removes a requirement; kept here and dropped when the tree is resolved.
    requires: Vec<(String, u8)>,
}

struct Jobs {
    list: Vec<JobAcc>,
    by_name: HashMap<String, usize>,
}

/// Merges entries naming the same job, and entries naming the same skill within a job: a
/// later key overrides an earlier one, and `Requires` merges by skill name (rule 5).
fn merge_jobs(raw: Vec<RawJob>) -> Jobs {
    let mut jobs = Jobs {
        list: Vec::new(),
        by_name: HashMap::new(),
    };
    for entry in raw {
        let key = entry.job.to_uppercase();
        let index = *jobs.by_name.entry(key).or_insert_with(|| {
            jobs.list.push(JobAcc {
                name: entry.job.clone(),
                ..JobAcc::default()
            });
            jobs.list.len() - 1
        });
        let job = &mut jobs.list[index];
        for (parent, on) in entry.inherit.0 {
            let parent = parent.to_uppercase();
            match job.inherit.iter_mut().find(|(name, _)| *name == parent) {
                Some(slot) => slot.1 = on,
                None => job.inherit.push((parent, on)),
            }
        }
        for raw in entry.tree {
            let at = match job
                .own
                .iter()
                .position(|e| e.name.eq_ignore_ascii_case(&raw.name))
            {
                Some(at) => at,
                None => {
                    job.own.push(EntryAcc {
                        name: raw.name.clone(),
                        ..EntryAcc::default()
                    });
                    job.own.len() - 1
                }
            };
            let acc = &mut job.own[at];
            if raw.max_level.is_some() {
                acc.max_level = raw.max_level;
            }
            if let Some(exclude) = raw.exclude {
                acc.exclude = exclude;
            }
            if let Some(level) = raw.base_level {
                acc.base_level = level;
            }
            if let Some(level) = raw.job_level {
                acc.job_level = level;
            }
            for need in raw.requires.into_iter().flatten() {
                match acc
                    .requires
                    .iter_mut()
                    .find(|(name, _)| name.eq_ignore_ascii_case(&need.name))
                {
                    Some(slot) => slot.1 = need.level,
                    None => acc.requires.push((need.name, need.level)),
                }
            }
        }
    }
    jobs
}

/// The job's own entries, then each parent's own entries in `Inherit` order (rules 1-4): an
/// `Exclude`d parent entry and a skill the job defines itself are not inherited, a later
/// parent wins a skill two parents define, `BaseLevel`/`JobLevel` are capped to the job's max
/// levels, and `MaxLevel: 0` removes the skill. Names become ids; sorted by skill id.
fn resolve_tree(
    job: &JobAcc,
    jobs: &Jobs,
    caps: Option<(u16, u16)>,
    skills: &HashMap<String, SkillData>,
    warnings: &mut Vec<String>,
) -> Vec<TreeEntry> {
    let mut merged: Vec<&EntryAcc> = Vec::new();
    let own_names: BTreeSet<String> = job.own.iter().map(|e| e.name.to_uppercase()).collect();
    for entry in &job.own {
        merged.push(entry);
    }
    for (parent, on) in &job.inherit {
        if !on {
            continue;
        }
        let Some(&index) = jobs.by_name.get(parent) else {
            warnings.push(format!("job {}: unknown parent {parent}", job.name));
            continue;
        };
        for entry in &jobs.list[index].own {
            let key = entry.name.to_uppercase();
            if entry.exclude || own_names.contains(&key) {
                continue;
            }
            match merged
                .iter_mut()
                .find(|e| e.name.eq_ignore_ascii_case(&entry.name))
            {
                Some(slot) => *slot = entry,
                None => merged.push(entry),
            }
        }
    }

    let mut tree = Vec::with_capacity(merged.len());
    for entry in merged {
        let Some(max_level) = entry.max_level else {
            warnings.push(format!(
                "job {}: {} has no MaxLevel, skipped",
                job.name, entry.name
            ));
            continue;
        };
        if max_level == 0 {
            continue;
        }
        let Some(skill) = skills.get(&entry.name.to_uppercase()) else {
            warnings.push(format!(
                "job {}: unknown skill {}, skipped",
                job.name, entry.name
            ));
            continue;
        };
        let (base_cap, job_cap) = caps.unwrap_or((u16::MAX, u16::MAX));
        let mut requires = Vec::with_capacity(entry.requires.len());
        for (name, level) in &entry.requires {
            if *level == 0 {
                continue;
            }
            let Some(need) = skills.get(&name.to_uppercase()) else {
                warnings.push(format!(
                    "job {}: {} requires unknown skill {name}, requirement dropped",
                    job.name, entry.name
                ));
                continue;
            };
            if *level > need.max_level {
                warnings.push(format!(
                    "job {}: {} requires {name} {level}, above its max {}; capped",
                    job.name, entry.name, need.max_level
                ));
            }
            requires.push(SkillRequirement {
                skill: need.id,
                level: (*level).min(need.max_level),
            });
        }
        tree.push(TreeEntry {
            skill: skill.id,
            max_level: max_level.min(skill.max_level),
            base_level: entry.base_level.min(base_cap),
            job_level: entry.job_level.min(job_cap),
            requires,
        });
    }
    tree.sort_by_key(|entry| entry.skill);
    tree
}

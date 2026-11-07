use crate::{
    EffectSpec, HealShow, HookBackend, ItemUse, ItemUseDef, LevelValue, MissingHook, SkillDef,
    TargetRule, Who,
};

/// Every skill and item-use definition, with hook names bound to one backend's ids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Definitions {
    /// Sorted by id.
    skills: Vec<SkillDef>,
    item_heal: ItemUseDef,
    percent_heal: ItemUseDef,
}

// Skill ids of the built-in definitions.
const SM_BASH: u16 = 5;
const AL_HEAL: u16 = 28;
const MC_MAMMONITE: u16 = 42;
const NV_FIRSTAID: u16 = 142;

impl Definitions {
    /// The built-in definitions (spec §2.5, §2.7), bound to `hooks`:
    /// - First Aid: heal the caster 5 HP;
    /// - Bash: weapon damage at ratio `100 + 30·lv`, hit `+5·lv` %;
    /// - Mammonite: weapon damage at ratio `100 + 50·lv` (its zeny cost is data);
    /// - Heal: the `al_heal` hook; Undead mobs refused;
    /// - potions: the `item_heal` and `percent_heal` hooks.
    pub fn builtin(hooks: &dyn HookBackend) -> Result<Self, MissingHook> {
        let hook = |name: &'static str| hooks.hook_id(name).ok_or(MissingHook(name));
        let weapon = |ratio, hit_pct| EffectSpec::WeaponDamage {
            ratio,
            hit_pct,
            div: LevelValue::fixed(1),
        };
        let mut skills = vec![
            SkillDef {
                id: NV_FIRSTAID,
                effects: vec![EffectSpec::Heal {
                    to: Who::Caster,
                    hp: LevelValue::fixed(5),
                    sp: LevelValue::fixed(0),
                    show: HealShow::Skill,
                }],
                hook: None,
                target: TargetRule::default(),
            },
            SkillDef {
                id: SM_BASH,
                effects: vec![weapon(LevelValue::new(100, 30), LevelValue::new(0, 5))],
                hook: None,
                target: TargetRule::default(),
            },
            SkillDef {
                id: MC_MAMMONITE,
                effects: vec![weapon(LevelValue::new(100, 50), LevelValue::fixed(0))],
                hook: None,
                target: TargetRule::default(),
            },
            SkillDef {
                id: AL_HEAL,
                effects: Vec::new(),
                hook: Some(hook("al_heal")?),
                target: TargetRule {
                    reject_undead: true,
                },
            },
        ];
        skills.sort_by_key(|def| def.id);
        let item = |kind, name| -> Result<ItemUseDef, MissingHook> {
            Ok(ItemUseDef {
                kind,
                effects: Vec::new(),
                hook: Some(hook(name)?),
            })
        };
        Ok(Self {
            skills,
            item_heal: item(ItemUse::ItemHeal, "item_heal")?,
            percent_heal: item(ItemUse::PercentHeal, "percent_heal")?,
        })
    }

    /// The definition of skill `id`; `None` for a skill that can't be cast yet.
    pub fn skill(&self, id: u16) -> Option<&SkillDef> {
        self.skills
            .binary_search_by_key(&id, |def| def.id)
            .ok()
            .map(|index| &self.skills[index])
    }

    pub fn item_use(&self, kind: ItemUse) -> &ItemUseDef {
        match kind {
            ItemUse::ItemHeal => &self.item_heal,
            ItemUse::PercentHeal => &self.percent_heal,
        }
    }

    /// Skill definitions in id order.
    pub fn skills(&self) -> impl Iterator<Item = &SkillDef> {
        self.skills.iter()
    }
}

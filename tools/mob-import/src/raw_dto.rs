//! rAthena mob entries as written, and how a repeated id merges into the earlier entry.

use std::collections::BTreeMap;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct RathenaFile {
    #[serde(default)]
    pub(crate) body: Vec<RawMob>,
}

/// One rAthena entry. Every key is optional so a repeated id can merge only what it lists.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct RawMob {
    pub(crate) id: u32,
    pub(crate) aegis_name: Option<String>,
    pub(crate) name: Option<String>,
    pub(crate) level: Option<u16>,
    pub(crate) hp: Option<u32>,
    pub(crate) base_exp: Option<u64>,
    pub(crate) job_exp: Option<u64>,
    pub(crate) attack: Option<u16>,
    pub(crate) defense: Option<u16>,
    pub(crate) resistance: Option<u16>,
    pub(crate) str: Option<u16>,
    pub(crate) agi: Option<u16>,
    pub(crate) vit: Option<u16>,
    pub(crate) int: Option<u16>,
    pub(crate) dex: Option<u16>,
    pub(crate) luk: Option<u16>,
    pub(crate) attack_range: Option<u16>,
    pub(crate) size: Option<String>,
    pub(crate) race: Option<String>,
    pub(crate) element: Option<String>,
    pub(crate) element_level: Option<u16>,
    pub(crate) walk_speed: Option<u16>,
    pub(crate) attack_delay: Option<u16>,
    pub(crate) attack_motion: Option<u16>,
    pub(crate) damage_motion: Option<u16>,
    /// `02`, `ABR_PASSIVE`, ...: kept as written.
    pub(crate) ai: Option<String>,
    pub(crate) class: Option<String>,
    pub(crate) modes: Option<BTreeMap<String, bool>>,
    /// Every occurrence's list, in file order; slots are assigned at build.
    #[serde(default)]
    pub(crate) drops: Vec<RawDrop>,
    #[serde(default)]
    pub(crate) mvp_drops: Vec<RawDrop>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct RawDrop {
    pub(crate) item: String,
    pub(crate) rate: u32,
    pub(crate) index: Option<usize>,
}

pub(crate) fn take<T>(earlier: &mut Option<T>, later: Option<T>) {
    if later.is_some() {
        *earlier = later;
    }
}

impl RawMob {
    pub(crate) fn merge(&mut self, later: RawMob) {
        take(&mut self.aegis_name, later.aegis_name);
        take(&mut self.name, later.name);
        take(&mut self.level, later.level);
        take(&mut self.hp, later.hp);
        take(&mut self.base_exp, later.base_exp);
        take(&mut self.job_exp, later.job_exp);
        take(&mut self.attack, later.attack);
        take(&mut self.defense, later.defense);
        take(&mut self.resistance, later.resistance);
        take(&mut self.str, later.str);
        take(&mut self.agi, later.agi);
        take(&mut self.vit, later.vit);
        take(&mut self.int, later.int);
        take(&mut self.dex, later.dex);
        take(&mut self.luk, later.luk);
        take(&mut self.attack_range, later.attack_range);
        take(&mut self.size, later.size);
        take(&mut self.race, later.race);
        take(&mut self.element, later.element);
        take(&mut self.element_level, later.element_level);
        take(&mut self.walk_speed, later.walk_speed);
        take(&mut self.attack_delay, later.attack_delay);
        take(&mut self.attack_motion, later.attack_motion);
        take(&mut self.damage_motion, later.damage_motion);
        take(&mut self.class, later.class);
        // A new `Ai` replaces the whole mask, so earlier `Modes` toggles are gone with it.
        if later.ai.is_some() {
            self.ai = later.ai;
            self.modes = None;
        }
        if let Some(modes) = later.modes {
            self.modes.get_or_insert_with(BTreeMap::new).extend(modes);
        }
        self.drops.extend(later.drops);
        self.mvp_drops.extend(later.mvp_drops);
    }
}

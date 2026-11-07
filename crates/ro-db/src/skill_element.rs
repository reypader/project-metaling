use serde::{Deserialize, Serialize};

use crate::Element;

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

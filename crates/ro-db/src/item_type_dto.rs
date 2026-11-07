//! Item classification as stored in `item_*.yml`: type, weapon and ammo sub-types, gender, and
//! the effect of using a consumable.

use serde::{Deserialize, Serialize, Serializer};

/// `EQP_SHADOW_WEAPON`.
const EQP_SHADOW_WEAPON: u32 = 0x2_0000;

/// Item type after rAthena's load rules (`DelayConsume` is already `Usable`). `Etc` is rAthena's
/// default type.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ItemType {
    Healing,
    Usable,
    #[default]
    Etc,
    Armor,
    Weapon,
    Card,
    PetEgg,
    PetArmor,
    Ammo,
    ShadowGear,
    Cash,
}

impl ItemType {
    /// Weapons, armor, pet eggs, pet armor and shadow gear take one slot per item.
    pub fn is_stackable(self) -> bool {
        !matches!(
            self,
            Self::Weapon | Self::Armor | Self::PetEgg | Self::PetArmor | Self::ShadowGear
        )
    }

    /// The `type` byte of the item packets. Pet eggs show as armor; shadow gear shows as a
    /// weapon on the shadow weapon slot, else as armor.
    pub fn wire_type(self, locations: u32) -> u8 {
        match self {
            Self::Healing => 0,
            Self::Usable => 2,
            Self::Etc => 3,
            Self::Armor | Self::PetEgg => 4,
            Self::Weapon => 5,
            Self::Card => 6,
            Self::PetArmor => 8,
            Self::Ammo => 10,
            Self::ShadowGear if locations & EQP_SHADOW_WEAPON != 0 => 5,
            Self::ShadowGear => 4,
            Self::Cash => 18,
        }
    }
}

/// A weapon's `SubType` (rAthena `W_*`). Items that aren't weapons, and weapons without a
/// `SubType`, are `Fist`.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub enum WeaponType {
    #[default]
    Fist,
    Dagger,
    OneHandSword,
    TwoHandSword,
    OneHandSpear,
    TwoHandSpear,
    OneHandAxe,
    TwoHandAxe,
    Mace,
    TwoHandMace,
    Staff,
    Bow,
    Knuckle,
    Musical,
    Whip,
    Book,
    Katar,
    Revolver,
    Rifle,
    Gatling,
    Shotgun,
    Grenade,
    Huuma,
    TwoHandStaff,
}

impl WeaponType {
    /// The `W_*` number (char-select `weapon` field, ASPD tables).
    pub fn id(self) -> u8 {
        self as u8
    }
}

/// An ammo item's `SubType` (rAthena `AMMO_*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AmmoType {
    Arrow = 1,
    Dagger,
    Bullet,
    Shell,
    Grenade,
    Shuriken,
    Kunai,
    Cannonball,
    ThrowWeapon,
}

impl AmmoType {
    /// The `AMMO_*` number.
    pub fn id(self) -> u8 {
        self as u8
    }
}

/// Who may wear an item (rAthena `e_sex` values: female 0, male 1, both 2).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Gender {
    Female,
    Male,
    #[default]
    Both,
}

/// What using a consumable does: the whole rAthena `Script` when it is a single heal
/// statement, lifted by the importer (`specs/effects/spec.md` §0.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all_fields = "PascalCase", deny_unknown_fields)]
pub enum ItemUse {
    /// `itemheal`: HP and SP rolled in an inclusive `(min, max)` range each, then scaled by
    /// the user's VIT/INT and recovery skills.
    ItemHeal {
        #[serde(serialize_with = "flow_pair")]
        hp: (u32, u32),
        #[serde(serialize_with = "flow_pair")]
        sp: (u32, u32),
    },
    /// `percentheal`: percent of max HP and SP.
    PercentHeal { hp: u8, sp: u8 },
}

fn flow_pair<S: Serializer>(pair: &(u32, u32), serializer: S) -> Result<S::Ok, S::Error> {
    serde_saphyr::FlowSeq([pair.0, pair.1]).serialize(serializer)
}

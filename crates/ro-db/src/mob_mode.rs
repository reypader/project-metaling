use serde::{Deserialize, Serialize};

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

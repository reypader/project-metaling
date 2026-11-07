/// A skill's cast-time data at one level (`skill_db` `CastTime`, `FixedCastTime`,
/// `CastTimeFlags.IgnoreDex`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CastTimeData {
    /// ms.
    pub cast_time: u32,
    /// ms; −1 splits `cast_time` by `default_fixed_castrate`.
    pub fixed_cast_time: i32,
    pub ignore_dex: bool,
}

/// Who casts: a player's DEX and INT reduce the variable part; mobs and NPCs use the data
/// cast time as is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Caster {
    Player { dex: i32, int: i32 },
    MobOrNpc,
}

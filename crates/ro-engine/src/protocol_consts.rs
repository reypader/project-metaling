//! Wire constants shared by several engine features: rAthena enum values the client expects.

/// `SP_*` var ids (rAthena `enum _sp`), sent with `0x00b0`/`0x00b1`/`0x0acb`/`0x00be` and friends.
pub(crate) const SP_BASEEXP: u16 = 1;
pub(crate) const SP_JOBEXP: u16 = 2;
pub(crate) const SP_HP: u16 = 5;
pub(crate) const SP_MAXHP: u16 = 6;
pub(crate) const SP_SP: u16 = 7;
pub(crate) const SP_MAXSP: u16 = 8;
pub(crate) const SP_STATUSPOINT: u16 = 9;
pub(crate) const SP_BASELEVEL: u16 = 11;
pub(crate) const SP_SKILLPOINT: u16 = 12;
pub(crate) const SP_STR: u16 = 13;
pub(crate) const SP_LUK: u16 = 18;
/// `SP_ZENY`, sent with `0x00b1`.
pub(crate) const SP_ZENY: u16 = 20;
pub(crate) const SP_NEXTBASEEXP: u16 = 22;
pub(crate) const SP_NEXTJOBEXP: u16 = 23;
/// `ZC_PAR_CHANGE` var ids (rAthena `SP_WEIGHT`, `SP_MAXWEIGHT`), in 0.1 weight units.
pub(crate) const SP_WEIGHT: u16 = 24;
pub(crate) const SP_MAXWEIGHT: u16 = 25;
/// `SP_USTR`..`SP_ULUK`: the `0x00be` next-point costs.
pub(crate) const SP_USTR: u16 = 32;
pub(crate) const SP_ATK1: u16 = 41;
pub(crate) const SP_ATK2: u16 = 42;
pub(crate) const SP_MATK1: u16 = 43;
pub(crate) const SP_MATK2: u16 = 44;
pub(crate) const SP_DEF1: u16 = 45;
pub(crate) const SP_DEF2: u16 = 46;
pub(crate) const SP_MDEF1: u16 = 47;
pub(crate) const SP_MDEF2: u16 = 48;
pub(crate) const SP_HIT: u16 = 49;
pub(crate) const SP_FLEE1: u16 = 50;
pub(crate) const SP_FLEE2: u16 = 51;
pub(crate) const SP_CRITICAL: u16 = 52;
pub(crate) const SP_ASPD: u16 = 53;
pub(crate) const SP_JOBLEVEL: u16 = 55;
pub(crate) const SP_POW: u16 = 219;
pub(crate) const SP_CRT: u16 = 224;
/// `SP_PATK`..`SP_CRATE`: the trait-derived values, sent as `0x00b0` in this order.
pub(crate) const SP_PATK: u16 = 225;
pub(crate) const SP_TRAITPOINT: u16 = 231;
pub(crate) const SP_AP: u16 = 232;
pub(crate) const SP_MAXAP: u16 = 233;
pub(crate) const SP_UPOW: u16 = 247;

/// STR..LUK and POW..CRT.
pub(crate) const STAT_COUNT: u16 = 6;

/// `ZC_NOTIFY_VANISH` types.
pub(crate) const VANISH_OUT_OF_SIGHT: u8 = 0;
pub(crate) const VANISH_DIED: u8 = 1;
pub(crate) const VANISH_LOGGED_OUT: u8 = 2;

/// `CZ_REQUEST_ACT` attack types; every other action stops a walk first (rAthena
/// `clif_parse_ActionRequest_sub`).
pub(crate) const ACTION_ATTACK: u8 = 0;
pub(crate) const ACTION_ATTACK_REPEAT: u8 = 7;
/// `CZ_REQUEST_ACT` action types.
pub(crate) const ACTION_SIT: u8 = 2;
pub(crate) const ACTION_STAND: u8 = 3;

/// rAthena `dead_sit` values.
pub(crate) const STANDING: u8 = 0;
/// rAthena `dead_sit` value of a dead unit.
pub(crate) const DEAD: u8 = 1;
pub(crate) const SITTING: u8 = 2;

/// `LOOK_*` types of `0x01d7`.
pub(crate) const LOOK_BASE: u8 = 0;
pub(crate) const LOOK_HAIR: u8 = 1;
pub(crate) const LOOK_WEAPON: u8 = 2;
pub(crate) const LOOK_HEAD_BOTTOM: u8 = 3;
pub(crate) const LOOK_HEAD_TOP: u8 = 4;
pub(crate) const LOOK_HEAD_MID: u8 = 5;
pub(crate) const LOOK_CLOTHES_COLOR: u8 = 7;
pub(crate) const LOOK_SHOES: u8 = 9;
pub(crate) const LOOK_ROBE: u8 = 12;
pub(crate) const LOOK_BODY2: u8 = 13;

/// `0x019b` effects.
pub(crate) const EFFECT_BASE_LEVEL_UP: u32 = 0;
pub(crate) const EFFECT_JOB_LEVEL_UP: u32 = 1;

/// Wire type of an item missing from the registry (Etc).
pub(crate) const UNKNOWN_ITEM_TYPE: u8 = 3;

/// A free-cell search (rAthena `map_cell_free`) rejects cells closer than this to the edge.
pub(crate) const EDGE_GAP: i32 = 5;

/// `0x0110` result `USESKILL_FAIL_SKILLINTERVAL`: a request inside `canact`.
pub(crate) const FAIL_SKILL_INTERVAL: u8 = 4;
/// `display_skill_fail` bit that hides the skill-interval refusal.
pub(crate) const HIDE_SKILL_INTERVAL: u32 = 2;

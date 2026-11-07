//! Equip positions (`EQP_*`), the client's equip order, equip failure codes and messages.

/// `EQP_*` position bits (catalog §1).
pub(crate) const EQP_HEAD_LOW: u32 = 0x0001;

pub(crate) const EQP_HAND_R: u32 = 0x0002;

pub(crate) const EQP_GARMENT: u32 = 0x0004;

pub(crate) const EQP_ACC_R: u32 = 0x0008;

pub(crate) const EQP_ARMOR: u32 = 0x0010;

pub(crate) const EQP_HAND_L: u32 = 0x0020;

pub(crate) const EQP_SHOES: u32 = 0x0040;

pub(crate) const EQP_ACC_L: u32 = 0x0080;

pub(crate) const EQP_HEAD_TOP: u32 = 0x0100;

pub(crate) const EQP_HEAD_MID: u32 = 0x0200;

pub(crate) const EQP_COSTUME_HEAD_TOP: u32 = 0x0400;

pub(crate) const EQP_COSTUME_HEAD_MID: u32 = 0x0800;

pub(crate) const EQP_COSTUME_HEAD_LOW: u32 = 0x1000;

pub(crate) const EQP_COSTUME_GARMENT: u32 = 0x2000;

pub(crate) const EQP_AMMO: u32 = 0x8000;

pub(crate) const EQP_SHADOW_ARMOR: u32 = 0x1_0000;

pub(crate) const EQP_SHADOW_WEAPON: u32 = 0x2_0000;

pub(crate) const EQP_SHADOW_SHIELD: u32 = 0x4_0000;

pub(crate) const EQP_SHADOW_SHOES: u32 = 0x8_0000;

pub(crate) const EQP_SHADOW_ACC_R: u32 = 0x10_0000;

pub(crate) const EQP_SHADOW_ACC_L: u32 = 0x20_0000;

/// Both hands: a two-hander, or a dual-wield capable weapon's capability.
pub(crate) const EQP_ARMS: u32 = EQP_HAND_R | EQP_HAND_L;

pub(crate) const EQP_ACC: u32 = EQP_ACC_R | EQP_ACC_L;

pub(crate) const EQP_SHADOW_ARMS: u32 = EQP_SHADOW_WEAPON | EQP_SHADOW_SHIELD;

pub(crate) const EQP_SHADOW_ACC: u32 = EQP_SHADOW_ACC_R | EQP_SHADOW_ACC_L;

/// Positions with a sprite: heads, garment and costumes. `0x0999` carries `View` for these.
pub(crate) const EQP_VISIBLE: u32 = EQP_HEAD_LOW
    | EQP_HEAD_MID
    | EQP_HEAD_TOP
    | EQP_GARMENT
    | EQP_COSTUME_HEAD_TOP
    | EQP_COSTUME_HEAD_MID
    | EQP_COSTUME_HEAD_LOW
    | EQP_COSTUME_GARMENT;

/// Every position in rAthena `EQI` order, the order displacement, unequip-all and gear status
/// walk them in.
pub(crate) const EQI_ORDER: [u32; 21] = [
    EQP_ACC_L,
    EQP_ACC_R,
    EQP_SHOES,
    EQP_GARMENT,
    EQP_HEAD_LOW,
    EQP_HEAD_MID,
    EQP_HEAD_TOP,
    EQP_ARMOR,
    EQP_HAND_L,
    EQP_HAND_R,
    EQP_COSTUME_HEAD_TOP,
    EQP_COSTUME_HEAD_MID,
    EQP_COSTUME_HEAD_LOW,
    EQP_COSTUME_GARMENT,
    EQP_AMMO,
    EQP_SHADOW_ARMOR,
    EQP_SHADOW_WEAPON,
    EQP_SHADOW_SHIELD,
    EQP_SHADOW_SHOES,
    EQP_SHADOW_ACC_R,
    EQP_SHADOW_ACC_L,
];

/// `0x0999` results.
pub(crate) const EQUIP_FAIL_LEVEL: u8 = 1;

pub(crate) const EQUIP_FAIL: u8 = 2;

/// `0x0291` message ids for the ammo pairing (catalog `packets/equipment.md` "Messages").
pub(crate) const MSI_FAIL_NEED_EQUIPPED_BOW: u16 = 1691;

pub(crate) const MSI_WRONG_BULLET: u16 = 2493;

pub(crate) const MSI_USESKILL_FAIL_MADOGEAR: u16 = 1435;

pub(crate) const MSI_USESKILL_FAIL_CART: u16 = 1519;

/// Dual-wield weapon types (rAthena `W_DOUBLE_*`).
pub(crate) const W_DOUBLE_DD: u16 = 25;

pub(crate) const W_DOUBLE_SS: u16 = 26;

pub(crate) const W_DOUBLE_AA: u16 = 27;

pub(crate) const W_DOUBLE_DS: u16 = 28;

pub(crate) const W_DOUBLE_DA: u16 = 29;

pub(crate) const W_DOUBLE_SA: u16 = 30;

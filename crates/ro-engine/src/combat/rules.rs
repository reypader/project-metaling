//! Pure combat rules: ammo needs, distance, and the attacker view for the damage formula.

use ro_db::{AmmoType, WeaponType};
use ro_mechanics::{AttackerView, WeaponAttack};

use crate::stats::weapon_class;

/// The ammo a weapon type fires, if it needs any (catalog §5.3): bows take arrows, guns and
/// grenade launchers take bullets.
pub(crate) fn needed_ammo(weapon: WeaponType) -> Option<AmmoType> {
    match weapon {
        WeaponType::Bow => Some(AmmoType::Arrow),
        WeaponType::Revolver
        | WeaponType::Rifle
        | WeaponType::Gatling
        | WeaponType::Shotgun
        | WeaponType::Grenade => Some(AmmoType::Bullet),
        _ => None,
    }
}

pub(crate) fn chebyshev(a: (u16, u16), b: (u16, u16)) -> u16 {
    a.0.abs_diff(b.0).max(a.1.abs_diff(b.1))
}

/// What `calc_weapon_attack` reads of a player: its status, the right-hand weapon, and the
/// ammo ATK and range of a ranged swing (`equip_atk` 0 and `long_range` false for melee and for
/// weapon skills).
pub(crate) fn attacker_view(
    info: &crate::world::PlayerInfo,
    equip_atk: i32,
    long_range: bool,
) -> AttackerView {
    let status = &info.derived;
    AttackerView {
        weapon: info.gear.right.map(|w| WeaponAttack {
            atk: w.atk,
            refine_atk: w.refine.bonus / 100,
            level: w.level,
            overrefine: status.overrefine[0],
            dex_based: weapon_class(w.weapon_type).is_ranged(),
        }),
        equip_atk,
        long_range,
        ..AttackerView::from_status(status)
    }
}

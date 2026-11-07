use ro_core::{CharacterLooks, InventoryItem, ItemId, Sex, UnitLooks};
use ro_db::{
    AmmoType, Gender, ItemData, ItemDb, ItemType, RefineBonus, RefineDb, RefineGroup, WeaponType,
};

use super::rules::{equip_point, resolve_position};
use super::*;
use crate::job::job_mapid;

const KNIFE: u32 = 1201;
const DAMASCUS: u32 = 1222;
const GLADIUS: u32 = 1219;
const TSURUGI: u32 = 1119;
const MACE: u32 = 1501;
const JUR: u32 = 1250;
const BOW: u32 = 1701;
const GUARD: u32 = 2101;
const HAT: u32 = 2220;
const GOGGLES: u32 = 2225;
const BRAVERY_BAG: u32 = 2576;
const COSTUME_HAT: u32 = 19500;
const CLIP: u32 = 2607;
const ARROW: u32 = 1750;
const BULLET: u32 = 13200;
const GRENADE: u32 = 13203;
const VENOM_KNIFE: u32 = 13250;
const SHURIKEN: u32 = 13251;
const CANNONBALL: u32 = 18000;
const JELLOPY: u32 = 909;
const PET_ARMOR: u32 = 10001;
const SWORD_ONLY: u32 = 1101;
const KNIGHT_ONLY: u32 = 1102;
const UPPER_ONLY: u32 = 2392;
const LEVEL_20: u32 = 1714;
const LEVEL_MAX_10: u32 = 2121;
const FEMALE_ONLY: u32 = 1950;

const ASSASSIN: u16 = 12;
const KNIGHT: u16 = 7;
const NOVICE: u16 = 0;

fn weapon(id: u32, wt: WeaponType, locations: u32, attack: u32) -> ItemData {
    ItemData {
        id: ItemId(id),
        item_type: ItemType::Weapon,
        locations,
        weapon_type: wt,
        attack,
        range: 1,
        ..ItemData::default()
    }
}

fn armor(id: u32, locations: u32, view: u32) -> ItemData {
    ItemData {
        id: ItemId(id),
        item_type: ItemType::Armor,
        locations,
        view,
        ..ItemData::default()
    }
}

fn ammo(id: u32, ammo: AmmoType) -> ItemData {
    ItemData {
        id: ItemId(id),
        item_type: ItemType::Ammo,
        locations: EQP_AMMO,
        ammo_type: Some(ammo),
        ..ItemData::default()
    }
}

fn db() -> ItemDb {
    let mut sword_only = weapon(SWORD_ONLY, WeaponType::OneHandSword, EQP_HAND_R, 25);
    sword_only.jobs = [1 << 1, 0, 0];
    let mut knight_only = weapon(KNIGHT_ONLY, WeaponType::OneHandSword, EQP_HAND_R, 25);
    knight_only.jobs = [0, 1 << 1, 0];
    let mut upper_only = armor(UPPER_ONLY, EQP_ARMOR, 0);
    upper_only.classes = 0x02;
    let mut level_20 = weapon(LEVEL_20, WeaponType::Bow, EQP_ARMS, 50);
    level_20.equip_level_min = 20;
    let mut level_max_10 = armor(LEVEL_MAX_10, EQP_HAND_L, 1);
    level_max_10.equip_level_max = 10;
    let mut female_only = weapon(FEMALE_ONLY, WeaponType::Whip, EQP_HAND_R, 80);
    female_only.gender = Gender::Female;
    female_only.equip_level_min = 50;
    let items = vec![
        weapon(KNIFE, WeaponType::Dagger, EQP_HAND_R, 17),
        weapon(DAMASCUS, WeaponType::Dagger, EQP_HAND_R, 118),
        weapon(GLADIUS, WeaponType::Dagger, EQP_HAND_R, 105),
        weapon(TSURUGI, WeaponType::OneHandSword, EQP_HAND_R, 130),
        weapon(MACE, WeaponType::Mace, EQP_HAND_R, 37),
        weapon(JUR, WeaponType::Katar, EQP_ARMS, 125),
        weapon(BOW, WeaponType::Bow, EQP_ARMS, 15),
        armor(GUARD, EQP_HAND_L, 1),
        armor(HAT, EQP_HEAD_TOP, 16),
        armor(GOGGLES, EQP_HEAD_TOP | EQP_HEAD_MID, 1),
        armor(BRAVERY_BAG, EQP_GARMENT, 2),
        armor(COSTUME_HAT, EQP_COSTUME_HEAD_TOP, 900),
        armor(CLIP, EQP_ACC, 0),
        ammo(ARROW, AmmoType::Arrow),
        ammo(BULLET, AmmoType::Bullet),
        ammo(GRENADE, AmmoType::Grenade),
        ammo(VENOM_KNIFE, AmmoType::Dagger),
        ammo(SHURIKEN, AmmoType::Shuriken),
        ammo(CANNONBALL, AmmoType::Cannonball),
        ItemData {
            id: ItemId(JELLOPY),
            ..ItemData::default()
        },
        ItemData {
            id: ItemId(PET_ARMOR),
            item_type: ItemType::PetArmor,
            ..ItemData::default()
        },
        sword_only,
        knight_only,
        upper_only,
        level_20,
        level_max_10,
        female_only,
    ];
    let items = items
        .into_iter()
        .map(|mut d| {
            d.aegis_name = format!("Item{}", d.id.0);
            d.name.clone_from(&d.aegis_name);
            d
        })
        .collect();
    ItemDb::from_items("test", items).expect("test items load")
}

/// Inventory slots holding `items` (id, worn mask) in order.
fn slots(items: &[(u32, u32)]) -> Vec<Option<InventoryItem>> {
    items
        .iter()
        .map(|&(id, equip)| {
            let mut item = InventoryItem::new(ItemId(id), 1);
            item.equip = equip;
            Some(item)
        })
        .collect()
}

fn wearer(job: u16) -> Wearer {
    Wearer {
        base_level: 99,
        sex: Sex::Male,
        mapid: job_mapid(job),
    }
}

fn data(db: &ItemDb, id: u32) -> &ItemData {
    db.get(ItemId(id)).expect("test item")
}

// ---- ENG-EQ-01: positions ----

#[test]
fn eng_eq_01_dual_wield_widening_for_assassins_only() {
    let db = db();
    let assassin = job_mapid(ASSASSIN);
    assert_eq!(equip_point(data(&db, DAMASCUS), assassin), EQP_ARMS);
    assert_eq!(equip_point(data(&db, TSURUGI), assassin), EQP_ARMS);
    assert_eq!(
        equip_point(data(&db, DAMASCUS), job_mapid(KNIGHT)),
        EQP_HAND_R
    );
    assert_eq!(equip_point(data(&db, MACE), assassin), EQP_HAND_R);
    assert_eq!(equip_point(data(&db, JUR), assassin), EQP_ARMS);
    assert_eq!(equip_point(data(&db, GUARD), assassin), EQP_HAND_L);
    assert_eq!(equip_point(data(&db, DAMASCUS), job_mapid(4059)), EQP_ARMS);
}

#[test]
fn eng_eq_01_accessory_auto_slot() {
    let db = db();
    let clip = data(&db, CLIP);
    let none = WornSlots::from_slots(&[]);
    assert_eq!(resolve_position(clip, EQP_ACC, EQP_ACC, &none), EQP_ACC_R);
    let right = WornSlots::from_slots(&slots(&[(CLIP, EQP_ACC_R)]));
    assert_eq!(resolve_position(clip, EQP_ACC, EQP_ACC, &right), EQP_ACC_L);
    let both = WornSlots::from_slots(&slots(&[(CLIP, EQP_ACC_R), (CLIP, EQP_ACC_L)]));
    assert_eq!(resolve_position(clip, EQP_ACC, EQP_ACC, &both), EQP_ACC_R);
    assert_eq!(resolve_position(clip, EQP_ACC, EQP_ACC_L, &none), EQP_ACC_L);
}

/// The hand choice captured in `equip_sequence_3.txt`.
#[test]
fn eng_eq_01_dual_wield_hand_choice() {
    let db = db();
    let damascus = data(&db, DAMASCUS);
    let none = WornSlots::from_slots(&[]);
    assert_eq!(
        resolve_position(damascus, EQP_ARMS, EQP_ARMS, &none),
        EQP_HAND_R
    );
    let right = WornSlots::from_slots(&slots(&[(KNIFE, EQP_HAND_R)]));
    assert_eq!(
        resolve_position(damascus, EQP_ARMS, EQP_ARMS, &right),
        EQP_HAND_L
    );
    let both = WornSlots::from_slots(&slots(&[(KNIFE, EQP_HAND_R), (DAMASCUS, EQP_HAND_L)]));
    assert_eq!(
        resolve_position(data(&db, GLADIUS), EQP_ARMS, EQP_ARMS, &both),
        EQP_HAND_R
    );
    let left = WornSlots::from_slots(&slots(&[(DAMASCUS, EQP_HAND_L)]));
    assert_eq!(
        resolve_position(damascus, EQP_ARMS, EQP_ARMS, &left),
        EQP_HAND_R
    );
    // A one-hand request (a list from before `@job`) takes that hand.
    assert_eq!(
        resolve_position(damascus, EQP_ARMS, EQP_HAND_R, &right),
        EQP_HAND_R
    );
    // A two-hander keeps both hands.
    assert_eq!(
        resolve_position(data(&db, JUR), EQP_ARMS, EQP_ARMS, &right),
        EQP_ARMS
    );
}

#[test]
fn eng_eq_01_weapon_type_resolution() {
    let db = db();
    let wt = |worn: &[(u32, u32)]| {
        let s = slots(worn);
        let w = WornSlots::from_slots(&s);
        Gear::new(&w, &s, &db).weapon_type()
    };
    assert_eq!(wt(&[]), 0);
    assert_eq!(wt(&[(TSURUGI, EQP_HAND_R), (DAMASCUS, EQP_HAND_L)]), 28);
    assert_eq!(wt(&[(DAMASCUS, EQP_HAND_R), (TSURUGI, EQP_HAND_L)]), 28);
    assert_eq!(wt(&[(KNIFE, EQP_HAND_R), (DAMASCUS, EQP_HAND_L)]), 25);
    assert_eq!(wt(&[(TSURUGI, EQP_HAND_R), (TSURUGI, EQP_HAND_L)]), 26);
    assert_eq!(wt(&[(JUR, EQP_ARMS)]), 16);
    assert_eq!(wt(&[(DAMASCUS, EQP_HAND_L)]), 1);
    assert_eq!(wt(&[(TSURUGI, EQP_HAND_R), (GUARD, EQP_HAND_L)]), 2);
    assert_eq!(wt(&[(MACE, EQP_HAND_R), (DAMASCUS, EQP_HAND_L)]), 8);
}

// ---- ENG-EQ-02: checks in order ----

fn check(db: &ItemDb, id: u32, who: &Wearer, weapon_type: u16) -> Result<(), Refusal> {
    check_equip(
        data(db, id),
        &InventoryItem::new(ItemId(id), 1),
        who,
        weapon_type,
    )
}

#[test]
fn eng_eq_02_level_gender_broken() {
    let db = db();
    let low = Wearer {
        base_level: 10,
        ..wearer(NOVICE)
    };
    assert_eq!(
        check(&db, LEVEL_20, &low, 0),
        Err(Refusal::Result(EQUIP_FAIL_LEVEL))
    );
    let high = Wearer {
        base_level: 11,
        ..wearer(NOVICE)
    };
    assert_eq!(
        check(&db, LEVEL_MAX_10, &high, 0),
        Err(Refusal::Result(EQUIP_FAIL_LEVEL))
    );
    assert_eq!(check(&db, LEVEL_MAX_10, &low, 0), Ok(()));
    // Female-only, level 50: the level fails first.
    assert_eq!(
        check(&db, FEMALE_ONLY, &low, 0),
        Err(Refusal::Result(EQUIP_FAIL_LEVEL))
    );
    assert_eq!(
        check(&db, FEMALE_ONLY, &wearer(NOVICE), 0),
        Err(Refusal::Result(EQUIP_FAIL))
    );
    let female = Wearer {
        sex: Sex::Female,
        ..wearer(NOVICE)
    };
    assert_eq!(check(&db, FEMALE_ONLY, &female, 0), Ok(()));
    let mut broken = InventoryItem::new(ItemId(KNIFE), 1);
    broken.attribute = 1;
    assert_eq!(
        check_equip(data(&db, KNIFE), &broken, &wearer(NOVICE), 0),
        Err(Refusal::Result(EQUIP_FAIL))
    );
}

#[test]
fn eng_eq_02_job_and_class() {
    let db = db();
    assert_eq!(
        check(&db, SWORD_ONLY, &wearer(NOVICE), 0),
        Err(Refusal::Result(EQUIP_FAIL))
    );
    assert_eq!(check(&db, SWORD_ONLY, &wearer(1), 0), Ok(()));
    assert_eq!(check(&db, KNIGHT_ONLY, &wearer(KNIGHT), 0), Ok(()));
    assert_eq!(
        check(&db, KNIGHT_ONLY, &wearer(1), 0),
        Err(Refusal::Result(EQUIP_FAIL))
    );
    assert_eq!(
        check(&db, UPPER_ONLY, &wearer(NOVICE), 0),
        Err(Refusal::Result(EQUIP_FAIL))
    );
    assert_eq!(check(&db, UPPER_ONLY, &wearer(4001), 0), Ok(()));
}

#[test]
fn eng_eq_02_ammo_pairing() {
    let db = db();
    let w = wearer(NOVICE);
    let bow = WeaponType::Bow.id().into();
    let dagger = WeaponType::Dagger.id().into();
    let msg = |msg_id| Err(Refusal::Message { msg_id });
    assert_eq!(
        check(&db, ARROW, &w, dagger),
        msg(MSI_FAIL_NEED_EQUIPPED_BOW)
    );
    assert_eq!(check(&db, ARROW, &w, 0), msg(MSI_FAIL_NEED_EQUIPPED_BOW));
    assert_eq!(check(&db, ARROW, &w, bow), Ok(()));
    assert_eq!(check(&db, ARROW, &w, WeaponType::Whip.id().into()), Ok(()));
    assert_eq!(check(&db, BULLET, &w, bow), msg(MSI_WRONG_BULLET));
    assert_eq!(
        check(&db, BULLET, &w, WeaponType::Rifle.id().into()),
        Ok(())
    );
    let revolver = WeaponType::Revolver.id().into();
    assert_eq!(check(&db, GRENADE, &w, revolver), msg(MSI_WRONG_BULLET));
    assert_eq!(
        check(&db, GRENADE, &w, WeaponType::Grenade.id().into()),
        Ok(())
    );
    assert_eq!(
        check(&db, VENOM_KNIFE, &w, dagger),
        Err(Refusal::Result(EQUIP_FAIL))
    );
    assert_eq!(check(&db, SHURIKEN, &w, 0), Ok(()));
    assert_eq!(
        check(&db, CANNONBALL, &wearer(4058), 0),
        msg(MSI_USESKILL_FAIL_MADOGEAR)
    );
    assert_eq!(
        check(&db, CANNONBALL, &wearer(4071), 0),
        msg(MSI_USESKILL_FAIL_CART)
    );
    assert_eq!(check(&db, CANNONBALL, &w, 0), Ok(()));
}

// ---- prepare_equip: spec §2.2 steps 3-9 ----

fn prepare(
    db: &ItemDb,
    worn: &[(u32, u32)],
    slot: usize,
    requested: u32,
    who: &Wearer,
) -> Result<u32, Refusal> {
    let s = slots(worn);
    let w = WornSlots::from_slots(&s);
    let entry = s[slot].as_ref().expect("slot holds the item");
    prepare_equip(entry, requested, who, &Gear::new(&w, &s, db))
}

#[test]
fn eng_eq_06_prepare_refusals() {
    let db = db();
    let w = wearer(ASSASSIN);
    let s = slots(&[(KNIFE, 0)]);
    let worn = WornSlots::from_slots(&s);
    let mut unidentified = InventoryItem::new(ItemId(KNIFE), 1);
    unidentified.identified = false;
    assert_eq!(
        prepare_equip(&unidentified, EQP_HAND_R, &w, &Gear::new(&worn, &s, &db)),
        Err(Refusal::Result(EQUIP_FAIL))
    );
    let unknown = InventoryItem::new(ItemId(4_000_000), 1);
    assert_eq!(
        prepare_equip(&unknown, EQP_HAND_R, &w, &Gear::new(&worn, &s, &db)),
        Err(Refusal::NoAck)
    );
    assert_eq!(
        prepare(&db, &[(JELLOPY, 0)], 0, EQP_HAND_R, &w),
        Err(Refusal::NoAck)
    );
    assert_eq!(
        prepare(&db, &[(PET_ARMOR, 0)], 0, EQP_HAND_R, &w),
        Err(Refusal::NoAck)
    );
    assert_eq!(
        prepare(&db, &[(KNIFE, EQP_HAND_R)], 0, EQP_HAND_R, &w),
        Err(Refusal::Result(EQUIP_FAIL)),
        "already worn"
    );
    assert_eq!(
        prepare(&db, &[(GUARD, 0)], 0, EQP_HAND_R, &w),
        Err(Refusal::Result(EQUIP_FAIL)),
        "wrong position"
    );
    assert_eq!(
        prepare(&db, &[(DAMASCUS, 0)], 0, EQP_HAND_L, &wearer(KNIGHT)),
        Err(Refusal::Result(EQUIP_FAIL)),
        "a knight can't take a dagger in the left hand"
    );
}

#[test]
fn eng_eq_01_prepare_positions() {
    let db = db();
    let w = wearer(ASSASSIN);
    assert_eq!(
        prepare(&db, &[(KNIFE, EQP_HAND_R), (DAMASCUS, 0)], 1, EQP_ARMS, &w),
        Ok(EQP_HAND_L)
    );
    assert_eq!(prepare(&db, &[(JUR, 0)], 0, EQP_ARMS, &w), Ok(EQP_ARMS));
    // Ammo goes to the ammo position whatever the client asked for.
    let bow = (BOW, EQP_ARMS);
    assert_eq!(prepare(&db, &[bow, (ARROW, 0)], 1, 0, &w), Ok(EQP_AMMO));
    assert_eq!(
        prepare(&db, &[bow, (ARROW, 0)], 1, EQP_AMMO, &w),
        Ok(EQP_AMMO)
    );
    assert_eq!(
        prepare(&db, &[(KNIFE, EQP_HAND_R), (ARROW, 0)], 1, 0, &w),
        Err(Refusal::Message {
            msg_id: MSI_FAIL_NEED_EQUIPPED_BOW
        })
    );
}

// ---- looks ----

fn looks(db: &ItemDb, worn: &[(u32, u32)]) -> (UnitLooks, CharacterLooks) {
    let s = slots(worn);
    let w = WornSlots::from_slots(&s);
    let gear = Gear::new(&w, &s, db);
    (gear.unit_looks(), gear.character_looks())
}

#[test]
fn eng_eq_13_dual_wield_looks() {
    let db = db();
    let (unit, chr) = looks(&db, &[(KNIFE, EQP_HAND_R), (DAMASCUS, EQP_HAND_L)]);
    assert_eq!((unit.weapon, unit.shield), (KNIFE, DAMASCUS));
    assert_eq!((chr.weapon_type, chr.shield), (25, 0));
    let (unit, chr) = looks(&db, &[(TSURUGI, EQP_HAND_R), (DAMASCUS, EQP_HAND_L)]);
    assert_eq!((unit.weapon, unit.shield), (TSURUGI, DAMASCUS));
    assert_eq!((chr.weapon_type, chr.shield), (28, 0));
    let (unit, chr) = looks(&db, &[(TSURUGI, EQP_HAND_R), (GUARD, EQP_HAND_L)]);
    assert_eq!((unit.weapon, unit.shield), (TSURUGI, GUARD));
    assert_eq!(
        (chr.weapon_type, chr.shield),
        (2, 1),
        "shield = the Guard's View"
    );
    let (unit, chr) = looks(&db, &[(JUR, EQP_ARMS)]);
    assert_eq!((unit.weapon, unit.shield), (JUR, 0));
    assert_eq!((chr.weapon_type, chr.shield), (16, 0));
    // Mid-displacement: a two-hander holding only the left hand so far.
    let (unit, _) = looks(&db, &[(JUR, EQP_HAND_L)]);
    assert_eq!((unit.weapon, unit.shield), (0, JUR));
    let (unit, chr) = looks(&db, &[]);
    assert_eq!(
        (unit, chr),
        (UnitLooks::default(), CharacterLooks::default())
    );
}

#[test]
fn eng_eq_05_head_and_robe_looks() {
    let db = db();
    let (unit, chr) = looks(
        &db,
        &[
            (GOGGLES, EQP_HEAD_TOP | EQP_HEAD_MID),
            (BRAVERY_BAG, EQP_GARMENT),
        ],
    );
    assert_eq!((unit.head_top, unit.head_mid, unit.robe), (1, 0, 2));
    assert_eq!((chr.head_top, chr.head_mid, chr.robe), (1, 0, 2));
    // Goggles holding only the mid slot so far (over a Hat): still no mid view.
    let (unit, _) = looks(&db, &[(GOGGLES, EQP_HEAD_MID)]);
    assert_eq!((unit.head_top, unit.head_mid), (0, 0));
    // A costume top overrides the normal top; without it the normal view shows.
    let (unit, chr) = looks(
        &db,
        &[(HAT, EQP_HEAD_TOP), (COSTUME_HAT, EQP_COSTUME_HEAD_TOP)],
    );
    assert_eq!((unit.head_top, chr.head_top), (900, 900));
    let (unit, _) = looks(&db, &[(HAT, EQP_HEAD_TOP)]);
    assert_eq!(unit.head_top, 16);
}

fn atk(status: &GearStatus) -> u32 {
    [status.right, status.left]
        .iter()
        .flatten()
        .map(|w| w.atk)
        .sum()
}

/// Spec §2.10, with the capture-3 values: both hands' ATK, a two-hander once (also while it
/// holds only the left hand, where it is the right-hand weapon), DEF per item once, ammo
/// skipped.
#[test]
fn gear_status_counts_each_item_once() {
    let db = db();
    let refine = RefineDb::default();
    let status = |worn: &[(u32, u32)]| {
        let s = slots(worn);
        let w = WornSlots::from_slots(&s);
        Gear::new(&w, &s, &db).gear_status(&refine)
    };
    assert_eq!(status(&[]).range, 1);
    assert_eq!(status(&[]).right, None, "bare hands");
    let dual = status(&[(KNIFE, EQP_HAND_R), (DAMASCUS, EQP_HAND_L)]);
    assert_eq!(atk(&dual), 135);
    assert_eq!(dual.left.map(|w| w.atk), Some(118));
    assert!(!dual.shield);
    assert_eq!(
        atk(&status(&[(TSURUGI, EQP_HAND_R), (DAMASCUS, EQP_HAND_L)])),
        248
    );
    let jur = status(&[(JUR, EQP_ARMS)]);
    assert_eq!((atk(&jur), jur.left), (125, None));
    let mid = status(&[(JUR, EQP_HAND_L)]);
    assert_eq!(
        (mid.right.map(|w| w.weapon_type), mid.left),
        (Some(WeaponType::Katar), None),
        "a two-hander on HAND_L only is the right-hand weapon"
    );
    assert!(status(&[(KNIFE, EQP_HAND_R), (GUARD, EQP_HAND_L)]).shield);
    let mut db2 = db.get(ItemId(GOGGLES)).cloned().expect("goggles");
    db2.defense = 5;
    let mut guard = db.get(ItemId(GUARD)).cloned().expect("guard");
    guard.defense = 20;
    let mut arrow = db.get(ItemId(ARROW)).cloned().expect("arrow");
    arrow.defense = 99;
    let mut bow = db.get(ItemId(BOW)).cloned().expect("bow");
    bow.range = 5;
    let db = ItemDb::from_items("def", vec![db2, guard, arrow, bow]).expect("loads");
    let s = slots(&[
        (GOGGLES, EQP_HEAD_TOP | EQP_HEAD_MID),
        (GUARD, EQP_HAND_L),
        (ARROW, EQP_AMMO),
    ]);
    let w = WornSlots::from_slots(&s);
    assert_eq!(Gear::new(&w, &s, &db).gear_status(&refine).hard_def, 25);
    let s = slots(&[(BOW, EQP_ARMS)]);
    let w = WornSlots::from_slots(&s);
    let bow_status = Gear::new(&w, &s, &db).gear_status(&refine);
    assert_eq!((atk(&bow_status), bow_status.range), (15, 5));
}

/// Refine bonuses: only `Refineable` items refined above 0, weapons by weapon level, armor
/// summed over every worn armor (spec §2.2 "Refine").
#[test]
fn gear_status_reads_the_refine_table() {
    let mut knife = weapon(KNIFE, WeaponType::Dagger, EQP_HAND_R, 17);
    knife.refineable = true;
    knife.weapon_level = 1;
    let mut hat = armor(HAT, EQP_HEAD_TOP, 16);
    hat.refineable = true;
    hat.armor_level = 1;
    let mut guard = armor(GUARD, EQP_HAND_L, 1);
    guard.refineable = true;
    guard.armor_level = 1;
    let mut goggles = armor(GOGGLES, EQP_HEAD_MID, 1);
    goggles.armor_level = 1;
    let mut items = vec![knife, hat, guard, goggles];
    for d in &mut items {
        d.aegis_name = format!("Item{}", d.id.0);
    }
    let db = ItemDb::from_items("refine", items).expect("loads");
    // Cumulative per level, 20 levels.
    let steps = |per: u32| (1..=20).map(|i| i * per).collect::<Vec<u32>>();
    let row = |group, level, bonus: Vec<u32>, random_bonus: Vec<u32>| ro_db::RefineRow {
        group,
        level,
        bonus,
        random_bonus,
    };
    let refine = RefineDb::from_rows(
        "refine",
        vec![
            row(RefineGroup::Armor, 1, steps(100), vec![]),
            row(
                RefineGroup::Weapon,
                1,
                steps(200),
                steps(300).into_iter().map(|v| v - 300).collect(),
            ),
        ],
    )
    .expect("table");
    let mut s = slots(&[
        (KNIFE, EQP_HAND_R),
        (HAT, EQP_HEAD_TOP),
        (GUARD, EQP_HAND_L),
        (GOGGLES, EQP_HEAD_MID),
    ]);
    for (slot, level) in [(0, 2), (1, 2), (2, 1), (3, 2)] {
        if let Some(item) = s[slot].as_mut() {
            item.refine = level;
        }
    }
    let w = WornSlots::from_slots(&s);
    let status = Gear::new(&w, &s, &db).gear_status(&refine);
    assert_eq!(
        status.right.map(|w| w.refine),
        Some(RefineBonus {
            bonus: 400,
            random_bonus: 300
        })
    );
    assert_eq!(
        status.armor_refine, 300,
        "hat +2 and guard +1; goggles aren't refineable"
    );
}

#[test]
fn equip_view_only_for_visible_positions() {
    let db = db();
    assert_eq!(equip_view(data(&db, HAT), EQP_HEAD_TOP), 16);
    assert_eq!(equip_view(data(&db, GUARD), EQP_HAND_L), 0);
    assert_eq!(equip_view(data(&db, BRAVERY_BAG), EQP_GARMENT), 2);
}

#[test]
fn worn_slots_track_each_position() {
    let s = slots(&[(JUR, EQP_ARMS), (HAT, EQP_HEAD_TOP)]);
    let mut worn = WornSlots::from_slots(&s);
    assert_eq!(worn.at(EQP_HAND_R), Some(0));
    assert_eq!(worn.at(EQP_HAND_L), Some(0));
    assert_eq!(worn.at(EQP_HEAD_TOP), Some(1));
    assert_eq!(worn.at(EQP_SHOES), None);
    worn.clear_slot(0);
    assert_eq!(worn.at(EQP_ARMS), None);
    worn.assign(EQP_HAND_L, 0);
    assert_eq!((worn.at(EQP_HAND_L), worn.at(EQP_HAND_R)), (Some(0), None));
}

#[test]
fn ammo_cascade_rules() {
    let bow = WeaponType::Bow.id().into();
    assert!(ammo_fits_weapon(AmmoType::Arrow, bow));
    assert!(!ammo_fits_weapon(
        AmmoType::Arrow,
        WeaponType::Dagger.id().into()
    ));
    assert!(!ammo_fits_weapon(AmmoType::Bullet, bow));
    assert!(ammo_fits_weapon(AmmoType::Shuriken, 0));
    assert!(is_ranged(WeaponType::Bow) && is_ranged(WeaponType::Whip));
    assert!(is_ranged(WeaponType::Grenade) && !is_ranged(WeaponType::Katar));
}

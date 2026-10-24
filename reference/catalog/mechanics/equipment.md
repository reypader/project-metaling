# Equipping and Unequipping Items (renewal, PACKETVER 20260219)

Wire formats are in [../packets/equipment.md](../packets/equipment.md). Inventory item storage and list packets: [inventory.md](./inventory.md), [../packets/inventory.md](../packets/inventory.md). Derived stats: [stats.md](./stats.md). Combat: [combat.md](./combat.md).

**Provenance:** repo `rathena` (`$RATHENA_PATH`), `src/map/pc.cpp`, `pc.hpp`, `clif.cpp`, `status.cpp`, `itemdb.cpp`, `src/common/mmo.hpp`, `db/re/item_db_equip.yml`, `db/re/job_aspd.yml`. **Applies to `20260219` renewal: yes** (`RENEWAL` and `RENEWAL_ASPD` branches taken). A weapon swap is capture-verified ([packets/equipment.md#captured-weapon-swap](../packets/equipment.md#captured-weapon-swap)); the rest is source reading.

## 1. Slot model

### `EQP_*` location bitmask (`mmo.hpp:336-363`)

| Name | Hex | Dec | `item_db` `Locations:` key |
| :--- | :--- | :--- | :--- |
| `EQP_HEAD_LOW` | `0x000001` | 1 | `Head_Low` |
| `EQP_HAND_R` (weapon) | `0x000002` | 2 | `Right_Hand` |
| `EQP_GARMENT` | `0x000004` | 4 | `Garment` |
| `EQP_ACC_R` | `0x000008` | 8 | `Right_Accessory` |
| `EQP_ARMOR` | `0x000010` | 16 | `Armor` |
| `EQP_HAND_L` (shield) | `0x000020` | 32 | `Left_Hand` |
| `EQP_SHOES` | `0x000040` | 64 | `Shoes` |
| `EQP_ACC_L` | `0x000080` | 128 | `Left_Accessory` |
| `EQP_HEAD_TOP` | `0x000100` | 256 | `Head_Top` |
| `EQP_HEAD_MID` | `0x000200` | 512 | `Head_Mid` |
| `EQP_COSTUME_HEAD_TOP` | `0x000400` | 1024 | `Costume_Head_Top` |
| `EQP_COSTUME_HEAD_MID` | `0x000800` | 2048 | `Costume_Head_Mid` |
| `EQP_COSTUME_HEAD_LOW` | `0x001000` | 4096 | `Costume_Head_Low` |
| `EQP_COSTUME_GARMENT` | `0x002000` | 8192 | `Costume_Garment` |
| (unused costume floor) | `0x004000` | 16384 | commented out |
| `EQP_AMMO` | `0x008000` | 32768 | `Ammo` |
| `EQP_SHADOW_ARMOR` | `0x010000` | 65536 | `Shadow_Armor` |
| `EQP_SHADOW_WEAPON` | `0x020000` | 131072 | `Shadow_Weapon` |
| `EQP_SHADOW_SHIELD` | `0x040000` | 262144 | `Shadow_Shield` |
| `EQP_SHADOW_SHOES` | `0x080000` | 524288 | `Shadow_Shoes` |
| `EQP_SHADOW_ACC_R` | `0x100000` | 1048576 | `Shadow_Right_Accessory` |
| `EQP_SHADOW_ACC_L` | `0x200000` | 2097152 | `Shadow_Left_Accessory` |

Combined masks (`pc.hpp:1130-1139`, `mmo.hpp:361-362`): `EQP_ARMS = HAND_R|HAND_L = 0x22` (YAML key `Both_Hand`, i.e. two-handed), `EQP_ACC = ACC_R|ACC_L = 0x88` (`Both_Accessory`), `EQP_HELM = LOW|MID|TOP = 0x301`, `EQP_COSTUME = four costume slots = 0x3C00`, `EQP_COSTUME_HELM = 0x1C00`, `EQP_SHADOW_ACC = 0x300000`, `EQP_SHADOW_ARMS = 0x60000`, `EQP_SHADOW_GEAR = 0x3F0000`. Multi-key `Locations:` entries OR together (a headgear with `Head_Top` + `Head_Mid` has `0x300`). Type check: any shadow location requires item type `ShadowGear`, otherwise the item degrades to Etc (`itemdb.cpp:~455-472`). Ammo items live in `item_db_etc.yml` with `Locations: Ammo: true`.

The wire `location`/`position` fields carry these same values unchanged (u32 at this packetver).

### `EQI_*` index enum and `sd->equip_index[]` (`pc.hpp:71-96`, table `pc.cpp:~685-708`)

`sd->equip_index[EQI_MAX]` is `int16`, value = inventory slot (server index, not wire index) of the equipped item or `-1`. Built at login by `pc_setequipindex` (`pc.cpp:1562`) from each inventory item's `equip` mask (every set bit points at the same slot). A two-slot item therefore appears in two entries.

| EQI | Val | Mask | EQI | Val | Mask |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `EQI_ACC_L` | 0 | `EQP_ACC_L` | `EQI_COSTUME_HEAD_TOP` | 10 | `EQP_COSTUME_HEAD_TOP` |
| `EQI_ACC_R` | 1 | `EQP_ACC_R` | `EQI_COSTUME_HEAD_MID` | 11 | `EQP_COSTUME_HEAD_MID` |
| `EQI_SHOES` | 2 | `EQP_SHOES` | `EQI_COSTUME_HEAD_LOW` | 12 | `EQP_COSTUME_HEAD_LOW` |
| `EQI_GARMENT` | 3 | `EQP_GARMENT` | `EQI_COSTUME_GARMENT` | 13 | `EQP_COSTUME_GARMENT` |
| `EQI_HEAD_LOW` | 4 | `EQP_HEAD_LOW` | `EQI_AMMO` | 14 | `EQP_AMMO` |
| `EQI_HEAD_MID` | 5 | `EQP_HEAD_MID` | `EQI_SHADOW_ARMOR` | 15 | `EQP_SHADOW_ARMOR` |
| `EQI_HEAD_TOP` | 6 | `EQP_HEAD_TOP` | `EQI_SHADOW_WEAPON` | 16 | `EQP_SHADOW_WEAPON` |
| `EQI_ARMOR` | 7 | `EQP_ARMOR` | `EQI_SHADOW_SHIELD` | 17 | `EQP_SHADOW_SHIELD` |
| `EQI_HAND_L` | 8 | `EQP_HAND_L` | `EQI_SHADOW_SHOES` | 18 | `EQP_SHADOW_SHOES` |
| `EQI_HAND_R` | 9 | `EQP_HAND_R` | `EQI_SHADOW_ACC_R` | 19 | `EQP_SHADOW_ACC_R` |
| `EQI_COMPOUND_ON` | -1 | (sentinel) | `EQI_SHADOW_ACC_L` | 20 | `EQP_SHADOW_ACC_L` |

`EQI_MAX = 21`. The order is rAthena-internal; the bitmask is what the client sees. `pc_checkequip(sd, mask, checkall)` (`pc.cpp:7472`) returns the index for the first `EQI` whose bit is in `mask` (returns -1 if empty).

### Duplicate-slot rule (`pc_is_same_equip_index`, `pc.cpp:15308`)

An item occupying several `EQI` entries must be processed once for bonuses. A slot entry is skipped when it is the same item as: `HAND_R` when `HAND_L` shares it (2H / both-hand); `HEAD_MID` when `HEAD_LOW` shares it; `HEAD_TOP` when `MID` or `LOW` shares it; same two rules for costume head slots.

### Two-handed and dual wield

* Two-handed weapons (and other `Both_Hand` items) have `item.equip = EQP_ARMS`. Equipping one sets both `HAND_R` and `HAND_L` bits in `equip`, and the slot loop unequips whatever is in either hand (so a shield is removed).
* One-hand weapons have `equip = EQP_HAND_R`. `pc_equippoint_sub` (`pc.cpp:1464-1481`) widens it to `EQP_ARMS` (also `EQP_SHADOW_WEAPON` -> `EQP_SHADOW_ARMS`) only when the weapon subtype is `W_DAGGER`, `W_1HSWORD` or `W_1HAXE` **and** the character has skill `AS_LEFT` (Left Hand Mastery) > 0 or its second-class mask is Assassin or Kagerou/Oboro. Other classes (and non-dagger/sword/axe weapons) cannot take a weapon in the left hand.
* Left-hand weapon: stored with `equip = EQP_HAND_L` only, `status.shield` is forced to 0 (`W_FIST`) and `weapontype2` = its subtype. Treated as shield-less for ASPD/shield checks.
* Slot choice for a dual-wield capable weapon (`pc.cpp:~12137-12148`, renewal branch): if the request names both hands, pick left only if right is occupied and left is empty, otherwise right. If the request names exactly one hand, use it.
* Weapon type resolution (`pc_calcweapontype`, `pc.cpp:1516`): if one hand is fist, `status.weapon` = the other hand's type. If both are weapons it maps the pair to a dual constant: dagger+dagger `W_DOUBLE_DD`, sword+sword `W_DOUBLE_SS`, axe+axe `W_DOUBLE_AA`, dagger+sword `W_DOUBLE_DS`, dagger+axe `W_DOUBLE_DA`, sword+axe `W_DOUBLE_SA` (order-independent); any other pair (including a 2H weapon that sets both `weapontype1` and `weapontype2` to the same type) falls back to the right-hand type.

### Accessory auto-slot (`pc.cpp:~12118-12138`)

An item whose capability is `EQP_ACC` (both accessory slots) goes into one slot: take `request & EQP_ACC`; if the request names both, choose left only when right is already occupied and left is empty, else right. If the accessory has a card inserted whose own location is `EQP_ACC_L` or `EQP_ACC_R`, that card's location overrides the choice. The same logic applies to shadow accessories (`EQP_SHADOW_ACC`, choosing right if left occupied and right empty, else left).

### Ammo

> Open findings: [PF-18](../../../PACKET_FINDINGS.md#pf-18).

* Item type `Ammo` (`IT_AMMO`), `SubType` is `e_ammo_type` (`pc.hpp:998-1010`): `Arrow=1, Dagger=2, Bullet=3, Shell=4, Grenade=5, Shuriken=6, Kunai=7, Cannonball=8, Throwweapon=9`. `Locations: Ammo`.
* Client does not send a position for ammo; server substitutes `EQP_AMMO` (packets file).
* `pc_isequip` weapon/ammo pairing (only if `ammo_check_weapon` = yes, default yes in `conf/battle/battle.conf:177`): arrows need `W_BOW`/`W_MUSICAL`/`W_WHIP` equipped; bullets and shells need `W_REVOLVER`/`W_RIFLE`/`W_GATLING`/`W_SHOTGUN`/`W_GRENADE` (renewal); dagger ammo needs skill `AS_VENOMKNIFE`; cannonball needs Mechanic in Mado Gear and Genetic with cart. Shuriken/Kunai/Throwweapon have no check here.
* With `ammo_unequip` = yes (default): equipping a weapon that cannot use the current ammo unequips the ammo, and unequipping a ranged weapon unequips the ammo.

## 2. Item fields that matter (`item_db_equip.yml`, parser `itemdb.cpp`)

| Field | Effect on equip |
| :--- | :--- |
| `Type` (Weapon, Armor, Ammo, ShadowGear) | `itemdb_isequip2` true only for these four (`itemdb.cpp:3253`); other types are never equippable |
| `SubType` | weapon: `W_*` (below); ammo: `AMMO_*` |
| `Locations` | `item.equip` capability mask |
| `View` | `item.look`: headgear/garment/costume sprite id; weapons/shields: legacy view (shield needs non-zero or defaults to 1) |
| `AliasName` | `item.view_id`: another item's id sent instead of own id for weapon/shield look |
| `EquipLevelMin` / `EquipLevelMax` | `item.elv` / `elvmax` vs base level |
| `Gender` | `Both` default; musical weapons forced male, whips female (`itemdb.cpp:~1229-1245`) |
| `Jobs` | `class_base[3]` bitmask |
| `Classes` | `class_upper` (normal/upper/baby/third/third-upper/third-baby/fourth) |
| `WeaponLevel` / `ArmorLevel` | weapon level (clamped to 5) / armor level; renewal level-5 weapons add refine*2 to P.ATK and S.MATK, level-2 armor adds refine*2 to RES and MRES |
| `Attack`, `MagicAttack`, `Defense`, `Range` | base values added to status (below) |
| `Slots` | number of card slots |
| `Refineable`, `Gradable` | gate for refine/enchant-grade bonuses |
| `Script`, `EquipScript`, `UnEquipScript` | `Script` is recomputed on every `status_calc_pc`; Equip/UnEquip scripts run once at the event |
| Flags (`BindOnEquip`, `NoEquip` map restriction) | see validation |

### Weapon type enum `W_*` (`pc.hpp:958-996`)

`FIST 0, DAGGER 1, 1HSWORD 2, 2HSWORD 3, 1HSPEAR 4, 2HSPEAR 5, 1HAXE 6, 2HAXE 7, MACE 8, 2HMACE 9 (unused), STAFF 10, BOW 11, KNUCKLE 12, MUSICAL 13, WHIP 14, BOOK 15, KATAR 16, REVOLVER 17, RIFLE 18, GATLING 19, SHOTGUN 20, GRENADE 21, HUUMA 22, 2HSTAFF 23`, `MAX_WEAPON_TYPE = 24`; dual constants `DOUBLE_DD 25 ... DOUBLE_SA 30` (value 24 is `MAX_WEAPON_TYPE`, used as the "Shield" index in the ASPD table; the dual constants follow it).

## 3. `pc_equipitem` flow

Source: `pc.cpp:12015-12396`.

Inputs: inventory slot `n`, requested mask `req_pos`.

1. `n` outside `[0, MAX_INVENTORY)`: return false, no ack. `canequip_tick` still in the future: ack FAIL.
2. No item data: return false, no ack.
3. `pos = pc_equippoint(n)` (capability mask; dual-wield widening applied).
4. `pc_isequip(n)` result `!= OK`: ack with that result (FAIL or FAILLEVEL), return false. Checks, in order (`pc.cpp:1850-1960`):
   - permission `PC_PERM_USE_ALL_EQUIPMENT`: always OK.
   - no item data: FAIL.
   - Soul Linked Super Novice shortcuts (base level >= 90: any headgear; >= 96: any level-4 dagger/1h sword/1h axe/2h axe/mace/staff/2h staff).
   - `base_level < elv` or `> elvmax` (when set): FAILLEVEL.
   - gender mismatch: FAIL.
   - broken (`attribute == 1`): FAIL.
   - map `NoEquip` restriction (only when `allow_equip_restricted_item` is false; default yes, so allowed): FAIL.
   - ammo pairing checks (above), each with a `clif_msg`.
   - statuses: Strip Weapon (any weapon in either hand), Strip Shield (armor in left hand), Strip Armor, Strip Helm (any item with `HEAD_TOP`), `SC__STRIPACCESSORY`, `SC__WEAKNESS` (either hand), `SC_SHADOW_STRIP` (shadow gear), `SC_KYOUGAKU`/`SC_SUHIDE` (everything): FAIL.
   - job (`Jobs` bit by first-class id and 2-1/2-2 branch) and class (`Classes`, upper/baby/third/fourth rules, renewal variant): FAIL.
5. Equip-switch only: ammo rejected. (Skipped for normal equips.)
6. Fit check: `(pos & req_pos) == 0` **or the item's `equip` is already non-zero** (already worn): FAIL ack.
7. `sc.cant.equip` or `SC_PYROCLASTIC` with a weapon: FAIL ack.
8. Bind-on-equip: if flagged and not yet bound, set `bound = default_bind_on_equip` (4, `conf/battle/items.conf:109`) and send `clif_notify_bindOnEquip`.
9. Resolve the concrete slot mask `pos` (accessory / dual-wield / shadow rules above).
10. If the new item lands on `HAND_R` and `skillrange_from_weapon` applies to players: remember whether weapon range differs (re-send skill info later).
11. For every `EQI` whose bit is in `pos`: if an item is already there, `pc_unequipitem(thatItem, flag 1|2|4)` (calc, forced, no skill/ammo side effects; sends its own unequip ack), then `equip_index[i] = n`. Also drops the item from the equip-switch list.
12. Ack: ammo -> `0x013c` + `0x013b type 3`; else `0x0999` OK with the final mask and the `View` id for visible slots. Then `inventory[n].equip = pos`.
13. Weapon state: if `pos` has `HAND_R`: `weapontype1 = item subtype`, `pc_calcweapontype`, send `LOOK_WEAPON`. If `pos` has `HAND_L`: weapon -> `status.shield = 0`, `weapontype2 = subtype`; armor (shield) -> `status.shield = item.look`, `weapontype2 = W_FIST`; recalc type; send `LOOK_SHIELD` (merged into the same `0x01d7`). Shoes slot: send `LOOK_SHOES`.
14. `ammo_unequip`: if a weapon was equipped into either hand and current ammo cannot be used with it (arrow without bow/instrument/whip; bullet/shell without gun), unequip the ammo (flag 2|4).
15. `pc_set_costume_view`: recompute `status.head_bottom/head_mid/head_top/robe` and send `LOOK_HEAD_*` / `LOOK_ROBE` for those that changed (rules in section 5).
16. `pc_checkallowskill` (end weapon/shield-dependent statuses), item combo check (item and each card), `status_calc_pc(SCO_NONE)`, optional skill-info re-send.
17. Run `EquipScript` of the item, then of each card in the item (not for forged/special-card weapons whose `card[0]` is special). Skipped if the map restricts the item, unless the player has the all-equipment permission.

On failure after step 4 nothing is mutated.

### 3a. Job, class and gender checks

Source: rAthena `d4b8e7b8f16061cc2496d377ac8f777ce72a4f39` (checked-out HEAD, 2026-10-06). Applies to `20260219` renewal (`RENEWAL` branch). Files: `src/map/pc.cpp` (`pc_job_can_use_item` 1771-1792, `pc_isItemClass` 1804-1842, `pc_isequip` 1850-1968, `pc_jobid2mapid` 7519), `src/map/map.hpp:86-262` (mapid enum), `src/map/itemdb.cpp` (`Jobs` 330-374, `Classes` 376-421, `Gender` 423-445, `defaultGender` 1227, `itemdb_jobid2mapid` 3201), `src/map/itemdb.hpp:211-231`, `src/common/mmo.hpp` (`e_job`, `e_sex`), `src/map/script_constants.hpp:276-420`.

**Check order inside `pc_isequip`** (each line returns at the first hit):

| # | Check | Result |
| :-- | :-- | :-- |
| 1 | `PC_PERM_USE_ALL_EQUIPMENT` | OK, skips everything (even before the null-item check) |
| 2 | no item data | FAIL |
| 3 | Soul Linked Super Novice shortcut (see §3 step 4) | OK |
| 4 | `elv != 0 && base_level < elv` | FAILLEVEL |
| 5 | `elvmax != 0 && base_level > elvmax` | FAILLEVEL |
| 6 | `item.sex != BOTH && char.sex != item.sex` | FAIL |
| 7 | broken, map NoEquip, ammo pairing, strip statuses (see §3 step 4) | FAIL |
| 8 | `pc_job_can_use_item` false | FAIL |
| 9 | `pc_isItemClass` false | FAIL |

So level failures take priority over gender, job and class failures; gender is checked before job and class. Wire codes: FAILLEVEL = 1, FAIL = 2 (`clif.hpp:525-527`, ack `0x0999`).

#### Gender
- `e_sex` (`mmo.hpp:1116`): `FEMALE 0, MALE 1, BOTH 2, SERVER 3`. The character's `status.sex` is 0 or 1 (account sex; `SERVER` is for accounts that are the server).
- Check is plain inequality (step 6 above). Item `Gender` YAML: `Female | Male | Both`; unknown value defaults to `Both`; omitted on a new item defaults to `Both`.
- Forced by `defaultGender` (applied after parsing, overrides any YAML value): item `WEDDING_RING_M` (Groom's Ring) = Male, `WEDDING_RING_F` (Bride's Ring) = Female, weapon subtype `W_MUSICAL` = Male, `W_WHIP` = Female.

#### Job id -> mapid (`sd->class_`)
`sd->class_ = pc_jobid2mapid(status.class_)`; an id with no mapping (or not in `job_db`) is reset to Novice at login (`pc.cpp:2099-2104`). The client-visible mounted/alternate ids (`KNIGHT2`=13, `CRUSADER2`=21, `LORD_KNIGHT2`, `PALADIN2`, `BABY_KNIGHT2`, `*_2ND`, `RUNE_KNIGHT2`, `RANGER2`, `STAR_EMPEROR2`, `SKY_EMPEROR2`, ...) have no mapping, so they are invalid as stored class ids. Male/female pairs map to one mapid: Bard/Dancer -> `BARDDANCER`, Clown/Gypsy -> `CLOWNGYPSY`, Minstrel/Wanderer(+`_T`, baby) -> `MINSTRELWANDERER`, Troubadour/Trouvere -> `TROUBADOURTROUVERE`, Kagerou/Oboro (+baby) -> `KAGEROUOBORO`, Shinkiro/Shiranui -> `SHINKIROSHIRANUI`.

Bit layout (`map.hpp:86-102`):

| Bits | Mask | Meaning |
| :-- | :-- | :-- |
| 0-7 | `0xff` (`MAPID_FIRSTMASK`) | base-class number (table below) |
| 8 | `0x100` `JOBL_2_1` | 2-1 branch |
| 9 | `0x200` `JOBL_2_2` | 2-2 branch |
| 12 | `0x1000` `JOBL_THIRD` | third (also set on 4th) |
| 16 | `0x10000` `JOBL_FOURTH` | fourth |
| 20 | `0x100000` `JOBL_UPPER` | transcendent |
| 21 | `0x200000` `JOBL_BABY` | baby |

Other masks: `SECONDMASK 0xfff`, `THIRDMASK 0xffff`, `FOURTHMASK 0xfffff`. Super Novice is the 2-1 of Novice (`0x100`), not a first class.

Base-class number (`mapid & 0xff`) by branch (same number reused across branches):

| n | 1st (no branch) | 2-1 (`0x100`) | 2-2 (`0x200`) |
| :-- | :-- | :-- | :-- |
| 0 | Novice | Super Novice | - |
| 1 | Swordman | Knight | Crusader |
| 2 | Mage | Wizard | Sage |
| 3 | Archer | Hunter | BardDancer |
| 4 | Acolyte | Priest | Monk |
| 5 | Merchant | Blacksmith | Alchemist |
| 6 | Thief | Assassin | Rogue |
| 7 | Taekwon | Star Gladiator | Soul Linker |
| 8 | Gunslinger | Rebellion | - |
| 9 | Ninja | KagerouOboro | - |
| 10 | Summoner | Spirit Handler | - |
| 11 | Gangsi | Death Knight | Dark Collector |
| 12-17 | Wedding, Xmas, Summer, Hanbok, Oktoberfest, Summer2 (costume jobs) | - | - |

Composition: transcendent = `UPPER | 2nd-class mapid` (Lord Knight = `UPPER|KNIGHT`, Paladin = `UPPER|CRUSADER`; Novice High/1st-class highs = `UPPER|n`); baby = `BABY | mapid` (Super Baby = `BABY|SUPER_NOVICE`); third = `THIRD | 2nd-class mapid` (Rune Knight = `THIRD|KNIGHT`, Royal Guard = `THIRD|CRUSADER`, Super Novice E = `THIRD|SUPER_NOVICE`, Star Emperor = `THIRD|STAR_GLADIATOR`, Soul Reaper = `THIRD|SOUL_LINKER`, Night Watch = `THIRD|REBELLION`, Shinkiro/Shiranui = `THIRD|KAGEROUOBORO`); trans-third (`*_T`) = `THIRD|UPPER|2nd`; baby-third = `THIRD|BABY|2nd`; fourth = `FOURTH|THIRD|2nd-class mapid` (Hyper Novice = `FOURTH|SUPER_NOVICE_E`, Dragon Knight = `FOURTH|RUNE_KNIGHT`, Imperial Guard = `FOURTH|ROYAL_GUARD`, Sky Emperor = `FOURTH|STAR_EMPEROR`, Soul Ascetic = `FOURTH|SOUL_REAPER`). Fourth classes carry no UPPER/BABY bit. There are no baby fourth classes and no baby Death Knight/Spirit Handler/Gangsi.

Selected numeric job ids (`mmo.hpp` `e_job`, client ids): Novice 0, Swordman 1, Mage 2, Archer 3, Acolyte 4, Merchant 5, Thief 6, Knight 7, Priest 8, Wizard 9, Blacksmith 10, Hunter 11, Assassin 12, Crusader 14, Monk 15, Sage 16, Rogue 17, Alchemist 18, Bard 19, Dancer 20, Wedding 22, Super Novice 23, Gunslinger 24, Ninja 25, Xmas 26, Summer 27, Hanbok 28, Oktoberfest 29, Summer2 30; Novice High 4001 .. Thief High 4007, Lord Knight 4008, High Priest 4009, High Wizard 4010, Whitesmith 4011, Sniper 4012, Assassin Cross 4013, Paladin 4015, Champion 4016, Professor 4017, Stalker 4018, Creator 4019, Clown 4020, Gypsy 4021; Baby 4023 ..; Super Baby 4045; Taekwon 4046, Star Gladiator 4047, Soul Linker 4049, Gangsi 4050, Death Knight 4051, Dark Collector 4052; Rune Knight 4054, Warlock 4055, Ranger 4056, Arch Bishop 4057, Mechanic 4058, Guillotine Cross 4059, `*_T` 4060-4065, Royal Guard 4066, Sorcerer 4067, Minstrel 4068, Wanderer 4069, Sura 4070, Genetic 4071, Shadow Chaser 4072, `*_T` 4073-4079; baby thirds 4096-4108; Super Novice E 4190, Super Baby E 4191; Kagerou 4211, Oboro 4212; Rebellion 4215; Summoner 4218; Baby Ninja 4222; Baby Taekwon 4225 ..; Star Emperor 4239, Soul Reaper 4240; Dragon Knight 4252 .. Trouvere 4264 (4th jobs 4252-4264); Sky Emperor 4302, Soul Ascetic 4303, Shinkiro 4304, Shiranui 4305, Night Watch 4306, Hyper Novice 4307, Spirit Handler 4308. (Enum is sequential from the anchors shown in `e_job`; verify any id not listed by counting from the nearest explicit `= N`.)

#### `Jobs` -> `class_base[3]`
- `class_base` is three `u64` bitsets: index 0 = first class and Novice branch (no 2-x bit), index 1 = 2-1 branch, index 2 = 2-2 branch. Bit position = base-class number `n` from the table above.
- A `Jobs` name resolves to the constant `EAJ_<Name>` (case-insensitive lookup), whose value is the mapid. It sets/clears bit `(mapid & 0xff)` in index 1 if `mapid & 0x100`, else index 2 if `mapid & 0x200`, else index 0. The `UPPER/BABY/THIRD/FOURTH` bits of the constant are discarded, so `Knight` is one bit shared by Knight, Lord Knight, Rune Knight, Dragon Knight, Baby Knight and Baby Rune Knight.
- The `Jobs` map value is a bool: `true` sets the bit, `false` clears it. Entries apply in file order after `All`.
- `All: true` sets all three bitsets to all ones (`UINT64_MAX`); `All: false` is a no-op clear.
- `Jobs` present: the three sets are reset to 0 first (so an overriding entry replaces the old list, not merges), then `All`, then each named entry.
- `Jobs` absent: new item -> all ones in all three sets (any job); an existing item being overridden keeps its previous value.
- Unknown job name: warning and the item becomes usable by all jobs (all ones, parsing of the rest of the map stops).

Name -> (index, bit) for the names used by `db/re/item_db_equip.yml` (plus the other `EAJ_` names that resolve to first-class/2-x bits):

| `Jobs` name | index | bit | `Jobs` name | index | bit |
| :-- | :-- | :-- | :-- | :-- | :-- |
| Novice | 0 | 0 | SuperNovice (`Super_Novice`) | 1 | 0 |
| Swordman | 0 | 1 | Knight | 1 | 1 |
| Mage | 0 | 2 | Wizard | 1 | 2 |
| Archer | 0 | 3 | Hunter | 1 | 3 |
| Acolyte | 0 | 4 | Priest | 1 | 4 |
| Merchant | 0 | 5 | Blacksmith | 1 | 5 |
| Thief | 0 | 6 | Assassin | 1 | 6 |
| Taekwon | 0 | 7 | StarGladiator (`Star_Gladiator`) | 1 | 7 |
| Gunslinger | 0 | 8 | Rebellion | 1 | 8 |
| Ninja | 0 | 9 | KagerouOboro | 1 | 9 |
| Summoner | 0 | 10 | Spirit_Handler | 1 | 10 |
| Gangsi | 0 | 11 | Death_Knight / DeathKnight | 1 | 11 |
| Crusader | 2 | 1 | Sage | 2 | 2 |
| BardDancer | 2 | 3 | Monk | 2 | 4 |
| Alchemist | 2 | 5 | Rogue | 2 | 6 |
| SoulLinker (`Soul_Linker`) | 2 | 7 | Dark_Collector / DarkCollector | 2 | 11 |

Other accepted names that collapse onto these bits: every `EAJ_*` constant listed in `script_constants.hpp:281-420` (`*_HIGH`, `BABY_*`, `*_T`, 3rd-job names such as `RUNE_KNIGHT`, `STAR_EMPEROR`, `SOUL_REAPER`, `HYPER_NOVICE`...) resolves by the same rule, but the shipped item DB only uses the names above (`Novice, Swordman, Mage, Archer, Acolyte, Merchant, Thief, Knight, Priest, Wizard, Blacksmith, Hunter, Assassin, Crusader, Monk, Sage, Rogue, Alchemist, BardDancer, StarGladiator, SoulLinker, Gunslinger, Ninja, KagerouOboro, Rebellion, SuperNovice, Spirit_Handler` plus `Taekwon` and `Summoner`). Consequences: Star Emperor tests (1,7) like Star Gladiator, Soul Reaper tests (2,7) like Soul Linker, Night Watch tests (1,8) like Rebellion, Shinkiro/Shiranui test (1,9) like Kagerou/Oboro, Hyper Novice and Super Novice E test (1,0) like Super Novice, Sky Emperor tests (1,7), Soul Ascetic (2,7), Imperial Guard (2,1), Dragon Knight (1,1).

#### Job test (`pc_job_can_use_item`)
Compute `bit = 1 << (class_ & 0xff)`; index = 1 if `class_ & 0x100`, else 2 if `class_ & 0x200`, else 0; pass iff `class_base[index] & bit != 0`. Novice High / Baby Novice use index 0 bit 0 like Novice; Super Baby, Super Novice E, Hyper Novice use index 1 bit 0 like Super Novice.

#### `Classes` -> `class_upper` and the class test (`pc_isItemClass`, RENEWAL branch)
- `ITEMJ_*`: `NORMAL 0x01, UPPER 0x02, BABY 0x04, THIRD 0x08, THIRD_UPPER 0x10, THIRD_BABY 0x20, FOURTH 0x40`. Groups: `All_Upper = UPPER|THIRD_UPPER|FOURTH`, `All_Baby = BABY|THIRD_BABY`, `All_Third = THIRD|THIRD_UPPER|THIRD_BABY` (so `All_Third` does not include `FOURTH`). `ITEMJ_ALL` in renewal = all seven bits (`0x7f`).
- `Classes` parsing: `All: true` ORs in `ITEMJ_ALL`, `All: false` clears it; each named entry (resolved as `ITEMJ_<Name>`) sets or clears its bits; an unknown name sets `ITEMJ_ALL` and stops. Unlike `Jobs`, `class_upper` is not reset when `Classes` is present, so an override edits the previous value. `Classes` absent on a new item = `ITEMJ_ALL`.
- The test passes at the first matching rule, tested against the character's flag bits (`UPPER 0x100000`, `BABY 0x200000`, `THIRD 0x1000`, `FOURTH 0x10000` of `class_`):

| Character flags | Item must have |
| :-- | :-- |
| none of UPPER/BABY/THIRD/FOURTH | `NORMAL` |
| UPPER, not THIRD | `UPPER` |
| BABY, not THIRD | `BABY` |
| THIRD only (no UPPER, BABY, FOURTH) | `THIRD` |
| THIRD + UPPER (no FOURTH) | `THIRD_UPPER` |
| THIRD + BABY (no FOURTH) | `THIRD_BABY` |
| FOURTH (any other flags) | `FOURTH` |

No fall-through: a transcendent char needs `UPPER`, not `NORMAL`; a third class does not accept `UPPER` or `NORMAL` items in renewal. (The non-renewal branch, which lets thirds use trans items, is not compiled.) Special cases follow from the flags and need no code: Super Novice, Taekwon, Star Gladiator, Soul Linker, Gunslinger, Ninja, Summoner, Rebellion, Kagerou/Oboro, Spirit Handler, Death Knight have no flags -> `NORMAL`; Baby Taekwon/Ninja/Gunslinger/Rebellion/Kagerou/Summoner/etc. -> `BABY`; Super Novice E, Star Emperor, Soul Reaper, Night Watch, Shinkiro/Shiranui -> `THIRD`; Super Baby E, Baby Star Emperor, Baby Soul Reaper -> `THIRD_BABY`; Hyper Novice, Sky Emperor, Soul Ascetic and all 4th jobs -> `FOURTH`. An item must pass both the job test and the class test; the 4th-class `Jobs` bits come from the 2nd-class base (Dragon Knight uses Knight's bit).

#### GM bypass
`PC_PERM_USE_ALL_EQUIPMENT` (group permission `all_equipment`, `pc_groups.hpp:26,65`; read by `pc_has_permission` = bit test of `sd->permissions`, `pc.cpp:13170`; documented `doc/permissions.txt:61-66`: "equip any item regardless of requirements (class, level, etc.)", client errors possible if the sprite is missing). The bypass is the first line of `pc_isequip`, so it also skips the level, gender, broken, NoEquip, ammo and strip-status checks and the Soul Linker shortcut. It additionally lets equip scripts and card bonuses run on NoEquip maps (`status.cpp:3920-4172`). Shipped `conf/groups.yml`: `all_equipment` is not enabled for any group (Admin group lists it as `false`), so it is off by default.

## 4. `pc_unequipitem` flow

Source: `pc.cpp:12398-12503`.

`flag` bits: 1 = recalc status, 2 = forced (ignores cannot-unequip status), 4 = equip-switch/internal (skip skill-check and ammo cascade).

1. Slot out of range: `0x099a` fail with index 0. Slot not worn (`equip == 0`): fail ack with that index.
2. Unless forced: `sc.cant.unequip`, or `SC_PYROCLASTIC` with a weapon: fail ack.
3. Clear `equip_index[]` for every bit of the item's `equip`.
4. If it covered `HAND_R`: `weapontype1 = W_FIST`, `status.weapon = weapontype2`, recalc, send `LOOK_WEAPON`, end `SC_DANCING` (unless `dancing_weaponswitch_fix`, default yes), optionally end `SC_EDP`.
5. If it covered `HAND_L`: cancel an in-progress Shield Spell cast, `status.shield = weapontype2 = W_FIST`, recalc, send `LOOK_SHIELD`.
6. Shoes: send `LOOK_SHOES` (value 0).
7. Send `0x099a` success with the item's `equip` mask (this is emitted before the item's `equip` is cleared).
8. `pc_set_costume_view` (head/robe looks).
9. End all statuses flagged `REMOVEONUNEQUIP`; if a weapon left a hand: with `ammo_unequip` and not flag 4, ranged weapon types (bow, instrument, whip, revolver, rifle, gatling, shotgun, grenade) also unequip the current ammo (own `0x099a`) and run its unequip handling; statuses flagged `REMOVEONUNEQUIPWEAPON` end. Armor slot: `REMOVEONUNEQUIPARMOR` statuses end. Ammo: `SC_P_ALTER` ends.
10. `pc_unequipitem_sub`: delete autobonus entries tied to that location, set `equip = 0`, `pc_checkallowskill` (unless flag 4), remove item/card combos, `status_calc_pc(SCO_FORCE)` if flag 1 or a combo ended, end `SC_SIGNUMCRUCIS` on non-undead, run `UnEquipScript` of item then cards.

Unequip-all (`0x0bf5`) loops all `EQI` entries with flag 1.

Other places that force-unequip with flag 2: level/class change invalidation (`pc_isequip` re-run over `equip_index`), death/other events (`pc.cpp:6116, 7005, 8311, 9331, 10090-10104, 10988`).

## 5. View handling

Server state: `sd->status.{weapon,shield,head_top,head_mid,head_bottom,robe}` (all persisted in the `char` table as `smallint unsigned`, `sql-files/main.sql:253-258`) and the per-unit `vd.look[]` array that entry packets read.

* **Weapon/shield look** (`update_look`, `pc.cpp:623`): right-hand item -> `view_id` if non-zero else `nameid`; left-hand item likewise, but 0 when left and right slots are the same item (2H); 0 when nothing is worn; both 0 under `OPTION_COSTUME`. Sent together in one `LOOK_WEAPON` packet (val = weapon, val2 = shield). `status.weapon` holds the weapon **type** (`W_*` or dual constant) and `status.shield` holds the shield **view** (`item.look`) or 0; they are distinct from the sent look.
* **Headgear/robe look** (`pc_set_costume_view`, `pc.cpp:15490`): start from zero, then normal slots: head_low -> `look` unless the same item also occupies mid/top (then 0); head_mid -> `look` unless it also occupies top (then 0); head_top -> `look`; garment -> robe `look`. Then, unless the map has `nocostume` or the character setting `disable_showcostumes` is set, costume slots override the same way (costume low/mid/top, costume garment). `setlook_*` script overrides win last. A multi-slot headgear is therefore drawn by its highest slot only (avoids the client drawing it on top of itself). A look packet is sent only when the new value differs from the previous.
* **Shoes**: look 9 = shoes `view_id` or nameid, or 0.
* The entry packets (`0x09fe`/`0x09ff`/`0x09fd`) carry `weapon`, `shield`, `head`s and `robe` straight from these values ([../packets/map_units.md](../packets/map_units.md) field table).

## 6. Effect on status and combat

`status_calc_pc` (`status.cpp:~3890-4220`) rebuilds from scratch. Equipment-related pieces, per `EQI` entry (skipping `EQI_AMMO`, and duplicates per section 1):
* `def += item.def` for every worn item; refine bonus accumulates into `refinedef`, finally `def += (refinedef + 50) / 100`.
* Weapon (`Type=Weapon`): goes to the right (`rhw`) or left (`lhw`) weapon record: left only if the item is in `EQI_HAND_L` with `equip == EQP_HAND_L`. Adds `atk`, `atk2 += refine bonus/100` (plus grade bonus), renewal `matk` (+ refine bonus unless bow), `wlv`, `range += item.range`, overrefine bonus, forged-weapon star/element from `card[0..3]`. Runs the item `Script`.
* Armor (`Type=Armor`): refine def, renewal level-2 armor RES/MRES, `Script` (left hand flagged as shield).
* Shadow gear: `Script` only.
* Ammo: `arrow_atk += item.atk`, `Script` with arrow flag (skipped for throwables).
* Combos after all items. Cards: every card in a worn item's 4 slots runs its `Script` (skipped when `card[0]` is a special marker such as forged/pet egg).
* Attack range: `rhw.range` and `lhw.range` default to at least 1, then `rhw.range = max(rhw, lhw)` (`status.cpp:4216-4219`). Bare-handed range is 1. `SP_ATTACKRANGE` is pushed to the client when it changes.
* Base attack uses the weapon type for stat formula selection (bow/instrument/whip/guns use DEX as main stat), see stats.md.

### ASPD per weapon (renewal, `status_base_amotion_pc`, `status.cpp:2355`)

* Base per class from `db/re/job_aspd.yml` `BaseASPD:` (default 2000 for unlisted weapon types, which makes them unusable): `aspd = BaseASPD[weapontype1]`; if a shield is worn (`status.shield != 0`) add `BaseASPD[Shield]` (shield index = `MAX_WEAPON_TYPE`); else if a left-hand weapon is worn add `BaseASPD[weapontype2] / 4`.
* Examples from the file: Novice: Fist 40, Dagger 55, 1hSword 57, Mace 50, Staff 65, Shield 10; Swordman: Fist 40, Dagger 47, 1hSword 47, 2hSword 54, 2hSpear 65, Shield 5. (Larger = slower; the rest of the formula is in stats.md.)
* Stat term differs by weapon: ranged types (bow, instrument, whip, revolver, rifle, gatling, shotgun, grenade) use `DEX^2/7 + AGI^2*0.5`, everything else `DEX^2/5 + AGI^2*0.5`; `temp = sqrt(that)*0.25 + 196`; final = `temp + (status_aspd_mods + skill bonuses)*AGI/200 - min(BaseASPD, 200)`. Dual-wield weapon pairs use the `status.weapon` dual constant for that stat selection only.
* Weapon-dependent skill bonuses (Advanced Book with a book, Single Action with guns, Devil, riding penalties) are applied here; full derivation belongs to stats.md.

## 7. Persistence and login

* `inventory.equip` (`sql-files/main.sql:721`) is `int unsigned`, the **currently worn `EQP_*` mask** for that row (0 = not worn). A worn two-handed weapon stores `0x22`, a right-hand weapon `0x02`, a left-hand weapon `0x20`, an accessory `0x08` or `0x80`, ammo `0x8000`. Also stored per row: `identify`, `refine`, `attribute` (1 = broken), `card0-card3`, option columns, `favorite`, `bound`, `equip_switch`, enchant grade, expire time. The cross-table state `char.weapon/shield/head_*/robe` is derived and rewritten on save.
* At login the map server loads the inventory, calls `pc_setinventorydata` (item data pointers) and `pc_setequipindex` (rebuilds `equip_index[]`, `weapontype1/2`, `status.weapon`) (`intif.cpp:3485-3486`), then `status_calc_pc(SCO_FIRST)`, which also runs the `EquipScript` of worn items and cards.
* The client learns the worn set from the inventory equip list (`WearState` = `equip` mask per entry) and `0x013c` for ammo (packets/equipment.md), and sees others via entry packets.
* Equip changes are not saved immediately: no `chrif_save` in `pc_equipitem`/`pc_unequipitem`; the `inventory.equip` mask and the `char.weapon/shield/head_*/robe` columns persist on autosave or logout. At login the map server re-derives `status.weapon` from the equipped rows; the six `char` columns only feed the character-select entry. Details and sources: [../packets/char.md](../packets/char.md#per-character-look-fields-character_info-0x006b--0x0b72--0x0b6f).
* For `ro-persistence`: an `equip` integer column (u32 range needed, values up to `0x3FFFFF` today) on the inventory/item row is sufficient; no separate equipment table exists in rAthena.

## 8. Refine and cards (field presence only)

Per-item instance fields that affect equipment: `refine` (0..`MAX_REFINE`, clamped on calc; affects weapon ATK, armor DEF, renewal weapon MATK, level-5 weapon P.ATK/S.MATK, level-2 armor RES/MRES), `enchantgrade` (extra % on the refine bonus; `RefiningLevel` + `grade` in the equip list entry), `card[4]` (u16 ids; first slot `CARD0_FORGE` marks a forged weapon whose `card[1..3]` pack element and star crumbs; other special `card[0]` markers disable card scripts), `attribute` (broken flag), `option_*` (random options), `bound`. Only the presence is recorded here; refine success rates, forging and card-compound rules are out of scope.

## Open points

* The `MSI_WRONG_BULLET` value is 2493 or 2494 depending on a packetver branch not resolved here.
* Only the one-hand weapon swap is captured. Take-off, unequip-all, shields and two-handers, headgear looks, failure acks and ammo are source reading only.
* `clif_msg` and status-change (`EFST_CLIENT_ONLY_EQUIP_ARROW`) packet layouts are not yet in the catalog.

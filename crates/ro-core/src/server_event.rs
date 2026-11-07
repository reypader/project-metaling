//! Server events: the vocabulary from engine and servers to the wire encoders.

use std::sync::Arc;

use crate::char_select_dto::{CharServerInfo, CharacterSummary};
use crate::client_ui_dto::{AchievementSummary, SkillInfo, StatusWindow};
use crate::ids_dto::{AccountId, CharId, MapPosition, Sex};
use crate::item_dto::{FloorItemAppearance, ItemEntry};
use crate::unit_dto::UnitAppearance;

/// High-level domain events emitted by logic modules and engine systems, decoupled from wire serialization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerEvent {
    LoginAccepted {
        account_id: AccountId,
        login_id1: u32,
        login_id2: u32,
        sex: Sex,
        char_servers: Vec<CharServerInfo>,
    },
    LoginRefused {
        error_code: u8,
    },
    CharacterListLoaded {
        account_id: AccountId,
        total_slots: u8,
        characters: Vec<CharacterSummary>,
    },
    CharacterListRefused {
        error_code: u8,
    },
    CharacterPageLoaded {
        characters: Vec<CharacterSummary>,
    },
    CharacterSelected {
        char_id: CharId,
        map_name: String,
        map_host: [u8; 4],
        map_port: u16,
    },
    CharacterCreated {
        character: CharacterSummary,
    },
    CharacterCreateRefused {
        error_code: u8,
    },
    CharacterDeleteReserved {
        char_id: CharId,
        result: u32,
        delete_delay_seconds: u32,
    },
    CharacterDeleteCancelled {
        char_id: CharId,
        result: u32,
    },
    Aid {
        account_id: AccountId,
    },
    InventoryExpansionInfo {
        expansion_size: i16,
    },
    AcceptEnter {
        account_id: AccountId,
        server_tick: u32,
        pos: MapPosition,
    },
    TimeSynchronized {
        server_tick: u32,
    },
    /// `0x0087` to the mover: its own walk started or was redirected.
    PlayerMoved {
        start: (u16, u16),
        dest: (u16, u16),
        move_start_tick: u32,
    },
    /// `0x09fd`: a walking unit, sent to observers. The encoder takes the cells from
    /// `start`/`dest`; `unit.pos` is ignored (`0x09fd` has no `pos_dir` field).
    UnitWalking {
        unit: UnitAppearance,
        start: (u16, u16),
        dest: (u16, u16),
        move_start_tick: u32,
    },
    /// `0x09ff`: an idle unit came into an observer's view.
    UnitStanding {
        unit: UnitAppearance,
    },
    /// `0x09fe`: a unit spawned in view.
    UnitSpawned {
        unit: UnitAppearance,
    },
    /// `0x0080`: `vanish_type` 0 out of sight, 1 died, 2 logged out, 3 teleport.
    UnitVanished {
        gid: u32,
        vanish_type: u8,
    },
    /// `0x0088`: a unit's walk stopped at (x, y).
    UnitStopped {
        gid: u32,
        x: u16,
        y: u16,
    },
    /// `0x009c`.
    DirectionChanged {
        gid: u32,
        head_dir: u16,
        body_dir: u8,
    },
    /// `0x0a30`: reply to `CZ_REQNAME` for a player.
    AckEntityName {
        gid: u32,
        name: String,
        party_name: String,
        guild_name: String,
        position_name: String,
        title_id: i32,
    },
    /// `0x0adf`: reply to `CZ_REQNAME` for an NPC.
    AckNpcName {
        gid: u32,
        name: Arc<str>,
    },
    /// `0x0091`.
    PlayerMapMove {
        dest_map: String,
        dest_x: u16,
        dest_y: u16,
    },
    EntityAction {
        source_id: u32,
        target_id: u32,
        action_type: u8,
        action_tick: u32,
    },
    ParameterChanged {
        var_id: u16,
        value: i32,
    },
    RestartAck {
        result: u8,
    },
    /// Reply to `RequestDisconnect`: 0 lets the client exit.
    DisconnectAck {
        result: u16,
    },
    /// `0x0b08`: the inventory list starts (inventory type 0, no name).
    InventoryStart,
    /// `0x0b09`: stackable inventory items.
    InventoryNormalItems {
        items: Vec<ItemEntry>,
    },
    /// `0x0b39`: non-stackable inventory items.
    InventoryEquipItems {
        items: Vec<ItemEntry>,
    },
    /// `0x0b0b`: the inventory list ends.
    InventoryEnd,
    /// `0x0a9b`: the equip-switch list. Always empty until equip switch exists.
    EquipSwitchList,
    /// `0x0b41` success: `item.amount` was added at `item.index`.
    ItemAdded {
        item: ItemEntry,
    },
    /// `0x0b41` failure: index 2, count 0, this `result` code, every other byte 0.
    ItemAddFailed {
        result: u8,
    },
    /// `0x00af`: `amount` left the inventory at wire `index` by a drop; 0 = refused.
    ItemThrown {
        index: u16,
        amount: u16,
    },
    /// `0x0add`: an item fell to the floor in view.
    FloorItemDropped {
        item: FloorItemAppearance,
    },
    /// `0x009d`: a floor item already on the ground came into view.
    FloorItemEntered {
        item: FloorItemAppearance,
    },
    /// `0x00a1`: a floor item vanished (picked up, expired or out of view).
    FloorItemVanished {
        id: u32,
    },
    /// `0x008d`: public chat from `gid`; `text` is `"<name> : <message>"`.
    PublicChat {
        gid: u32,
        text: Vec<u8>,
    },
    /// `0x008e`: a chat line to self (the speaker's echo or a system message).
    SelfChat {
        text: Vec<u8>,
    },
    /// `0x0201`: the friends list. Always empty until friends exist.
    FriendsList,
    /// `0x010f`: the skill list.
    SkillList {
        skills: Vec<SkillInfo>,
    },
    /// `0x010e`: one skill's level, SP cost, range and level-up button after a skill-up.
    SkillUpdate {
        id: u16,
        level: u16,
        sp: u16,
        range: u16,
        upgradable: bool,
    },
    /// `0x0441`: a skill left the list.
    SkillDelete {
        id: u16,
    },
    /// `0x0acb`: a 64-bit status var (exp and next exp).
    LongLongParameterChanged {
        var_id: u16,
        value: i64,
    },
    /// `0x00bd`: the whole status window, sent once at login.
    StatusWindow {
        status: StatusWindow,
    },
    /// `0x0141`: a stat's base value and its bonus (STR..LUK, POW..CRT).
    CoupleStatus {
        var_id: u32,
        base: i32,
        bonus: i32,
    },
    /// `0x013a`: attack range in cells.
    AttackRange {
        range: u16,
    },
    /// `0x00be`: the cost of raising one stat by one point.
    StatusCost {
        var_id: u16,
        value: u8,
    },
    /// `0x0b20`: one hotkey bank (`tab` 0 or 1). Always an empty bar until hotkeys are saved.
    ShortcutKeyList {
        tab: u8,
    },
    /// `0x02c9`: whether party invites are refused automatically.
    PartyConfig {
        deny_invites: bool,
    },
    /// `0x02da`: whether others may open this character's equipment window.
    EquipWindowConfig {
        show_equip: bool,
    },
    /// `0x02d9`: one `ZC_CONFIG` flag (`config_type` from rAthena's `e_config_type`).
    ConfigChanged {
        config_type: u32,
        value: u32,
    },
    /// `0x0b8d`: the reputation list. Always every type at 0 points until reputation exists.
    ReputationList,
    /// `0x0b1b`: reply to `BlockingPlayCancel`; the client unblocks input.
    LoadConfirmed,
    /// `0x01d7`: one look of unit `gid` changed (`look_type` from rAthena's `LOOK_*`).
    SpriteChanged {
        gid: u32,
        look_type: u8,
        value: u32,
        value2: u32,
    },
    /// `0x099b`: the current map's property type and flags.
    MapProperty {
        property: u16,
        flags: u32,
    },
    /// `0x09e7`: whether the mail icon shows unread mail.
    UnreadMail {
        unread: bool,
    },
    /// `0x0a24`: the achievement totals with no achievement attached.
    AchievementUpdate {
        summary: AchievementSummary,
    },
    /// `0x0ade`: the weight percentage at which natural regeneration stops.
    OverweightPercent {
        percent: u32,
    },
    /// `0x0999`: the reply to `EquipItem`. `result` 0 ok, 1 level too low, 2 failed; a failure
    /// carries location 0 and view 0. `view` is the item's `View` for a visible slot, else 0.
    EquipAck {
        index: u16,
        location: u32,
        view: u16,
        result: u8,
    },
    /// `0x099a`: an item came off (`location` = the mask it was worn on), or a take-off failed.
    UnequipAck {
        index: u16,
        location: u32,
        failed: bool,
    },
    /// `0x0bae`: the reply to `UnequipAll`.
    UnequipAllAck {
        failed: bool,
    },
    /// `0x013c`: the ammo at wire `index` is now worn.
    AmmoEquipped {
        index: u16,
    },
    /// `0x013b`: `ZC_ACTION_FAILURE`; type 3 is "ammunition equipped", 0 "equip ammo first".
    ActionFailure {
        failure_type: u16,
    },
    /// `0x0291`: show the client's message number `msg_id` (a `msgstringtable` line, 0-based).
    ClientMessage {
        msg_id: u16,
    },
    /// `0x0983`: status icon `efst` started on unit `gid`. Times are in ms.
    StatusIconStarted {
        efst: u16,
        gid: u32,
        state: u8,
        total_ms: u32,
        remain_ms: i32,
        values: [i32; 3],
    },
    /// `0x00bc`: the reply to `RaiseStat`. On success `value` is the new stat, capped at 255; a
    /// failure sends `ok: false` and 0.
    StatusChangeAck {
        var_id: u16,
        ok: bool,
        value: u8,
    },
    /// `0x019b`: play effect `effect` on unit `gid` (0 base level up, 1 job level up).
    SpecialEffect {
        gid: u32,
        effect: u32,
    },
    /// `0x08c8` for an attack swing. `src_speed` is the attacker's amotion, `dmg_speed` the
    /// target's dmotion (0 on a miss); `damage_type` 0 normal, 10 crit, 11 lucky dodge.
    Damage {
        source_id: u32,
        target_id: u32,
        tick: u32,
        src_speed: i32,
        dmg_speed: i32,
        damage: i32,
        div: u16,
        damage_type: u8,
        damage2: i32,
    },
    /// `0x0977` `ZC_HP_INFO`: a mob's HP bar, to its damage-log members.
    MobHpBar {
        gid: u32,
        hp: u32,
        max_hp: u32,
    },
    /// `0x0139` `ZC_ATTACK_FAILURE_FOR_DISTANCE`: the target at (`target_x`, `target_y`) is out
    /// of `range` from the attacker at (`x`, `y`). The client walks closer.
    AttackOutOfRange {
        target_id: u32,
        target_x: u16,
        target_y: u16,
        x: u16,
        y: u16,
        range: u16,
    },
    /// `0x0acc` `ZC_NOTIFY_EXP2`: `amount` of `var_id` exp (1 base, 2 job) was gained by the
    /// player `gid` (its own account id). Always the normal (non-quest) exp type.
    ExpGained {
        gid: u32,
        var_id: u16,
        amount: i64,
    },
    /// `0x0110` `ZC_ACK_TOUSESKILL`: skill `skill_id` failed with `cause` (84 = needs bullets).
    SkillFail {
        skill_id: u16,
        cause: u8,
    },
    /// `0x07fa` `ZC_DELETE_ITEM_FROM_BODY`: `amount` left wire `index`; `reason` 0 normal,
    /// 1 used up by a skill or an attack (ammo).
    ItemDeleted {
        index: u16,
        amount: u16,
        reason: u16,
    },
    /// `0x00b1` `ZC_LONGPAR_CHANGE`: a status var sent as a long (`SP_ZENY` 20 is the only user).
    LongParameterChanged {
        var_id: u16,
        value: i32,
    },
    /// `0x0b1a` `ZC_USESKILL_ACK`: `src` began casting `skill_id` on `dst`. `delay_ms` is the cast
    /// time (0 for an instant cast, which still sends this). `x`/`y` are 0 for a unit target.
    SkillCasting {
        src: u32,
        dst: u32,
        x: u16,
        y: u16,
        skill_id: u16,
        element: u32,
        delay_ms: u32,
    },
    /// `0x01de` `ZC_NOTIFY_SKILL2`: a damaging skill hit (or missed, with `damage` 0).
    /// `src_speed` is the caster's amotion, `dmg_speed` the target's dmotion (0 on a miss).
    SkillDamage {
        skill_id: u16,
        src: u32,
        dst: u32,
        tick: u32,
        src_speed: i32,
        dmg_speed: i32,
        damage: i32,
        level: i16,
        div: i16,
        damage_type: u8,
    },
    /// `0x09cb` `ZC_USE_SKILL2`: a non-damaging skill took effect. For heals `value` is the
    /// computed (unclamped) amount.
    SkillNoDamage {
        skill_id: u16,
        value: i32,
        target: u32,
        src: u32,
        ok: bool,
    },
    /// `0x01c8` `ZC_USE_ITEM_ACK2`: the use of the item at wire `index` by `gid`. `amount` is the
    /// amount left on success and 0 on a failure.
    ItemUseAck {
        index: u16,
        item_id: u32,
        gid: u32,
        amount: i16,
        ok: bool,
    },
}

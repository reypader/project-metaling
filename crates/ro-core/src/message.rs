//! Decoded client intents and server events: the vocabulary between packets, engine and servers.

use std::fmt;
use std::sync::Arc;

use crate::ids::{
    AccountId, CharId, CharServerInfo, CharacterSummary, MapPosition, SessionId, Sex,
};
use crate::item::{FloorItemAppearance, ItemEntry};

/// Scope definition for spatial or broadcast packets emitted by map-server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BroadcastScope {
    SelfOnly(CharId),
    /// Explicit recipients, resolved by the emitter at emission time (the engine resolves its
    /// AoI events this way, like rAthena's `clif_send(AREA)`).
    Characters(Vec<CharId>),
    /// Everyone whose character is in AoI range of `center`, minus `exclude`.
    AreaOfInterest {
        map_name: String,
        center: MapPosition,
        exclude: Option<CharId>,
    },
    MapWide {
        map_name: String,
    },
}

/// High-level intents decoded from incoming client packets (`CA_`, `CH_`, `CZ_`).
///
/// Variants contain only the fields physically present in the wire frame.
/// Session-level identity (`SessionId`, `AccountId`, `CharId`) is carried by [`SessionIntent`].
#[derive(Clone, PartialEq, Eq)]
pub enum ClientIntent {
    Login {
        username: String,
        password: String,
        client_version: u32,
        client_type: u8,
    },
    EnterCharServer {
        account_id: AccountId,
        login_id1: u32,
        login_id2: u32,
        client_version: u16,
        sex: Sex,
    },
    RequestCharacterList,
    SelectCharacter {
        slot: u8,
    },
    CreateCharacter {
        name: String,
        slot: u8,
        hair_color: u16,
        hair_style: u16,
        starting_job: u32,
        sex: Sex,
    },
    ReserveDeleteCharacter {
        char_id: CharId,
    },
    CancelDeleteCharacter {
        char_id: CharId,
    },
    CharPing {
        account_id: AccountId,
    },
    EnterMap {
        account_id: AccountId,
        char_id: CharId,
        login_id1: u32,
        client_tick: u32,
        sex: Sex,
    },
    NotifyActorInit,
    /// `CZ_BLOCKING_PLAY_CANCEL` (0x0447): the client finished loading and asks to unblock input.
    BlockingPlayCancel,
    RequestTime {
        client_tick: u32,
    },
    RequestMove {
        dest_x: u16,
        dest_y: u16,
    },
    ChangeDirection {
        head_dir: u16,
        body_dir: u8,
    },
    RequestAction {
        target_id: u32,
        action_type: u8,
    },
    RequestEntityName {
        entity_id: u32,
    },
    Restart {
        restart_type: u8,
    },
    /// "Exit to Windows" (`CZ_REQ_DISCONNECT`).
    RequestDisconnect {
        disconnect_type: u16,
    },
    /// `0x0362`: pick up the floor item with this object id.
    PickUpItem {
        floor_id: u32,
    },
    /// `0x0363`: drop `amount` of the item at wire `index` (slot + 2).
    DropItem {
        index: u16,
        amount: u16,
    },
    /// `0x00f3`: public chat. `text` is the raw `"<name> : <message>"` bytes, any trailing NUL
    /// removed. The map-server checks and strips the name prefix.
    Chat {
        text: Vec<u8>,
    },
}

impl fmt::Debug for ClientIntent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Login {
                username,
                password: _,
                client_version,
                client_type,
            } => f
                .debug_struct("Login")
                .field("username", username)
                .field("password", &"[REDACTED]")
                .field("client_version", client_version)
                .field("client_type", client_type)
                .finish(),
            Self::EnterCharServer {
                account_id,
                login_id1,
                login_id2,
                client_version,
                sex,
            } => f
                .debug_struct("EnterCharServer")
                .field("account_id", account_id)
                .field("login_id1", login_id1)
                .field("login_id2", login_id2)
                .field("client_version", client_version)
                .field("sex", sex)
                .finish(),
            Self::RequestCharacterList => write!(f, "RequestCharacterList"),
            Self::SelectCharacter { slot } => f
                .debug_struct("SelectCharacter")
                .field("slot", slot)
                .finish(),
            Self::CreateCharacter {
                name,
                slot,
                hair_color,
                hair_style,
                starting_job,
                sex,
            } => f
                .debug_struct("CreateCharacter")
                .field("name", name)
                .field("slot", slot)
                .field("hair_color", hair_color)
                .field("hair_style", hair_style)
                .field("starting_job", starting_job)
                .field("sex", sex)
                .finish(),
            Self::ReserveDeleteCharacter { char_id } => f
                .debug_struct("ReserveDeleteCharacter")
                .field("char_id", char_id)
                .finish(),
            Self::CancelDeleteCharacter { char_id } => f
                .debug_struct("CancelDeleteCharacter")
                .field("char_id", char_id)
                .finish(),
            Self::CharPing { account_id } => f
                .debug_struct("CharPing")
                .field("account_id", account_id)
                .finish(),
            Self::EnterMap {
                account_id,
                char_id,
                login_id1,
                client_tick,
                sex,
            } => f
                .debug_struct("EnterMap")
                .field("account_id", account_id)
                .field("char_id", char_id)
                .field("login_id1", login_id1)
                .field("client_tick", client_tick)
                .field("sex", sex)
                .finish(),
            Self::NotifyActorInit => write!(f, "NotifyActorInit"),
            Self::BlockingPlayCancel => write!(f, "BlockingPlayCancel"),
            Self::RequestTime { client_tick } => f
                .debug_struct("RequestTime")
                .field("client_tick", client_tick)
                .finish(),
            Self::RequestMove { dest_x, dest_y } => f
                .debug_struct("RequestMove")
                .field("dest_x", dest_x)
                .field("dest_y", dest_y)
                .finish(),
            Self::ChangeDirection { head_dir, body_dir } => f
                .debug_struct("ChangeDirection")
                .field("head_dir", head_dir)
                .field("body_dir", body_dir)
                .finish(),
            Self::RequestAction {
                target_id,
                action_type,
            } => f
                .debug_struct("RequestAction")
                .field("target_id", target_id)
                .field("action_type", action_type)
                .finish(),
            Self::RequestEntityName { entity_id } => f
                .debug_struct("RequestEntityName")
                .field("entity_id", entity_id)
                .finish(),
            Self::Restart { restart_type } => f
                .debug_struct("Restart")
                .field("restart_type", restart_type)
                .finish(),
            Self::RequestDisconnect { disconnect_type } => f
                .debug_struct("RequestDisconnect")
                .field("disconnect_type", disconnect_type)
                .finish(),
            Self::PickUpItem { floor_id } => f
                .debug_struct("PickUpItem")
                .field("floor_id", floor_id)
                .finish(),
            Self::DropItem { index, amount } => f
                .debug_struct("DropItem")
                .field("index", index)
                .field("amount", amount)
                .finish(),
            Self::Chat { text } => f
                .debug_struct("Chat")
                .field("text", &String::from_utf8_lossy(text))
                .finish(),
        }
    }
}

/// Session-authenticated envelope wrapping a wire `ClientIntent` for dispatch to server logic actors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionIntent {
    pub session_id: SessionId,
    pub account_id: Option<AccountId>,
    pub char_id: Option<CharId>,
    pub intent: ClientIntent,
}

impl SessionIntent {
    pub fn new(
        session_id: SessionId,
        account_id: Option<AccountId>,
        char_id: Option<CharId>,
        intent: ClientIntent,
    ) -> Self {
        Self {
            session_id,
            account_id,
            char_id,
            intent,
        }
    }
}

/// What a unit is to the client: the entry packets' `object_type` (rAthena `clif_bl_type`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitObjectType {
    Pc,
    Npc,
    /// An NPC whose sprite is a mob id.
    MobSpriteNpc,
    Mob,
}

/// One `0x010f` skill-list entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillInfo {
    pub id: u16,
    /// Target type (rAthena `skill_get_inf`); 0 for a passive.
    pub inf: i32,
    pub level: u16,
    pub sp: u16,
    pub range: u16,
    /// Aegis name, e.g. `NV_BASIC`; at most 23 bytes go on the wire.
    pub name: Arc<str>,
    /// The client shows the level-up button.
    pub upgradable: bool,
}

/// The `0x00bd` status window: allocated stats with the cost of raising each by one, and the
/// derived values. Stats above 255 are saturated on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StatusWindow {
    pub status_point: u16,
    /// STR, AGI, VIT, INT, DEX, LUK.
    pub stats: [u16; 6],
    /// Points to raise each stat by one; 0 at the cap.
    pub costs: [u8; 6],
    /// Status ATK (left side of ATK).
    pub atk: i16,
    /// Weapon and equipment ATK (right side).
    pub atk_bonus: i16,
    /// Weapon and equipment MATK (right side).
    pub matk_bonus: i16,
    /// Status MATK (left side).
    pub matk: i16,
    pub soft_def: i16,
    pub hard_def: i16,
    pub soft_mdef: i16,
    pub hard_mdef: i16,
    pub hit: i16,
    pub flee: i16,
    pub perfect_dodge: i16,
    pub critical: i16,
    /// rAthena `amotion`.
    pub aspd: i16,
}

/// Appearance snapshot shared by the unit entry packets (`0x09fd`/`0x09fe`/`0x09ff`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitAppearance {
    pub object_type: UnitObjectType,
    /// Wire id (`bl.id`): the account_id for players, the NPC/mob id for NPCs and mobs.
    pub gid: u32,
    /// Written to the `GID` field; `None` (non-players) writes 0.
    pub char_id: Option<CharId>,
    /// Shared, so entry events don't copy the string.
    pub name: Arc<str>,
    pub job: u16,
    pub sex: Sex,
    pub hair_style: u16,
    pub hair_color: u16,
    /// 0 straight, 1 clockwise, 2 counter-clockwise.
    pub head_dir: u16,
    /// Cell and body direction.
    pub pos: MapPosition,
    pub speed: u16,
    /// rAthena `dead_sit`: 0 standing, 1 dead, 2 sitting. Goes only to the `0x09ff` `state`
    /// byte (offset 68). It is NOT the wire `body_state` (`sc.opt1`, offset 15), which stays 0.
    pub dead_sit: u8,
    pub clevel: u16,
    /// rAthena `sc.option` (`effect_state`): `0x2` hidden, `0x4` cloaked NPCs.
    pub effect_state: i32,
}

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
}

/// High-level domain event tagged with its target broadcast scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopedServerEvent {
    pub scope: BroadcastScope,
    pub event: ServerEvent,
}

impl ScopedServerEvent {
    pub fn new(scope: BroadcastScope, event: ServerEvent) -> Self {
        Self { scope, event }
    }
}

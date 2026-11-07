//! Decoded client intents: the vocabulary from packets to engine and servers.

use std::fmt;

use crate::ids_dto::{AccountId, CharId, Sex};

/// High-level intents decoded from incoming client packets (`CA_`, `CH_`, `CZ_`).
///
/// Variants contain only the fields physically present in the wire frame.
/// Session-level identity (`SessionId`, `AccountId`, `CharId`) is carried by
/// [`SessionIntent`](crate::SessionIntent).
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
    /// `0x0118` `CZ_CANCEL_LOCKON`: stop the current attack. No reply.
    CancelAttack,
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
    /// `0x0998`: wear the item at wire `index` (slot + 2) on the `EQP_*` `position` mask.
    EquipItem {
        index: u16,
        position: u32,
    },
    /// `0x00ab`: take off the item at wire `index`.
    UnequipItem {
        index: u16,
    },
    /// `0x0bf5`: take off everything worn on the `EQP_*` `location` mask.
    UnequipAll {
        location: u32,
    },
    /// `0x00bb`: raise stat `var` (`SP_STR` 13 .. `SP_LUK` 18) by `amount` points.
    RaiseStat {
        var: u16,
        amount: u8,
    },
    /// `0x0b24`: raise trait `var` (`SP_POW` 219 .. `SP_CRT` 224) by `amount` trait points. The
    /// amount is signed on the wire; the engine drops a negative one.
    RaiseTrait {
        var: u16,
        amount: i16,
    },
    /// `0x0112`: raise skill `id` by one level with a skill point.
    UpgradeSkill {
        id: u16,
    },
    /// `0x0438`: cast skill `skill_id` at `level` on unit `target` (a wire gid).
    UseSkill {
        level: u16,
        skill_id: u16,
        target: u32,
    },
    /// `0x0439`: use the item at wire `index` (slot + 2). The packet's AID is not read.
    UseItem {
        index: u16,
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
            Self::CancelAttack => write!(f, "CancelAttack"),
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
            Self::EquipItem { index, position } => f
                .debug_struct("EquipItem")
                .field("index", index)
                .field("position", position)
                .finish(),
            Self::UnequipItem { index } => {
                f.debug_struct("UnequipItem").field("index", index).finish()
            }
            Self::UnequipAll { location } => f
                .debug_struct("UnequipAll")
                .field("location", location)
                .finish(),
            Self::RaiseStat { var, amount } => f
                .debug_struct("RaiseStat")
                .field("var", var)
                .field("amount", amount)
                .finish(),
            Self::RaiseTrait { var, amount } => f
                .debug_struct("RaiseTrait")
                .field("var", var)
                .field("amount", amount)
                .finish(),
            Self::UpgradeSkill { id } => f.debug_struct("UpgradeSkill").field("id", id).finish(),
            Self::UseSkill {
                level,
                skill_id,
                target,
            } => f
                .debug_struct("UseSkill")
                .field("level", level)
                .field("skill_id", skill_id)
                .field("target", target)
                .finish(),
            Self::UseItem { index } => f.debug_struct("UseItem").field("index", index).finish(),
        }
    }
}

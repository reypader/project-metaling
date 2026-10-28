//! Public chat and GM commands (spec `specs/items/spec.md` §2.7; catalog `packets/chat.md`,
//! `mechanics/atcommands.md`). Reply wording is our own. The level and job commands are in
//! `level_command.rs`.

use ro_core::{BroadcastScope, InventoryItem, ItemId, ScopedServerEvent, ServerEvent};
use ro_db::{ItemData, ItemType};

use crate::engine::MapInstance;
use crate::entity::Entity;

/// The group that may use every command (our own rule; no group database yet).
pub(crate) const GM_GROUP: i32 = 99;
/// Public chat reaches players within this Chebyshev distance (`AREA_SIZE - 5`).
const CHAT_RANGE: u16 = 9;
/// Longest message after the name prefix: rAthena's `CHAT_SIZE_MAX` (256) holds the message
/// plus the NUL it adds, and rejects `length + 1 > 255`.
const MAX_MESSAGE: usize = 254;
/// The separator between the speaker's name and the message.
const NAME_SEPARATOR: &[u8] = b" : ";

const MSG_ITEM_USAGE: &str = "Usage: @item <item name or id>[:<item>...] [amount]";
const MSG_ITEMS_ADDED: &str = "Items added to your inventory.";
const MSG_PET_EGG: &str = "Pet eggs can't be created with @item yet.";

impl MapInstance {
    /// `CZ_REQUEST_CHAT`. `text` is the full `"<name> : <message>"`; the map-server already closed
    /// sessions whose prefix doesn't match, so a mismatch here is just dropped.
    pub(crate) fn chat(&mut self, e: Entity, text: &[u8]) {
        let Some(info) = self.world.players.get(e) else {
            return;
        };
        let name = info.name.as_bytes();
        let group_id = info.group_id;
        let Some(message) = text
            .strip_prefix(name)
            .and_then(|rest| rest.strip_prefix(NAME_SEPARATOR))
        else {
            return;
        };
        if message.is_empty() || message.len() > MAX_MESSAGE {
            return;
        }
        if message.first() == Some(&b'@') && group_id == GM_GROUP {
            let line = String::from_utf8_lossy(message).into_owned();
            self.run_command(e, &line);
            return;
        }
        let gid = self.gid(e);
        let listeners = self.recipients_within(e, CHAT_RANGE, true);
        if !listeners.is_empty() {
            self.events.push(ScopedServerEvent::new(
                BroadcastScope::Characters(listeners),
                ServerEvent::PublicChat {
                    gid,
                    text: text.to_vec(),
                },
            ));
        }
        self.emit_self(
            e,
            ServerEvent::SelfChat {
                text: text.to_vec(),
            },
        );
    }

    /// A system line to `e` (`0x008e`).
    pub(crate) fn tell(&mut self, e: Entity, message: &str) {
        self.emit_self(
            e,
            ServerEvent::SelfChat {
                text: message.as_bytes().to_vec(),
            },
        );
    }

    /// Runs `@<command> <params>` for a GM.
    fn run_command(&mut self, e: Entity, line: &str) {
        let body = line.trim_start_matches('@');
        let (command, params) = match body.split_once(char::is_whitespace) {
            Some((command, params)) => (command, params.trim_start()),
            None => (body, ""),
        };
        match command.to_ascii_lowercase().as_str() {
            "item" => self.command_item(e, params),
            "baselevelup" | "blvl" | "lvup" | "baselevel" | "baselvl" | "baselvup"
            | "baselvlup" | "blevel" => self.command_base_level(e, params),
            "joblevelup" | "jlvl" | "jlevel" | "joblevel" | "joblvl" | "joblvlup" | "joblvup" => {
                self.command_job_level(e, params)
            }
            "jobchange" | "job" => self.command_job(e, params),
            _ => self.tell(e, &format!("Unknown command: @{command}")),
        }
    }

    /// `@item <name|id>[:<name|id>…] [amount]` (spec §2.7).
    fn command_item(&mut self, e: Entity, params: &str) {
        let Some((names, amount)) = parse_item_params(params) else {
            return self.tell(e, MSG_ITEM_USAGE);
        };
        let mut found: Vec<(ItemId, bool, bool)> = Vec::new();
        for token in names.split(':') {
            let Some(data) = self.find_item(token) else {
                return self.tell(e, &format!("No item matches '{token}'."));
            };
            found.push((
                data.id,
                data.is_stackable(),
                data.item_type == ItemType::PetEgg,
            ));
        }
        // `None` until an add is attempted (pet eggs are refused before any add).
        let mut last = None;
        for (id, stackable, pet_egg) in found {
            if pet_egg {
                self.tell(e, MSG_PET_EGG);
                continue;
            }
            let (per_add, times) = if stackable { (amount, 1) } else { (1, amount) };
            // Identified, everything else zero.
            let Ok(per_add) = u16::try_from(per_add) else {
                // Past u16 is past the 30000 stack cap.
                last = Some(Err(crate::inventory::ADD_OVER_AMOUNT));
                continue;
            };
            let item = InventoryItem::new(id, per_add);
            for _ in 0..times {
                let result = self.add_item(e, &item);
                last = Some(result);
                // Weight and slots only fill up: once one add fails, the rest would too.
                if result.is_err() {
                    break;
                }
            }
        }
        match last {
            None => {}
            Some(Ok(())) => self.tell(e, MSG_ITEMS_ADDED),
            Some(Err(result)) => self.emit_self(e, ServerEvent::ItemAddFailed { result }),
        }
    }

    /// rAthena's lookup order: lowercase aegis name, lowercase name, then numeric id.
    fn find_item(&self, token: &str) -> Option<&ItemData> {
        self.items
            .by_aegis_name(token)
            .or_else(|| self.items.by_name(token))
            .or_else(|| {
                token
                    .parse::<u32>()
                    .ok()
                    .and_then(|id| self.items.get(ItemId(id)))
            })
    }
}

/// Splits `@item` parameters into the name list and the amount. A quoted first argument may
/// contain spaces. An omitted, unreadable or non-positive amount is 1. `None` without a name.
fn parse_item_params(params: &str) -> Option<(&str, u32)> {
    let params = params.trim();
    let (names, rest) = if let Some(quoted) = params.strip_prefix('"') {
        let end = quoted.find('"')?;
        (&quoted[..end], &quoted[end + 1..])
    } else {
        match params.split_once(char::is_whitespace) {
            Some((names, rest)) => (names, rest),
            None => (params, ""),
        }
    };
    if names.is_empty() {
        return None;
    }
    let amount = rest
        .split_whitespace()
        .next()
        .and_then(|a| a.parse::<i64>().ok())
        .filter(|&a| a > 0)
        .map_or(1, |a| u32::try_from(a).unwrap_or(u32::MAX));
    Some((names, amount))
}

#[cfg(test)]
mod tests {
    use super::parse_item_params;

    #[test]
    fn item_params() {
        assert_eq!(parse_item_params("501 5"), Some(("501", 5)));
        assert_eq!(parse_item_params("knife"), Some(("knife", 1)));
        assert_eq!(
            parse_item_params("\"Red Potion\" 3"),
            Some(("Red Potion", 3))
        );
        assert_eq!(parse_item_params("\"Red Potion\""), Some(("Red Potion", 1)));
        assert_eq!(parse_item_params("a:b -4"), Some(("a:b", 1)));
        assert_eq!(parse_item_params("a 0"), Some(("a", 1)));
        assert_eq!(parse_item_params("a lots"), Some(("a", 1)));
        assert_eq!(parse_item_params(""), None);
        assert_eq!(parse_item_params("\"unclosed 3"), None);
        assert_eq!(parse_item_params("\"\" 3"), None);
    }
}

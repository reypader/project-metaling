//! A player's session on the map: the entry settings burst, chat and commands, and vitals.

mod chat;
mod entry_settings;
mod vitals;
mod vitals_commands;

pub(crate) use entry_settings::Arrival;

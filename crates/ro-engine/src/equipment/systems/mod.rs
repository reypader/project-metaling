//! The equip flows (spec `specs/equipment/spec.md` §2.2, §2.4-§2.6): wear an item, take one
//! off, unequip all. The rules they apply are in `equipment/rules.rs`; these files send the packets in
//! the captured order and keeps the worn-slot table, the looks cache and the status in step.

mod equip;
mod looks;
mod recheck;
mod unequip;

#[cfg(test)]
mod tests;

//! Warp portals: the trigger-cell index, destination resolution and the player warp (spec
//! `specs/warp-map-change/spec.md` §1, ENG-WARP-01..07).

mod cell_bits;
mod systems;
mod transfer_dto;

pub(crate) use cell_bits::CellBits;
pub use transfer_dto::{MapTransfer, PlaySnapshot};

#[cfg(test)]
mod tests;

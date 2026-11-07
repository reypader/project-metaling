//! A resolved sprite: its class id and whether it is a mob.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sprite {
    pub id: i16,
    pub is_mob: bool,
}

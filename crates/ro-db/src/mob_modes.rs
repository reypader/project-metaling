use serde::{Deserialize, Serialize};

use crate::MobMode;

/// A set of `MobMode`s, stored as rAthena's mode bits. YAML: a list of mode names.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct MobModes(u32);

impl MobModes {
    /// From raw bits; unknown bits are dropped.
    pub fn from_bits(bits: u32) -> Self {
        let known = MobMode::ALL.iter().fold(0, |acc, mode| acc | mode.bit());
        Self(bits & known)
    }

    pub fn bits(self) -> u32 {
        self.0
    }

    pub fn contains(self, mode: MobMode) -> bool {
        self.0 & mode.bit() != 0
    }

    pub fn set(&mut self, mode: MobMode, on: bool) {
        if on {
            self.0 |= mode.bit();
        } else {
            self.0 &= !mode.bit();
        }
    }

    /// The modes in bit order.
    pub fn iter(self) -> impl Iterator<Item = MobMode> {
        MobMode::ALL
            .into_iter()
            .filter(move |mode| self.contains(*mode))
    }
}

impl FromIterator<MobMode> for MobModes {
    fn from_iter<I: IntoIterator<Item = MobMode>>(iter: I) -> Self {
        let mut modes = Self::default();
        for mode in iter {
            modes.set(mode, true);
        }
        modes
    }
}

impl Serialize for MobModes {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let modes: Vec<MobMode> = self.iter().collect();
        serde_saphyr::FlowSeq(&modes).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for MobModes {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Vec::<MobMode>::deserialize(deserializer)?
            .into_iter()
            .collect())
    }
}

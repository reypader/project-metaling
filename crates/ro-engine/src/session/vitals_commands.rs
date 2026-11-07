//! `@zeny` and `@heal`.

use ro_core::MAX_ZENY;

use crate::ecs::Entity;
use crate::map_instance::MapInstance;

// Our wording.
const MSG_ZENY_USAGE: &str = "Usage: @zeny <amount> (a negative amount removes zeny)";

const MSG_ZENY_CHANGED: &str = "Your zeny changed.";

const MSG_ZENY_OUT_OF_RANGE: &str = "That would take your zeny out of range.";

const MSG_HEAL_USAGE: &str = "Usage: @heal [<hp> [<sp>]]";

const MSG_HEALED: &str = "HP and SP restored.";

const MSG_HEAL_CHANGED: &str = "HP and SP changed.";

impl MapInstance {
    /// `@zeny <n>` (spec §2.9): adds `n` (negative removes). A result outside `[0, MAX_ZENY]`
    /// is refused with a message only.
    pub(crate) fn command_zeny(&mut self, e: Entity, params: &str) {
        let amount = params
            .split_whitespace()
            .next()
            .and_then(|token| token.parse::<i64>().ok())
            .filter(|&n| n != 0);
        let Some(amount) = amount else {
            return self.tell(e, MSG_ZENY_USAGE);
        };
        let next = i64::from(self.zeny(e)).saturating_add(amount);
        let Some(next) = u32::try_from(next).ok().filter(|&z| z <= MAX_ZENY) else {
            return self.tell(e, MSG_ZENY_OUT_OF_RANGE);
        };
        self.set_zeny(e, next);
        self.tell(e, MSG_ZENY_CHANGED);
    }

    /// `@heal [hp [sp]]` (spec §2.9): no argument fills HP and SP; otherwise each is added
    /// (negative removes), HP kept at 1 or more (there is no player death yet).
    pub(crate) fn command_heal(&mut self, e: Entity, params: &str) {
        let mut tokens = params.split_whitespace();
        let mut next = || tokens.next().map(str::parse::<i64>);
        let (hp, sp) = match (next(), next()) {
            (None, _) => (None, None),
            (Some(Ok(hp)), None) => (Some(hp), Some(0)),
            (Some(Ok(hp)), Some(Ok(sp))) => (Some(hp), Some(sp)),
            _ => return self.tell(e, MSG_HEAL_USAGE),
        };
        match (hp, sp) {
            (Some(hp), Some(sp)) => {
                self.change_vitals(e, hp, sp, 1);
                self.tell(e, MSG_HEAL_CHANGED);
            }
            _ => {
                let Some(status) = self.world.players.get(e).map(|info| info.status) else {
                    return;
                };
                self.heal_player(e, status.max_hp, status.max_sp);
                self.tell(e, MSG_HEALED);
            }
        }
    }
}

//! Event emission: AoI recipient lookup and the emit helpers.

use ro_core::{BroadcastScope, CharId, ScopedServerEvent, ServerEvent};

use super::MapInstance;
use crate::ecs::{Entity, AREA_SIZE};

/// A `0x008e` self-chat line.
pub(crate) fn chat_line(text: &str) -> ServerEvent {
    ServerEvent::SelfChat {
        text: text.as_bytes().to_vec(),
    }
}

impl MapInstance {
    /// Characters that see `e` right now (Chebyshev 14, visible only), sorted, optionally
    /// without `e` itself. Resolved at emission like rAthena's `clif_send(AREA)`.
    fn aoi_recipients(&mut self, e: Entity, exclude_self: bool) -> Vec<CharId> {
        self.recipients_within(e, AREA_SIZE, exclude_self)
    }

    /// Characters within Chebyshev `radius` of `e` (visible only), sorted.
    pub(crate) fn recipients_within(
        &mut self,
        e: Entity,
        radius: u16,
        exclude_self: bool,
    ) -> Vec<CharId> {
        let exclude = exclude_self.then_some(e);
        self.recipients_around(self.cell(e), radius, exclude)
    }

    /// Characters within Chebyshev `radius` of a cell (visible only), sorted. For an area whose
    /// unit has left the grid, like a dead mob's.
    pub(crate) fn recipients_at(&mut self, cell: (u16, u16), radius: u16) -> Vec<CharId> {
        self.recipients_around(cell, radius, None)
    }

    fn recipients_around(
        &mut self,
        (x, y): (u16, u16),
        radius: u16,
        exclude: Option<Entity>,
    ) -> Vec<CharId> {
        self.world
            .grid
            .query_radius(x, y, radius, &self.world.positions, &mut self.recipients);
        let mut chars = Vec::with_capacity(self.recipients.len());
        for &r in &self.recipients {
            if exclude == Some(r) {
                continue;
            }
            if let Some(info) = self.world.players.get(r) {
                chars.push(info.char_id);
            }
        }
        chars.sort_unstable();
        chars
    }

    pub(crate) fn emit_self(&mut self, e: Entity, event: ServerEvent) {
        if let Some(char_id) = self.char_of(e) {
            self.events.push(ScopedServerEvent::new(
                BroadcastScope::SelfOnly(char_id),
                event,
            ));
        }
    }

    pub(crate) fn emit_aoi(&mut self, e: Entity, exclude_self: bool, event: ServerEvent) {
        let chars = self.aoi_recipients(e, exclude_self);
        self.emit_to(chars, event);
    }

    /// `event` to `chars` (sorted); nothing when there are none.
    pub(crate) fn emit_to(&mut self, chars: Vec<CharId>, event: ServerEvent) {
        if !chars.is_empty() {
            self.events.push(ScopedServerEvent::new(
                BroadcastScope::Characters(chars),
                event,
            ));
        }
    }

    pub(crate) fn emit_weight(&mut self, e: Entity, var_id: u16, value: u32) {
        self.emit_self(
            e,
            ServerEvent::ParameterChanged {
                var_id,
                value: i32::try_from(value).unwrap_or(i32::MAX),
            },
        );
    }

    pub(crate) fn emit_action(&mut self, e: Entity, action_type: u8) {
        let source_id = self.gid(e);
        self.emit_aoi(
            e,
            false,
            ServerEvent::EntityAction {
                source_id,
                target_id: 0,
                action_type,
                action_tick: 0,
            },
        );
    }
}

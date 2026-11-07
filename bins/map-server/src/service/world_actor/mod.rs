//! One map's actor: owns the map's engine and its sessions, and runs the tick loop.

mod join;
mod routing;
mod save;

use std::{
    collections::HashMap,
    mem,
    time::{Duration, Instant},
};

use bytes::{Bytes, BytesMut};
use ro_core::{AccountId, CharId, CharacterProgress, ScopedServerEvent, SessionId};
use ro_engine::{MapInstance, MapTransfer, PlaySnapshot};
use ro_packets::Phase0EventEncoder;
use tokio::sync::mpsc;
use tracing::{error, info};

use super::{MapData, MapServiceOptions, SessionControl, WorldCommand};
use crate::claim::Claim;

/// Engine tick period (BIN-SRV-01).
pub const TICK_INTERVAL: Duration = Duration::from_millis(20);

/// Most commands an actor handles before it lets a tick run.
const MAX_COMMANDS_PER_WAKE: usize = 256;

/// How often a map actor looks for due autosaves.
const AUTOSAVE_CHECK_MS: u64 = 1000;

/// Between the speaker's name and the message in `0x00f3` text.
const CHAT_SEPARATOR: &[u8] = b" : ";

/// A session that has joined this map.
struct Session {
    account_id: AccountId,
    char_id: CharId,
    /// The character's name: chat text must start with `"<name> : "`.
    name: String,
    /// Inventory revision of the last save (the engine starts every spawn at 0, which is what
    /// the database or the hand-off already holds).
    saved_revision: u64,
    /// The progress as last saved or loaded: autosave also writes when it changed.
    saved_progress: CharacterProgress,
    /// Skills revision of the last save or of the spawn: autosave also writes when it changed.
    saved_skills_revision: u32,
    /// When this session's next autosave is due (actor clock, ms).
    next_autosave_ms: u64,
    /// Released (with the last position set on it) when the session is dropped.
    claim: Claim,
    outbound_tx: mpsc::Sender<Bytes>,
    control_tx: mpsc::Sender<SessionControl>,
    /// Frames routed to this session during the current flush; sent as one `Bytes` at its end
    /// so a busy tick costs one queue slot, not one per packet.
    pending: BytesMut,
}

/// Long-lived actor owning one map's `MapInstance`, its sessions, and its 20 ms tick loop.
pub(super) struct MapWorldActor {
    map_name: String,
    engine: MapInstance,
    map_size: (u16, u16),
    /// Process clock origin shared by every map.
    started: Instant,
    sessions: HashMap<SessionId, Session>,
    by_char: HashMap<CharId, SessionId>,
    /// Senders of sessions that left the map but whose client hasn't closed yet (restart to
    /// char select, or exit): kept so the server doesn't close the socket first.
    detached: HashMap<SessionId, mpsc::Sender<Bytes>>,
    encoder: Phase0EventEncoder,
    /// Reused per-tick buffers.
    events: Vec<ScopedServerEvent>,
    aoi: Vec<CharId>,
    evict: Vec<SessionId>,
    transfers: Vec<MapTransfer>,
    /// Characters a flow asked to save now (`@job`), reused across ticks.
    save_requests: Vec<CharId>,
    autosave_interval_ms: u64,
    next_autosave_check_ms: u64,
}

/// Why a session leaves the map.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Departure {
    /// The client went away (or its queue failed): drop the sender.
    Gone,
    /// Back to character select, or exit to Windows: keep the socket open until the client
    /// closes it.
    ClientCloses,
}

impl MapWorldActor {
    pub(super) fn new(
        map_name: String,
        data: MapData,
        started: Instant,
        options: MapServiceOptions,
    ) -> Self {
        let map_size = (data.grid.width(), data.grid.height());
        let mut engine = MapInstance::new(map_name.clone(), data.grid);
        engine.set_registries(data.registries);
        // NPCs first: they take the low unit ids, like rAthena's load order.
        let npcs = engine.spawn_npcs(&data.npcs);
        let mobs = engine.spawn_mobs(&data.mobs);
        info!(
            "Map '{}': {} NPCs and {} mobs spawned",
            map_name, npcs, mobs
        );
        Self {
            map_name,
            engine,
            map_size,
            started,
            sessions: HashMap::new(),
            by_char: HashMap::new(),
            detached: HashMap::new(),
            encoder: Phase0EventEncoder,
            events: Vec::new(),
            aoi: Vec::new(),
            evict: Vec::new(),
            transfers: Vec::new(),
            save_requests: Vec::new(),
            autosave_interval_ms: u64::try_from(options.autosave_interval.as_millis())
                .unwrap_or(u64::MAX),
            next_autosave_check_ms: 0,
        }
    }

    fn now_ms(&self) -> u64 {
        u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    pub(super) async fn run(mut self, mut rx: mpsc::Receiver<WorldCommand>) {
        info!("MapWorldActor started for map '{}'", self.map_name);
        let mut tick_interval = tokio::time::interval(TICK_INTERVAL);
        tick_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        // Unbiased select plus a per-wake command cap: a flood of commands can't starve the tick.
        loop {
            tokio::select! {
                Some(cmd) = rx.recv() => {
                    self.handle_command(cmd);
                    for _ in 1..MAX_COMMANDS_PER_WAKE {
                        let Ok(next_cmd) = rx.try_recv() else { break };
                        self.handle_command(next_cmd);
                    }
                }
                _ = tick_interval.tick() => self.tick(),
                else => break,
            }
        }
        info!("MapWorldActor terminated for map '{}'", self.map_name);
    }

    pub(super) fn tick(&mut self) {
        let now = self.now_ms();
        let mut events = mem::take(&mut self.events);
        if let Err(err) = self.engine.step_tick(now, &mut events) {
            error!("Map '{}' tick failed: {}", self.map_name, err);
        }
        let mut transfers = mem::take(&mut self.transfers);
        self.engine.drain_transfers(&mut transfers);
        // The entity is gone, so the claim is the only record of where the character was. A
        // session evicted by this flush releases with this position. The inventory is saved
        // now (every map change saves it), before the flush can evict the session and skip
        // the hand-off.
        for transfer in &transfers {
            let char_id = transfer.spawn.char_id;
            if let Some(session) = self
                .by_char
                .get(&char_id)
                .and_then(|id| self.sessions.get_mut(id))
            {
                session.claim.set_last(&self.map_name, transfer.from_pos);
                session.claim.save_play(PlaySnapshot {
                    inventory: transfer.spawn.inventory.clone(),
                    looks: transfer.looks,
                    progress: transfer.spawn.progress(),
                    skills: transfer.spawn.skills.clone(),
                });
            }
        }
        // The warping session's last batch from this map goes out before the handoff.
        self.flush(&mut events);
        for transfer in transfers.drain(..) {
            self.hand_off(transfer);
        }
        self.transfers = transfers;
        self.save_requested();
        self.events = events;
        if now >= self.next_autosave_check_ms {
            self.next_autosave_check_ms = now + AUTOSAVE_CHECK_MS;
            self.autosave(now);
        }
    }
}

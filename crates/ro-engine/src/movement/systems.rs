//! Movement systems: path finding, walk start, step advance and AoI updates per step.

use std::mem;

use ro_core::{MapPosition, ServerEvent};
use smallvec::SmallVec;

use crate::ecs::{step_visibility, Entity, AREA_SIZE};
use crate::map_instance::MapInstance;
use crate::movement::{WalkingPath, MAX_WALKPATH, MAX_WALK_PATH_CLICK};
use crate::protocol_consts::{SITTING, VANISH_OUT_OF_SIGHT};

type PathCells = SmallVec<[(u16, u16); MAX_WALKPATH]>;

/// rAthena direction of a one-cell step (0 N, 1 NW, 2 W, 3 SW, 4 S, 5 SE, 6 E, 7 NE; y+ is north).
fn step_dir(from: (u16, u16), to: (u16, u16)) -> u8 {
    let dx = i32::from(to.0) - i32::from(from.0);
    let dy = i32::from(to.1) - i32::from(from.1);
    match (dx.signum(), dy.signum()) {
        (0, 1) => 0,
        (-1, 1) => 1,
        (-1, 0) => 2,
        (-1, -1) => 3,
        (0, -1) => 4,
        (1, -1) => 5,
        (1, 0) => 6,
        (1, 1) => 7,
        _ => 0,
    }
}

impl MapInstance {
    /// Final cell of `e`'s walk (a pending redirect target wins), if walking.
    pub(crate) fn walk_dest(&self, e: Entity) -> Option<(u16, u16)> {
        let path = self.world.walking_paths.get(e)?;
        Some(path.pending_target.unwrap_or_else(|| path.dest()))
    }

    pub(crate) fn query_into_after(&mut self, center: (u16, u16)) {
        self.world.grid.query_radius(
            center.0,
            center.1,
            AREA_SIZE,
            &self.world.positions,
            &mut self.aoi_after,
        );
    }

    /// Stops a walk in place: `ZC_STOPMOVE` to the AoI including the unit.
    pub(crate) fn stop_walking(&mut self, e: Entity) {
        if self.world.walking_paths.remove(e).is_some() {
            let (x, y) = self.cell(e);
            let gid = self.gid(e);
            self.emit_aoi(e, false, ServerEvent::UnitStopped { gid, x, y });
        }
    }

    fn find_path(
        &mut self,
        from: (u16, u16),
        to: (u16, u16),
        max_steps: usize,
    ) -> Option<PathCells> {
        self.scratch
            .find_path(&self.walkability, from, to, max_steps)
            .map(PathCells::from_slice)
    }

    /// Starts (or restarts) a walk along `cells` from `from` at `start_ms`, and announces it.
    fn start_walk(&mut self, e: Entity, cells: &[(u16, u16)], from: (u16, u16), start_ms: u64) {
        let speed = self.world.speed[e.index()];
        let path = WalkingPath::new(cells, start_ms, speed, from);
        let dest = path.dest();
        self.world.walking_paths.insert(e, path);
        self.world.head_dir[e.index()] = 0;
        let tick = start_ms as u32;
        self.emit_self(
            e,
            ServerEvent::PlayerMoved {
                start: from,
                dest,
                move_start_tick: tick,
            },
        );
        if let Some(unit) = self.appearance(e) {
            self.emit_aoi(
                e,
                true,
                ServerEvent::UnitWalking {
                    unit,
                    start: from,
                    dest,
                    move_start_tick: tick,
                },
            );
        }
    }

    pub(crate) fn plan_moves(&mut self, now_ms: u64) {
        let mut moves = mem::take(&mut self.moves);
        for e in moves.drain(..) {
            if self.world.entities.is_alive(e) {
                self.plan_move(e, now_ms);
            }
        }
        self.moves = moves;
    }

    /// Turns the entity's pending move request (if any) into a walk or a redirect. Normally run
    /// by `plan_moves`; an attack request runs it early so a move queued before it in the same
    /// tick happens first, as in rAthena.
    pub(crate) fn plan_move(&mut self, e: Entity, now_ms: u64) {
        let Some(dest) = self.world.pending_move[e.index()].take() else {
            return;
        };
        if self.world.dead_sit[e.index()] == SITTING {
            return;
        }
        let from = self.cell(e);
        // rAthena checks `max_walk_path` after a successful search; same outcome.
        let Some(cells) = self.find_path(from, dest, MAX_WALK_PATH_CLICK) else {
            return;
        };
        // rAthena `unit_walktoxy` ends the attack (and its step action).
        self.drop_attack(e);
        if let Some(path) = self.world.walking_paths.get_mut(e) {
            // Redirect: validated now, applied when the in-flight step ends.
            path.pending_target = Some(dest);
        } else if cells.is_empty() {
            self.emit_self(
                e,
                ServerEvent::PlayerMoved {
                    start: from,
                    dest: from,
                    move_start_tick: now_ms as u32,
                },
            );
        } else {
            self.start_walk(e, &cells, from, now_ms);
        }
    }

    pub(crate) fn advance_walkers(&mut self, now_ms: u64) {
        self.walkers.clear();
        self.walkers
            .extend_from_slice(self.world.walking_paths.entities());
        for k in 0..self.walkers.len() {
            let e = self.walkers[k];
            while let Some(path) = self.world.walking_paths.get(e) {
                if path.next_step_at > now_ms {
                    break;
                }
                let step_end = path.next_step_at;
                let Some(next) = path.next_cell() else {
                    self.world.walking_paths.remove(e);
                    break;
                };
                let old = self.cell(e);
                self.step(e, next, step_end);
                let diagonal = old.0 != next.0 && old.1 != next.1;
                self.arm_step_action(e, diagonal, step_end);
                if self.world.players.contains(e) {
                    // A warp clears the walk (or despawns `e`), which ends this loop.
                    self.touch_warps(e);
                }
                let Some(path) = self.world.walking_paths.get_mut(e) else {
                    break;
                };
                if let Some(target) = path.pending_target.take() {
                    self.repath(e, target, step_end);
                } else if path.next_cell().is_none() {
                    self.world.walking_paths.remove(e);
                }
            }
        }
    }

    /// Completes one step into `next` (ending at `step_end`) and emits visibility changes.
    fn step(&mut self, e: Entity, next: (u16, u16), step_end: u64) {
        let i = e.index();
        let old = self.cell(e);
        let visible = self.world.in_grid[i];
        if visible {
            self.world.grid.query_radius(
                old.0,
                old.1,
                AREA_SIZE + 1,
                &self.world.positions,
                &mut self.step_candidates,
            );
            self.world.grid.move_entity(e, old, next);
        }
        self.world.positions[i] = MapPosition {
            x: next.0,
            y: next.1,
            dir: step_dir(old, next),
        };
        let speed = self.world.speed[i];
        if let Some(path) = self.world.walking_paths.get_mut(e) {
            path.advance(speed, next);
        }
        if !visible {
            return;
        }
        // A unit whose walk ends on this step is shown standing.
        let finished = self
            .world
            .walking_paths
            .get(e)
            .is_some_and(|p| p.next_cell().is_none() && p.pending_target.is_none());
        step_visibility(
            e,
            old,
            next,
            &self.step_candidates,
            &self.world.positions,
            &mut self.left,
            &mut self.entered,
        );
        let gid = self.gid(e);
        for k in 0..self.left.len() {
            let other = self.left[k];
            let other_gid = self.gid(other);
            let gone = if self.is_floor_item(other) {
                ServerEvent::FloorItemVanished { id: other_gid }
            } else {
                ServerEvent::UnitVanished {
                    gid: other_gid,
                    vanish_type: VANISH_OUT_OF_SIGHT,
                }
            };
            self.emit_self(e, gone);
            // NPCs see nothing; skip building events for them.
            if !self.world.players.contains(other) {
                continue;
            }
            self.emit_self(
                other,
                ServerEvent::UnitVanished {
                    gid,
                    vanish_type: VANISH_OUT_OF_SIGHT,
                },
            );
        }
        let tick = step_end as u32;
        // Only players see anything: skip building entry events for a mob mover.
        let mover_sees = self.world.players.contains(e);
        for k in 0..self.entered.len() {
            let other = self.entered[k];
            if mover_sees {
                if let Some(event) = self.entry_event(other, tick) {
                    self.emit_self(e, event);
                }
            }
            if !self.world.players.contains(other) {
                continue;
            }
            let about_mover = if finished {
                self.appearance(e)
                    .map(|unit| ServerEvent::UnitStanding { unit })
            } else {
                self.entry_event(e, tick)
            };
            if let Some(event) = about_mover {
                self.emit_self(other, event);
            }
        }
    }

    /// Applies a pending redirect from the cell just reached. No `max_walk_path` check here.
    fn repath(&mut self, e: Entity, target: (u16, u16), step_end: u64) {
        let from = self.cell(e);
        match self.find_path(from, target, MAX_WALKPATH) {
            None => self.stop_walking(e),
            Some(cells) if cells.is_empty() => {
                // Redirected onto the reached cell: the mover resyncs, observers (who were told
                // about the old destination) get a stop.
                self.world.walking_paths.remove(e);
                self.emit_self(
                    e,
                    ServerEvent::PlayerMoved {
                        start: from,
                        dest: from,
                        move_start_tick: step_end as u32,
                    },
                );
                let gid = self.gid(e);
                self.emit_aoi(
                    e,
                    true,
                    ServerEvent::UnitStopped {
                        gid,
                        x: from.0,
                        y: from.1,
                    },
                );
            }
            Some(cells) => self.start_walk(e, &cells, from, step_end),
        }
    }
}

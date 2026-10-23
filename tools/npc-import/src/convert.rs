//! Turns header records into per-map `NpcSpawn` lists: names, sprites, duplicates, warps
//! (spec TOOL-NPC-03..07), and spawn lines into per-map `MobSpawn` lists (spec TOOL-MOB-01..05).

use std::collections::{BTreeMap, HashMap};

use ro_db::{MapMobSpawns, MapNpcs, MobId, MobSpawn, NpcKind, NpcSpawn, NpcState, WarpDest};

use crate::header::{self, Header, HeaderError, Location, MobHeader, Place, SkipReason};
use crate::lex;
use crate::sprite::{SpriteTable, INVISIBLE, WARPNPC};

/// Counts of what was converted and skipped, plus messages worth a look.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Report {
    pub files: usize,
    /// Converted positioned NPCs by header keyword (`warp`, `warp2`, `script`, `duplicate`, `shop`, ...).
    pub converted: BTreeMap<String, usize>,
    /// Records not converted, by reason.
    pub skipped: BTreeMap<String, usize>,
    /// Problems rAthena would warn about (bad header, unknown sprite, renamed duplicate name, ...).
    pub warnings: Vec<String>,
    /// Files rAthena would stop reading early, with the reason.
    pub file_errors: Vec<String>,
    /// Spawn lines (`monster` / `boss_monster`).
    pub mobs: MobReport,
}

/// What happened to the spawn lines.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MobReport {
    /// Converted lines by keyword (`monster`, `boss_monster`).
    pub converted: BTreeMap<String, usize>,
    /// Lines not converted, by reason.
    pub skipped: BTreeMap<String, usize>,
    /// Sum of `Amount` over converted lines.
    pub total_amount: u64,
    /// The map with the most mobs, and its total.
    pub largest_map: Option<(String, u64)>,
    /// Lines converted with a change (cut name, dropped size/ai, ...) or skipped.
    pub warnings: Vec<String>,
}

/// Everything one import produced, keyed by map name.
#[derive(Debug, Default)]
pub struct Imported {
    pub npcs: BTreeMap<String, MapNpcs>,
    pub mobs: BTreeMap<String, MapMobSpawns>,
    pub report: Report,
}

/// rAthena `MAX_LEVEL`.
const MAX_LEVEL: i64 = 275;
/// Largest spawn amount rAthena accepts.
const MAX_AMOUNT: i64 = 1000;
/// Delay used when the line gives none.
const DEFAULT_DELAY1: u32 = 5000;
/// rAthena rejects delays above this.
const MAX_DELAY: i64 = 0x0fff_ffff;
/// An event label shorter than this is ignored (the data uses `0` / `1` as "none").
const MIN_EVENT_LEN: usize = 4;

impl Report {
    fn count(map: &mut BTreeMap<String, usize>, key: &str) {
        *map.entry(key.to_string()).or_default() += 1;
    }
}

/// What a later `duplicate(...)` copies from its source.
#[derive(Debug, Clone)]
enum SourceKind {
    Script,
    Shop,
    Warp(WarpDest),
}

#[derive(Debug, Clone)]
struct Source {
    kind: SourceKind,
    state: NpcState,
}

/// Feed files in manifest order, then call `finish`.
pub struct Importer {
    sprites: SpriteTable,
    /// Every NPC's unique name (floating ones too), for duplicates and collisions.
    by_unique: HashMap<String, Source>,
    maps: BTreeMap<String, Vec<NpcSpawn>>,
    mobs: BTreeMap<String, Vec<MobSpawn>>,
    report: Report,
}

impl Importer {
    pub fn new(sprites: SpriteTable) -> Self {
        Self {
            sprites,
            by_unique: HashMap::new(),
            maps: BTreeMap::new(),
            mobs: BTreeMap::new(),
            report: Report::default(),
        }
    }

    /// Converts one NPC source file. `path` is its manifest path (used in `Source`).
    pub fn add_file(&mut self, path: &str, bytes: &[u8]) {
        self.report.files += 1;
        let (records, error) = lex::records(bytes);
        for rec in &records {
            let at = format!("{path}:{}", rec.line);
            match header::parse(rec) {
                Ok(h) => self.add_header(h, rec, &at),
                Err(HeaderError(msg)) => {
                    Report::count(&mut self.report.skipped, "bad header");
                    self.report.warnings.push(format!("{at}: {msg}"));
                }
            }
        }
        if let Some(err) = error {
            self.report.file_errors.push(format!("{path}: {err}"));
        }
    }

    pub fn finish(self) -> Imported {
        let npcs = self
            .maps
            .into_iter()
            .map(|(map, npcs)| (map, MapNpcs { npcs }))
            .collect();
        let mut report = self.report;
        let mut mobs = BTreeMap::new();
        for (map, spawns) in self.mobs {
            let total: u64 = spawns.iter().map(|s| u64::from(s.amount)).sum();
            report.mobs.total_amount += total;
            if report
                .mobs
                .largest_map
                .as_ref()
                .is_none_or(|(_, n)| total > *n)
            {
                report.mobs.largest_map = Some((map.clone(), total));
            }
            mobs.insert(map, MapMobSpawns { spawns });
        }
        Imported { npcs, mobs, report }
    }

    /// Validates a spawn line and resolves its mob and name (spec TOOL-MOB-01..04).
    fn add_mob(&mut self, h: MobHeader, at: &str) {
        let keyword = if h.boss { "boss_monster" } else { "monster" };
        let mobs = &mut self.report.mobs;
        let mut skip = |reason: &str, detail: String| {
            Report::count(&mut mobs.skipped, reason);
            mobs.warnings.push(format!("{at}: {detail}"));
        };
        let (Ok(x), Ok(y)) = (u16::try_from(h.x), u16::try_from(h.y)) else {
            return skip("negative coordinates", "negative coordinates".to_string());
        };
        let level = match h.level {
            None => None,
            Some(lv) if lv < 0 => None,
            Some(lv @ 1..=MAX_LEVEL) => u16::try_from(lv).ok(),
            Some(lv) => return skip("bad level", format!("level {lv} outside 1..={MAX_LEVEL}")),
        };
        let amount = match u16::try_from(h.amount) {
            Ok(n) if (1..=MAX_AMOUNT).contains(&h.amount) => n,
            _ => {
                return skip(
                    "bad amount",
                    format!("amount {} outside 1..={MAX_AMOUNT}", h.amount),
                )
            }
        };
        let Some(mob_id) = self.sprites.resolve_mob(&h.mob) else {
            return skip("unknown mob", format!("unknown mob '{}'", h.mob));
        };
        let delay = |d: Option<i64>, default: u32| match d {
            None => Some(default),
            Some(d) if (0..=MAX_DELAY).contains(&d) => u32::try_from(d).ok(),
            Some(_) => None,
        };
        let (Some(delay1), Some(delay2)) = (delay(h.delay1, DEFAULT_DELAY1), delay(h.delay2, 0))
        else {
            return skip(
                "bad delay",
                format!("delay above {MAX_DELAY:#x} or negative"),
            );
        };

        let mut warn = |msg: String| mobs.warnings.push(format!("{at}: {msg}"));
        if h.span.0 < 0 || h.span.1 < 0 {
            warn(format!("negative span {:?} set to 0", h.span));
        }
        let span = (clamp_u16(h.span.0), clamp_u16(h.span.1));
        let name = match h.name.as_str() {
            "--en--" => self.sprites.mob_name(mob_id),
            "--ja--" => self.sprites.mob_japanese_name(mob_id),
            _ => None,
        };
        let name = header::cut_name(name.unwrap_or(&h.name), header::MOB_NAME_MAX);
        if h.name_truncated || name.truncated {
            warn(format!("name cut to {} bytes", header::MOB_NAME_MAX));
        }
        if matches!(h.size, Some(1..=2)) || h.ai.is_some_and(|ai| ai > 0) {
            warn(format!(
                "size {:?} / ai {:?} not supported, dropped",
                h.size, h.ai
            ));
        }
        let event = h.event.and_then(|e| {
            let e = e.strip_prefix('"').unwrap_or(&e);
            let e = e.strip_suffix('"').unwrap_or(e);
            (e.len() >= MIN_EVENT_LEN).then(|| e.to_string())
        });

        Report::count(&mut mobs.converted, keyword);
        self.mobs.entry(h.map).or_default().push(MobSpawn {
            name: name.text,
            mob_id: MobId(mob_id),
            x,
            y,
            span,
            amount,
            delay1,
            delay2,
            level,
            boss: h.boss,
            event,
            source: at.to_string(),
        });
    }

    fn add_header(&mut self, h: Header, rec: &lex::Record, at: &str) {
        let keyword = rec.w2.to_ascii_lowercase();
        match h {
            Header::Skip(reason) => Report::count(&mut self.report.skipped, skip_label(reason)),
            Header::Mob(mob) => self.add_mob(mob, at),
            Header::Warp {
                loc,
                warp2,
                state,
                span,
                dest_map,
                dest,
            } => {
                let dest = WarpDest {
                    map: dest_map,
                    x: clamp_u16(dest.0),
                    y: clamp_u16(dest.1),
                    trigger_while_hidden: warp2,
                };
                let label = if warp2 { "warp2" } else { "warp" };
                let source = Source {
                    kind: SourceKind::Warp(dest.clone()),
                    state,
                };
                let trigger = non_negative(span);
                self.place(
                    rec,
                    at,
                    Place::Map(loc),
                    source,
                    SpriteSpec::Warp,
                    trigger,
                    None,
                    label,
                );
            }
            Header::Script {
                place,
                state,
                sprite,
                trigger,
            } => {
                let sprite = SpriteSpec::Token(sprite);
                let source = Source {
                    kind: SourceKind::Script,
                    state,
                };
                let trigger = trigger.and_then(non_negative);
                self.place(rec, at, place, source, sprite, trigger, None, "script");
            }
            Header::Shop { place, sprite } => {
                let sprite = SpriteSpec::Token(sprite);
                let source = Source {
                    kind: SourceKind::Shop,
                    state: NpcState::Normal,
                };
                self.place(rec, at, place, source, sprite, None, None, &keyword);
            }
            Header::Duplicate { place, src, w4 } => {
                let Some(source) = self.by_unique.get(&src).cloned() else {
                    Report::count(&mut self.report.skipped, "duplicate of unknown source");
                    self.report
                        .warnings
                        .push(format!("{at}: duplicate source '{src}' not defined yet"));
                    return;
                };
                let (sprite, trigger) = match &source.kind {
                    SourceKind::Warp(_) => {
                        let Some(span) = header::leading_pair(&w4) else {
                            Report::count(&mut self.report.skipped, "bad header");
                            self.report
                                .warnings
                                .push(format!("{at}: warp duplicate needs 'xs,ys'"));
                            return;
                        };
                        (SpriteSpec::Warp, non_negative(span))
                    }
                    SourceKind::Script => (
                        SpriteSpec::Token(header::sprite_token(&w4).to_string()),
                        header::after_first_comma_pair(&w4).and_then(non_negative),
                    ),
                    SourceKind::Shop => (
                        SpriteSpec::Token(header::sprite_token(&w4).to_string()),
                        None,
                    ),
                };
                self.place(
                    rec,
                    at,
                    place,
                    source,
                    sprite,
                    trigger,
                    Some(src),
                    "duplicate",
                );
            }
        }
    }

    fn sprite(&mut self, token: &str, at: &str) -> ResolvedSprite {
        match self.sprites.resolve(token) {
            Some(s) => ResolvedSprite {
                id: s.id,
                is_mob: s.is_mob,
            },
            None => {
                self.report
                    .warnings
                    .push(format!("{at}: unknown sprite '{token}', using {INVISIBLE}"));
                ResolvedSprite {
                    id: INVISIBLE,
                    is_mob: false,
                }
            }
        }
    }

    /// Registers the unique name, and records the NPC if it stands on a map.
    #[allow(clippy::too_many_arguments)]
    fn place(
        &mut self,
        rec: &lex::Record,
        at: &str,
        place: Place,
        source: Source,
        sprite: SpriteSpec,
        trigger: Option<(u16, u16)>,
        duplicate_of: Option<String>,
        label: &str,
    ) {
        let parsed = header::parse_name(&rec.w3);
        if parsed.truncated {
            self.report.warnings.push(format!(
                "{at}: name longer than {} bytes truncated",
                header::NPC_NAME_LENGTH
            ));
        }
        let loc = match &place {
            Place::Map(loc) => Some(loc),
            Place::Floating => None,
        };
        // rAthena rejects out-of-map coordinates before registering the NPC, so a later
        // duplicate can't name it.
        let cell = match loc.map(|l| (u16::try_from(l.x), u16::try_from(l.y))) {
            Some((Ok(x), Ok(y))) => Some((x, y)),
            Some(_) => {
                Report::count(&mut self.report.skipped, "negative coordinates");
                self.report
                    .warnings
                    .push(format!("{at}: negative coordinates"));
                return;
            }
            None => None,
        };
        let unique = self.unique_name(parsed.unique, loc, at);
        self.by_unique.insert(unique.clone(), source.clone());

        let (Some(loc), Some((x, y))) = (loc, cell) else {
            Report::count(&mut self.report.skipped, "floating");
            return;
        };
        let name = parsed.display;
        // Floating NPCs never resolve their sprite (rAthena forces FAKENPC), so only here.
        let sprite = match sprite {
            SpriteSpec::Warp => WARPNPC_SPRITE,
            SpriteSpec::Token(token) => self.sprite(&token, at),
        };
        let kind = match source.kind {
            SourceKind::Script => NpcKind::Script,
            SourceKind::Shop => NpcKind::Shop,
            SourceKind::Warp(dest) => NpcKind::Warp(dest),
        };
        let npc = NpcSpawn {
            name,
            unique_name: unique,
            x,
            y,
            // rAthena keeps `(uint8)dir`.
            dir: loc.dir as u8,
            sprite: sprite.id,
            sprite_is_mob: sprite.is_mob,
            state: source.state,
            trigger,
            duplicate_of,
            kind,
            source: at.to_string(),
        };
        self.maps.entry(loc.map.clone()).or_default().push(npc);
        Report::count(&mut self.report.converted, label);
    }

    /// rAthena gives a missing or colliding unique name `<n>_<map>_<x>_<y>`.
    fn unique_name(&mut self, wanted: Option<String>, loc: Option<&Location>, at: &str) -> String {
        if let Some(name) = &wanted {
            if !self.by_unique.contains_key(name) {
                return name.clone();
            }
        }
        let (map, x, y) = loc.map_or(("-", 0, 0), |l| (l.map.as_str(), l.x, l.y));
        let mut n = 0;
        loop {
            let generated = format!("{n}_{map}_{x}_{y}");
            if !self.by_unique.contains_key(&generated) {
                if let Some(name) = wanted {
                    self.report.warnings.push(format!(
                        "{at}: unique name '{name}' taken, using '{generated}'"
                    ));
                }
                return generated;
            }
            n += 1;
        }
    }
}

/// A sprite to resolve once the NPC is known to stand on a map.
enum SpriteSpec {
    Warp,
    Token(String),
}

#[derive(Debug, Clone, Copy)]
struct ResolvedSprite {
    id: i16,
    is_mob: bool,
}

const WARPNPC_SPRITE: ResolvedSprite = ResolvedSprite {
    id: WARPNPC,
    is_mob: false,
};

fn non_negative((x, y): (i16, i16)) -> Option<(u16, u16)> {
    Some((u16::try_from(x).ok()?, u16::try_from(y).ok()?))
}

fn clamp_u16(v: i16) -> u16 {
    u16::try_from(v).unwrap_or(0)
}

fn skip_label(reason: SkipReason) -> &'static str {
    match reason {
        SkipReason::Function => "function",
        SkipReason::Mapflag => "mapflag",
        SkipReason::UnknownKeyword => "unknown keyword",
    }
}

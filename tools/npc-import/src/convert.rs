//! Turns header records into per-map `NpcSpawn` lists: names, sprites, duplicates, warps
//! (spec TOOL-NPC-03..07).

use std::collections::{BTreeMap, HashMap};

use ro_db::{MapNpcs, NpcKind, NpcSpawn, NpcState, WarpDest};

use crate::header::{self, Header, HeaderError, Location, Place, SkipReason};
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
}

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
    report: Report,
}

impl Importer {
    pub fn new(sprites: SpriteTable) -> Self {
        Self {
            sprites,
            by_unique: HashMap::new(),
            maps: BTreeMap::new(),
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

    pub fn finish(self) -> (BTreeMap<String, MapNpcs>, Report) {
        let maps = self
            .maps
            .into_iter()
            .map(|(map, npcs)| (map, MapNpcs { npcs }))
            .collect();
        (maps, self.report)
    }

    fn add_header(&mut self, h: Header, rec: &lex::Record, at: &str) {
        let keyword = rec.w2.to_ascii_lowercase();
        match h {
            Header::Skip(reason) => Report::count(&mut self.report.skipped, skip_label(reason)),
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
        let (name, unique) = header::parse_name(&rec.w3);
        let loc = match &place {
            Place::Map(loc) => Some(loc),
            Place::Floating => None,
        };
        let unique = self.unique_name(unique, loc, at);
        self.by_unique.insert(unique.clone(), source.clone());

        let Some(loc) = loc else {
            Report::count(&mut self.report.skipped, "floating");
            return;
        };
        let (Ok(x), Ok(y)) = (u16::try_from(loc.x), u16::try_from(loc.y)) else {
            Report::count(&mut self.report.skipped, "negative coordinates");
            self.report
                .warnings
                .push(format!("{at}: negative coordinates {},{}", loc.x, loc.y));
            return;
        };
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
        SkipReason::Monster => "monster",
        SkipReason::BossMonster => "boss_monster",
        SkipReason::Mapflag => "mapflag",
        SkipReason::UnknownKeyword => "unknown keyword",
    }
}

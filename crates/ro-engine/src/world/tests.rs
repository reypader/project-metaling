use ro_core::{AccountId, CharId, CharacterSpawn, MapPosition, Sex};

use super::*;

fn spawn(account: u32, char_id: u32, x: u16, y: u16) -> CharacterSpawn {
    CharacterSpawn {
        account_id: AccountId(account),
        char_id: CharId(char_id),
        name: format!("c{char_id}"),
        sex: Sex::Male,
        job_class: 0,
        base_level: 1,
        hair_style: 1,
        hair_color: 1,
        pos: MapPosition { x, y, dir: 4 },
        speed: 150,
        status: Default::default(),
        group_id: 0,
        inventory: Default::default(),
        skills: Vec::new(),
        hidden_skills: Vec::new(),
        change_level: [0; 3],
    }
}

#[test]
fn eng_02_dense_columns_indexed_by_entity_index() {
    let mut world = World::new(200, 200, 16);
    let a = world
        .spawn_player(&spawn(2000001, 150001, 100, 100))
        .expect("spawn a");
    let b = world
        .spawn_player(&spawn(2000002, 150002, 103, 100))
        .expect("spawn b");
    assert_eq!(
        world.positions[a.index()],
        MapPosition {
            x: 100,
            y: 100,
            dir: 4
        }
    );
    assert_eq!(world.positions[b.index()].x, 103);
    assert_eq!(world.kinds[a.index()], EntityKind::Player);
    assert_eq!(world.speed[b.index()], 150);
    assert_eq!(world.dead_sit[a.index()], 0);
    assert_eq!(world.entity_by_char(CharId(150002)), Some(b));
    assert_eq!(world.entity_by_gid(2000001), Some(a));
    assert_eq!(world.players.get(a).map(|p| &*p.name), Some("c150001"));
    assert!(
        !world.in_grid[a.index()],
        "spawned entities are not visible yet"
    );
}

#[test]
fn eng_02_despawn_clears_lookups_and_stale_handles() {
    let mut world = World::new(200, 200, 16);
    let a = world
        .spawn_player(&spawn(2000001, 150001, 100, 100))
        .expect("spawn");
    world.show(a);
    assert!(world.in_grid[a.index()]);
    world.despawn(a).expect("despawn live entity");
    assert_eq!(world.kinds[a.index()], EntityKind::Free);
    assert_eq!(world.entity_by_char(CharId(150001)), None);
    assert_eq!(world.entity_by_gid(2000001), None);
    assert!(world.players.get(a).is_none());
    assert!(!world.in_grid[a.index()]);
    assert!(world.despawn(a).is_err(), "stale handle");
    let b = world
        .spawn_player(&spawn(2000002, 150002, 1, 1))
        .expect("reuse slot");
    assert_eq!(b.index(), a.index());
    assert!(world.players.get(a).is_none(), "old handle sees nothing");
}

#[test]
fn duplicate_ids_are_rejected_and_never_clobber_lookups() {
    let mut world = World::new(200, 200, 16);
    let a = world
        .spawn_player(&spawn(2000001, 150001, 1, 1))
        .expect("spawn");
    assert!(
        world.spawn_player(&spawn(2000001, 150009, 1, 1)).is_err(),
        "same account"
    );
    assert!(
        world.spawn_player(&spawn(2000009, 150001, 1, 1)).is_err(),
        "same char"
    );
    assert_eq!(world.entity_by_gid(2000001), Some(a));
    assert_eq!(world.entity_by_char(CharId(150001)), Some(a));
    assert_eq!(
        world.entities.slot_count(),
        1,
        "rejected spawns allocate nothing"
    );
}

#[test]
fn eng_02_capacity_is_enforced() {
    let mut world = World::new(50, 50, 1);
    world.spawn_player(&spawn(1, 1, 1, 1)).expect("first");
    assert!(world.spawn_player(&spawn(2, 2, 1, 1)).is_err());
}

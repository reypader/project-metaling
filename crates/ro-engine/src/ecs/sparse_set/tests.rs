use super::*;
use crate::ecs::Entity;

fn e(index: u32, generation: u32) -> Entity {
    Entity::new(index, generation)
}

#[test]
fn eng_02_insert_get_contains() {
    let mut set = SparseSet::new();
    assert_eq!(set.insert(e(3, 1), "a"), None);
    assert!(set.contains(e(3, 1)));
    assert_eq!(set.get(e(3, 1)), Some(&"a"));
    assert_eq!(set.insert(e(3, 1), "b"), Some("a"), "insert replaces");
    assert_eq!(set.len(), 1);
}

#[test]
fn eng_02_remove_swap_keeps_other_entries_reachable() {
    let mut set = SparseSet::new();
    set.insert(e(0, 1), 10);
    set.insert(e(5, 1), 50);
    set.insert(e(9, 1), 90);
    assert_eq!(set.remove(e(0, 1)), Some(10));
    assert_eq!(set.get(e(5, 1)), Some(&50));
    assert_eq!(set.get(e(9, 1)), Some(&90));
    assert!(!set.contains(e(0, 1)));
    assert_eq!(set.len(), 2);
    if let Some(v) = set.get_mut(e(9, 1)) {
        *v = 91;
    }
    assert_eq!(set.get(e(9, 1)), Some(&91));
}

#[test]
fn eng_02_dense_slices_iterate() {
    let mut set = SparseSet::new();
    set.insert(e(2, 1), 'x');
    set.insert(e(4, 1), 'y');
    assert_eq!(set.entities(), &[e(2, 1), e(4, 1)]);
    assert_eq!(set.values(), &['x', 'y']);
    for v in set.values_mut() {
        *v = 'z';
    }
    assert!(set.values().iter().all(|&v| v == 'z'));
}

#[test]
fn eng_02_remove_missing_is_noop() {
    let mut set: SparseSet<u8> = SparseSet::new();
    assert_eq!(set.remove(e(1, 1)), None);
    set.insert(e(1, 1), 1);
    assert_eq!(set.remove(e(100, 1)), None);
    assert_eq!(set.len(), 1);
}

#[test]
fn eng_02_stale_handle_returns_none() {
    let mut set = SparseSet::new();
    set.insert(e(1, 2), 7);
    assert_eq!(set.get(e(1, 1)), None);
    assert!(!set.contains(e(1, 1)));
    assert_eq!(set.remove(e(1, 1)), None);
    assert_eq!(set.get(e(1, 2)), Some(&7));
}

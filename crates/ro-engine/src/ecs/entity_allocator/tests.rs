use super::*;
use crate::EngineError;

#[test]
fn eng_01_first_allocation_is_index_0_generation_1() {
    let mut alloc = EntityAllocator::with_capacity(4);
    let e = alloc.allocate().expect("capacity available");
    assert_eq!(e.index(), 0);
    assert_eq!(e.generation(), 1);
    assert!(alloc.is_alive(e));
}

#[test]
fn eng_01_free_then_allocate_reuses_index_with_next_generation() {
    let mut alloc = EntityAllocator::with_capacity(4);
    let first = alloc.allocate().expect("alloc");
    alloc.free(first).expect("free live entity");
    assert!(!alloc.is_alive(first), "stale handle is dead");
    let second = alloc.allocate().expect("alloc");
    assert_eq!(second.index(), 0);
    assert_eq!(second.generation(), 2);
    assert!(alloc.is_alive(second));
    assert!(
        !alloc.is_alive(first),
        "old generation stays dead after reuse"
    );
}

#[test]
fn eng_01_double_free_errors_and_keeps_free_list_intact() {
    let mut alloc = EntityAllocator::with_capacity(4);
    let a = alloc.allocate().expect("alloc");
    let b = alloc.allocate().expect("alloc");
    alloc.free(a).expect("first free");
    assert!(alloc.free(a).is_err(), "double free must error");
    // Only one slot (index 0) is free: next alloc reuses it, the one after is fresh.
    let c = alloc.allocate().expect("alloc");
    let d = alloc.allocate().expect("alloc");
    assert_eq!(c.index(), 0);
    assert_eq!(d.index(), 2);
    assert!(alloc.is_alive(b));
}

#[test]
fn eng_01_allocating_past_capacity_fails() {
    let mut alloc = EntityAllocator::with_capacity(2);
    alloc.allocate().expect("1");
    alloc.allocate().expect("2");
    assert_eq!(alloc.allocate(), Err(EngineError::WorldCapacityExceeded));
}

#[test]
fn eng_01_generation_wrap_retires_slot_instead_of_reviving() {
    let mut alloc = EntityAllocator::with_capacity(2);
    let e = alloc.allocate().expect("alloc");
    alloc.force_generation_for_test(e.index(), u32::MAX);
    let max_gen = Entity::new(0, u32::MAX);
    assert!(alloc.is_alive(max_gen));
    alloc.free(max_gen).expect("free");
    let next = alloc.allocate().expect("alloc");
    assert_ne!(next.index(), 0, "a slot at u32::MAX generation is retired");
    assert!(!alloc.is_alive(Entity::new(0, 0)));
    assert!(!alloc.is_alive(Entity::new(0, 1)));
    assert!(!alloc.is_alive(max_gen));
}

#[test]
fn eng_01_unknown_index_is_not_alive() {
    let alloc = EntityAllocator::with_capacity(2);
    assert!(!alloc.is_alive(Entity::new(7, 1)));
}

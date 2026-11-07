use super::*;

#[test]
fn splitmix_is_deterministic_per_name() {
    let mut a = SplitMix64::from_name("prontera.gat");
    let mut b = SplitMix64::from_name("prontera.gat");
    let mut c = SplitMix64::from_name("geffen.gat");
    let seq_a: Vec<u64> = (0..4).map(|_| a.next_u64()).collect();
    let seq_b: Vec<u64> = (0..4).map(|_| b.next_u64()).collect();
    assert_eq!(seq_a, seq_b);
    assert_ne!(seq_a[0], c.next_u64());
}

#[test]
fn range_inclusive_hits_both_ends_and_nothing_outside() {
    let mut rng = SplitMix64::from_name("t");
    let mut seen = [false; 5];
    for _ in 0..1000 {
        let v = rng.range_inclusive(-2, 2);
        assert!((-2..=2).contains(&v), "{v}");
        seen[(v + 2) as usize] = true;
    }
    assert!(seen.iter().all(|&s| s), "{seen:?}");
    assert_eq!(rng.range_inclusive(7, 7), 7);
    assert_eq!(rng.range_inclusive(7, 3), 7, "empty range gives a");
    let wide = rng.range_inclusive(i32::MIN, i32::MAX);
    assert!(
        (i32::MIN..=i32::MAX).contains(&wide),
        "full range does not overflow"
    );
    assert_eq!(rng.below(0), 0);
    assert!(rng.below(3) < 3);
}

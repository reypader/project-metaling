use super::*;

#[test]
fn eng_06_step_duration_speed_150() {
    assert_eq!(step_duration_ms(150, false), 150);
    assert_eq!(step_duration_ms(150, true), 210);
}

#[test]
fn eng_06_step_duration_speed_112_uses_integer_math() {
    assert_eq!(step_duration_ms(112, false), 112);
    assert_eq!(step_duration_ms(112, true), 156);
}

#[test]
fn walking_path_tracks_steps() {
    let mut path = WalkingPath::new(&[(101, 100), (102, 101)], 1000, 150, (100, 100));
    assert_eq!(path.dest(), (102, 101));
    assert_eq!(path.next_cell(), Some((101, 100)));
    assert_eq!(path.next_step_at, 1150);
    path.advance(150, (101, 100));
    assert_eq!(path.next_cell(), Some((102, 101)));
    assert_eq!(path.next_step_at, 1150 + 210, "diagonal from (101,100)");
    path.advance(150, (102, 101));
    assert_eq!(path.next_cell(), None);
}

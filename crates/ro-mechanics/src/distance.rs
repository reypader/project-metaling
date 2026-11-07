/// The client's circular distance (`check_distance_client`): `floor(max(√(dx² + dy²) − 0.1,
/// 0))`. Straight lines reach one cell further than diagonals.
pub fn client_distance(dx: i32, dy: i32) -> i32 {
    let (dx, dy) = (f64::from(dx), f64::from(dy));
    ((dx * dx + dy * dy).sqrt() - 0.1).max(0.0) as i32
}

/// Whether a cell `(dx, dy)` away is within `range` by the client's distance.
pub fn check_distance_client(dx: i32, dy: i32, range: u16) -> bool {
    client_distance(dx, dy) <= i32::from(range)
}

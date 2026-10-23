//! Conversions between bare map names (`prontera`, as in NPC data) and `.gat` names
//! (`prontera.gat`, as on the wire and in persistence). Case-sensitive.

const GAT_SUFFIX: &str = ".gat";

/// Appends `.gat` unless `bare` already ends with it.
pub fn gat_map_name(bare: &str) -> String {
    if bare.ends_with(GAT_SUFFIX) {
        bare.to_string()
    } else {
        format!("{bare}{GAT_SUFFIX}")
    }
}

/// Strips one trailing `.gat`, if present.
pub fn bare_map_name(name: &str) -> &str {
    name.strip_suffix(GAT_SUFFIX).unwrap_or(name)
}

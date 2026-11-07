//! Fixtures and helpers the test modules share.

use std::path::PathBuf;

use item_import::{convert, Converted, SOURCES};
use ro_core::ItemId;
use ro_db::ItemData;

pub fn db(body: &str) -> String {
    format!("Header:\n  Type: ITEM_DB\n  Version: 3\nBody:\n{body}")
}

pub fn run(usable: &str, equip: &str, etc: &str) -> Converted {
    let (u, q, e) = (db(usable), db(equip), db(etc));
    convert(&[("usable.yml", &u), ("equip.yml", &q), ("etc.yml", &e)]).expect("converts")
}

pub fn find(out: &Converted, id: u32) -> &ItemData {
    out.files
        .iter()
        .flatten()
        .find(|i| i.id == ItemId(id))
        .unwrap_or_else(|| panic!("item {id} converted"))
}

pub fn jobs_of(list: &str) -> [u64; 3] {
    let equip = format!(
        "  - Id: 1201\n    AegisName: Knife\n    Name: Knife\n    Type: Weapon\n{list}    Locations:\n      Right_Hand: true\n"
    );
    find(&run("", &equip, ""), 1201).jobs
}

pub fn item_with(fields: &str) -> ItemData {
    let equip = format!(
        "  - Id: 1201\n    AegisName: Knife\n    Name: Knife\n    Type: Weapon\n{fields}    Locations:\n      Right_Hand: true\n"
    );
    find(&run("", &equip, ""), 1201).clone()
}

pub fn rathena() -> Option<PathBuf> {
    let path = PathBuf::from(std::env::var_os("RATHENA_PATH")?);
    path.join(SOURCES[0].0).is_file().then_some(path)
}

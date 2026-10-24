//! PostgreSQL implementation of `InventoryRepository`.

use async_trait::async_trait;
use sqlx::postgres::PgRow;
use sqlx::query;

use ro_core::{
    CharId, CharacterInventory, CharacterStatus, InventoryItem, ItemId, ItemOption,
    INVENTORY_SLOTS, ITEM_CARD_SLOTS, ITEM_OPTION_SLOTS,
};

use super::{get_col, map_sqlx_error};
use crate::{InventoryRepository, PlayState, PostgresRepo, RepositoryError};

const SELECT_ITEMS: &str = "SELECT nameid, amount, equip, identify, refine, attribute, \
    card0, card1, card2, card3, \
    option_id0, option_val0, option_parm0, option_id1, option_val1, option_parm1, \
    option_id2, option_val2, option_parm2, option_id3, option_val3, option_parm3, \
    option_id4, option_val4, option_parm4, \
    expire_time, favorite, bound, unique_id, equip_switch, enchantgrade \
    FROM inventory WHERE char_id = $1 ORDER BY id";

/// One multi-row insert: every column is an array, rows keep their slot order (`ORDINALITY`), so
/// row ids rise in slot order.
const INSERT_ITEMS: &str = "INSERT INTO inventory (char_id, nameid, amount, equip, identify, \
    refine, attribute, card0, card1, card2, card3, \
    option_id0, option_val0, option_parm0, option_id1, option_val1, option_parm1, \
    option_id2, option_val2, option_parm2, option_id3, option_val3, option_parm3, \
    option_id4, option_val4, option_parm4, \
    expire_time, favorite, bound, unique_id, equip_switch, enchantgrade) \
    SELECT $1, t.nameid, t.amount, t.equip, t.identify, t.refine, t.attribute, \
    t.card0, t.card1, t.card2, t.card3, \
    t.option_id0, t.option_val0, t.option_parm0, t.option_id1, t.option_val1, t.option_parm1, \
    t.option_id2, t.option_val2, t.option_parm2, t.option_id3, t.option_val3, t.option_parm3, \
    t.option_id4, t.option_val4, t.option_parm4, \
    t.expire_time, t.favorite, t.bound, t.unique_id, t.equip_switch, t.enchantgrade \
    FROM UNNEST($2::bigint[], $3::int[], $4::bigint[], $5::smallint[], $6::smallint[], \
    $7::smallint[], $8::bigint[], $9::bigint[], $10::bigint[], $11::bigint[], \
    $12::smallint[], $13::smallint[], $14::smallint[], $15::smallint[], $16::smallint[], \
    $17::smallint[], $18::smallint[], $19::smallint[], $20::smallint[], $21::smallint[], \
    $22::smallint[], $23::smallint[], $24::smallint[], $25::smallint[], $26::smallint[], \
    $27::bigint[], $28::smallint[], $29::smallint[], $30::bigint[], $31::bigint[], \
    $32::smallint[]) WITH ORDINALITY AS t(nameid, amount, equip, identify, refine, attribute, \
    card0, card1, card2, card3, \
    option_id0, option_val0, option_parm0, option_id1, option_val1, option_parm1, \
    option_id2, option_val2, option_parm2, option_id3, option_val3, option_parm3, \
    option_id4, option_val4, option_parm4, \
    expire_time, favorite, bound, unique_id, equip_switch, enchantgrade, ord) \
    ORDER BY t.ord";

fn bad(col: &str, value: i64) -> RepositoryError {
    RepositoryError::Internal(format!("inventory column '{col}' out of range: {value}"))
}

/// Reads an integer column stored wider than its Rust type.
fn narrow<T: TryFrom<i64>>(col: &str, wide: i64) -> Result<T, RepositoryError> {
    T::try_from(wide).map_err(|_| bad(col, wide))
}

fn big<T: TryFrom<i64>>(row: &PgRow, col: &str) -> Result<T, RepositoryError> {
    let wide: i64 = get_col(row, col)?;
    narrow(col, wide)
}

fn int<T: TryFrom<i64>>(row: &PgRow, col: &str) -> Result<T, RepositoryError> {
    let wide: i32 = get_col(row, col)?;
    narrow(col, i64::from(wide))
}

fn small<T: TryFrom<i64>>(row: &PgRow, col: &str) -> Result<T, RepositoryError> {
    let wide: i16 = get_col(row, col)?;
    narrow(col, i64::from(wide))
}

const CARD_COLS: [&str; ITEM_CARD_SLOTS] = ["card0", "card1", "card2", "card3"];
const OPTION_COLS: [[&str; 3]; ITEM_OPTION_SLOTS] = [
    ["option_id0", "option_val0", "option_parm0"],
    ["option_id1", "option_val1", "option_parm1"],
    ["option_id2", "option_val2", "option_parm2"],
    ["option_id3", "option_val3", "option_parm3"],
    ["option_id4", "option_val4", "option_parm4"],
];

fn row_to_item(row: &PgRow) -> Result<InventoryItem, RepositoryError> {
    let amount: i32 = get_col(row, "amount")?;
    let mut cards = [0u32; ITEM_CARD_SLOTS];
    for (card, col) in cards.iter_mut().zip(CARD_COLS) {
        *card = big(row, col)?;
    }
    let mut options = [ItemOption::default(); ITEM_OPTION_SLOTS];
    for (option, [id, val, parm]) in options.iter_mut().zip(OPTION_COLS) {
        *option = ItemOption {
            id: get_col(row, id)?,
            value: get_col(row, val)?,
            param: small(row, parm)?,
        };
    }
    let unique_id: i64 = get_col(row, "unique_id")?;
    Ok(InventoryItem {
        item_id: ItemId(big(row, "nameid")?),
        amount: narrow("amount", i64::from(amount))?,
        equip: big(row, "equip")?,
        identified: small::<u8>(row, "identify")? != 0,
        refine: small(row, "refine")?,
        attribute: small(row, "attribute")?,
        cards,
        options,
        expire_time: big(row, "expire_time")?,
        favorite: small::<u8>(row, "favorite")? != 0,
        bound: small(row, "bound")?,
        // Stored bit-cast: the top bit of a u64 is the sign bit of the BIGINT.
        unique_id: unique_id as u64,
        equip_switch: big(row, "equip_switch")?,
        enchant_grade: small(row, "enchantgrade")?,
    })
}

/// The insert's column arrays, one entry per stored item.
#[derive(Default)]
struct Columns {
    nameid: Vec<i64>,
    amount: Vec<i32>,
    equip: Vec<i64>,
    identify: Vec<i16>,
    refine: Vec<i16>,
    attribute: Vec<i16>,
    cards: [Vec<i64>; ITEM_CARD_SLOTS],
    option_id: [Vec<i16>; ITEM_OPTION_SLOTS],
    option_val: [Vec<i16>; ITEM_OPTION_SLOTS],
    option_parm: [Vec<i16>; ITEM_OPTION_SLOTS],
    expire_time: Vec<i64>,
    favorite: Vec<i16>,
    bound: Vec<i16>,
    unique_id: Vec<i64>,
    equip_switch: Vec<i64>,
    enchantgrade: Vec<i16>,
}

impl Columns {
    fn push(&mut self, item: &InventoryItem) {
        self.nameid.push(i64::from(item.item_id.0));
        self.amount.push(i32::from(item.amount));
        self.equip.push(i64::from(item.equip));
        self.identify.push(i16::from(item.identified));
        self.refine.push(i16::from(item.refine));
        self.attribute.push(i16::from(item.attribute));
        for (col, card) in self.cards.iter_mut().zip(item.cards) {
            col.push(i64::from(card));
        }
        for (i, option) in item.options.iter().enumerate() {
            self.option_id[i].push(option.id);
            self.option_val[i].push(option.value);
            self.option_parm[i].push(i16::from(option.param));
        }
        self.expire_time.push(i64::from(item.expire_time));
        self.favorite.push(i16::from(item.favorite));
        self.bound.push(i16::from(item.bound));
        self.unique_id.push(item.unique_id as i64);
        self.equip_switch.push(i64::from(item.equip_switch));
        self.enchantgrade.push(i16::from(item.enchant_grade));
    }
}

#[async_trait]
impl InventoryRepository for PostgresRepo {
    async fn load_play_state(&self, char_id: CharId) -> Result<Option<PlayState>, RepositoryError> {
        // One snapshot for both reads, so a save committing in between can't pair the old
        // counter with the new items (which could hand out a unique_id twice).
        let mut tx = self.pool.begin().await.map_err(map_sqlx_error)?;
        query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx_error)?;
        let Some(row) = query(
            "SELECT c.str, c.agi, c.vit, c.int_stat, c.dex, c.luk, c.status_point, \
             c.skill_point, c.base_exp, c.job_exp, c.hp, c.max_hp, c.sp, c.max_sp, \
             c.unique_item_counter, a.group_id \
             FROM characters c JOIN accounts a ON a.account_id = c.account_id \
             WHERE c.char_id = $1",
        )
        .bind(char_id.0 as i32)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx_error)?
        else {
            return Ok(None);
        };
        let status = CharacterStatus {
            str: small(&row, "str")?,
            agi: small(&row, "agi")?,
            vit: small(&row, "vit")?,
            int: small(&row, "int_stat")?,
            dex: small(&row, "dex")?,
            luk: small(&row, "luk")?,
            status_point: small(&row, "status_point")?,
            skill_point: small(&row, "skill_point")?,
            base_exp: big(&row, "base_exp")?,
            job_exp: big(&row, "job_exp")?,
            hp: int(&row, "hp")?,
            max_hp: int(&row, "max_hp")?,
            sp: int(&row, "sp")?,
            max_sp: int(&row, "max_sp")?,
        };
        let unique_item_counter = big(&row, "unique_item_counter")?;
        let group_id: i32 = get_col(&row, "group_id")?;

        let rows = query(SELECT_ITEMS)
            .bind(char_id.0 as i32)
            .fetch_all(&mut *tx)
            .await
            .map_err(map_sqlx_error)?;
        tx.commit().await.map_err(map_sqlx_error)?;
        // Refuse rather than drop the extra rows: the next save would delete them for good.
        if rows.len() > INVENTORY_SLOTS {
            return Err(RepositoryError::Internal(format!(
                "character {} has {} inventory rows (max {INVENTORY_SLOTS})",
                char_id.0,
                rows.len()
            )));
        }
        let slots = rows
            .iter()
            .map(|row| row_to_item(row).map(Some))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(PlayState {
            status,
            group_id,
            inventory: CharacterInventory {
                slots,
                unique_item_counter,
            },
        }))
    }

    async fn save_inventory(
        &self,
        char_id: CharId,
        inventory: &CharacterInventory,
    ) -> Result<(), RepositoryError> {
        if inventory.slots.len() > INVENTORY_SLOTS {
            return Err(RepositoryError::Internal(format!(
                "inventory of character {} has {} slots (max {INVENTORY_SLOTS})",
                char_id.0,
                inventory.slots.len()
            )));
        }
        let char_id = char_id.0 as i32;
        let mut tx = self.pool.begin().await.map_err(map_sqlx_error)?;
        // This UPDATE must stay first: its row lock on `characters` serializes concurrent saves
        // of one character. With the DELETE first, a second save could miss the first one's
        // inserted rows and both inventories would survive (`per_inv_03_concurrent_saves_serialize`).
        let updated = query("UPDATE characters SET unique_item_counter = $1 WHERE char_id = $2")
            .bind(i64::from(inventory.unique_item_counter))
            .bind(char_id)
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx_error)?;
        if updated.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }
        query("DELETE FROM inventory WHERE char_id = $1")
            .bind(char_id)
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx_error)?;

        let mut cols = Columns::default();
        for item in inventory.items() {
            cols.push(item);
        }
        if !cols.nameid.is_empty() {
            let [card0, card1, card2, card3] = cols.cards;
            let mut insert = query(INSERT_ITEMS)
                .bind(char_id)
                .bind(cols.nameid)
                .bind(cols.amount)
                .bind(cols.equip)
                .bind(cols.identify)
                .bind(cols.refine)
                .bind(cols.attribute)
                .bind(card0)
                .bind(card1)
                .bind(card2)
                .bind(card3);
            for ((id, val), parm) in cols
                .option_id
                .into_iter()
                .zip(cols.option_val)
                .zip(cols.option_parm)
            {
                insert = insert.bind(id).bind(val).bind(parm);
            }
            insert
                .bind(cols.expire_time)
                .bind(cols.favorite)
                .bind(cols.bound)
                .bind(cols.unique_id)
                .bind(cols.equip_switch)
                .bind(cols.enchantgrade)
                .execute(&mut *tx)
                .await
                .map_err(map_sqlx_error)?;
        }
        tx.commit().await.map_err(map_sqlx_error)
    }
}

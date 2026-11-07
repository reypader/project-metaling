//! Character repository implementation, slot queries, and character lifecycle management.

use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use sqlx::postgres::{PgRow, Postgres};
use sqlx::{query, Error as SqlxError, Transaction};

use crate::RepositoryError;
use crate::{CancelDeleteOutcome, CharacterRepository, CreateCharacterError, DeleteReserveOutcome};
use ro_core::{
    AccountId, CharId, CharacterLooks, CharacterSelectStats, CharacterSummary,
    CreateCharacterRequest, MapPosition, Sex, MAX_ZENY,
};

use super::mapping::{get_col, map_sex, map_sqlx_error, sex_to_i16};
use super::PostgresRepo;

/// The `characters` columns `row_to_character_summary` reads, for every summary query.
macro_rules! summary_columns {
    () => {
        "char_id, account_id, slot, name, sex, job_class, base_level, job_level, \
         hair_style, hair_color, last_map, last_x, last_y, last_dir, delete_date, \
         weapon_type, shield_view, head_top_view, head_mid_view, head_bot_view, robe_view, \
         base_exp, job_exp, hp, max_hp, sp, max_sp, status_point, skill_point, \
         str, agi, vit, int_stat, dex, luk, zeny"
    };
}

/// Validates pure invariant constraints on a character creation request.
pub(crate) fn validate_create_character_request(
    req: &CreateCharacterRequest,
    max_slots: u8,
) -> Result<(), CreateCharacterError> {
    if req.slot >= max_slots {
        return Err(CreateCharacterError::IneligibleSlot);
    }

    if req.sex != Sex::Female && req.sex != Sex::Male {
        return Err(CreateCharacterError::CreationDenied);
    }

    if req.starting_job != 0 && req.starting_job != 4218 {
        return Err(CreateCharacterError::CreationDenied);
    }

    let name_len = req.name.len();
    if !(4..=23).contains(&name_len) {
        return Err(CreateCharacterError::CreationDenied);
    }

    if req.hair_style > i16::MAX as u16 || req.hair_color > i16::MAX as u16 {
        return Err(CreateCharacterError::CreationDenied);
    }

    if req.name.starts_with(' ') || req.name.ends_with(' ') {
        return Err(CreateCharacterError::InvalidNameSymbols);
    }

    if req.name.starts_with('#') || !req.name.bytes().all(|b| (0x20..=0x7E).contains(&b)) {
        return Err(CreateCharacterError::InvalidNameSymbols);
    }

    if req.name.eq_ignore_ascii_case("server") {
        return Err(CreateCharacterError::NameAlreadyExists);
    }

    Ok(())
}

/// Verifies name uniqueness and slot availability inside an active transaction.
pub(crate) async fn check_slot_and_name_availability(
    tx: &mut Transaction<'_, Postgres>,
    req: &CreateCharacterRequest,
    max_slots: u8,
) -> Result<(), CreateCharacterError> {
    let account_exists = query("SELECT 1 FROM accounts WHERE account_id = $1 FOR UPDATE")
        .bind(req.account_id.0 as i32)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|e| CreateCharacterError::Repository(map_sqlx_error(e)))?;

    if account_exists.is_none() {
        return Err(CreateCharacterError::CreationDenied);
    }

    let existing_name = query("SELECT 1 FROM characters WHERE LOWER(name) = LOWER($1)")
        .bind(&req.name)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|e| CreateCharacterError::Repository(map_sqlx_error(e)))?;

    if existing_name.is_some() {
        return Err(CreateCharacterError::NameAlreadyExists);
    }

    let existing_chars = query("SELECT slot FROM characters WHERE account_id = $1 FOR UPDATE")
        .bind(req.account_id.0 as i32)
        .fetch_all(&mut **tx)
        .await
        .map_err(|e| CreateCharacterError::Repository(map_sqlx_error(e)))?;

    if existing_chars.len() >= max_slots as usize {
        return Err(CreateCharacterError::CreationDenied);
    }

    for row in &existing_chars {
        let slot: i16 = get_col(row, "slot").map_err(CreateCharacterError::Repository)?;
        if slot == req.slot as i16 {
            return Err(CreateCharacterError::CreationDenied);
        }
    }

    Ok(())
}

/// Maps database errors during character insertion into specific `CreateCharacterError` variants.
pub(crate) fn map_db_create_error(err: SqlxError) -> CreateCharacterError {
    match err {
        SqlxError::Database(db_err) if db_err.is_unique_violation() => {
            let constraint = db_err.constraint().unwrap_or("");
            let msg = db_err.message();
            let is_name_violation = constraint.contains("name")
                || constraint.contains("idx_characters_name_lower")
                || msg.contains("characters_name_key")
                || msg.contains("idx_characters_name_lower");
            if is_name_violation {
                CreateCharacterError::NameAlreadyExists
            } else {
                CreateCharacterError::CreationDenied
            }
        }
        other => CreateCharacterError::Repository(map_sqlx_error(other)),
    }
}

/// The select-screen status values of a summary row. Like the looks, an out-of-range value (a
/// hand-edited row) shows as 0 rather than failing the whole list.
fn select_stats(row: &PgRow) -> Result<CharacterSelectStats, RepositoryError> {
    let big = |col: &str| -> Result<u64, RepositoryError> {
        let v: i64 = get_col(row, col)?;
        Ok(u64::try_from(v).unwrap_or(0))
    };
    let int = |col: &str| -> Result<u32, RepositoryError> {
        let v: i32 = get_col(row, col)?;
        Ok(u32::try_from(v).unwrap_or(0))
    };
    let small = |col: &str| -> Result<u16, RepositoryError> {
        let v: i16 = get_col(row, col)?;
        Ok(u16::try_from(v).unwrap_or(0))
    };
    Ok(CharacterSelectStats {
        base_exp: big("base_exp")?,
        job_exp: big("job_exp")?,
        hp: int("hp")?,
        max_hp: int("max_hp")?,
        sp: int("sp")?,
        max_sp: int("max_sp")?,
        status_point: small("status_point")?,
        skill_point: small("skill_point")?,
        stats: [
            small("str")?,
            small("agi")?,
            small("vit")?,
            small("int_stat")?,
            small("dex")?,
            small("luk")?,
        ],
        // A hand-edited value above the cap loads capped (spec §2.8).
        zeny: int("zeny")?.min(MAX_ZENY),
    })
}

/// Maps a database row to a `CharacterSummary` model using the row extraction helper.
pub(crate) fn row_to_character_summary(row: &PgRow) -> Result<CharacterSummary, RepositoryError> {
    let char_id: i32 = get_col(row, "char_id")?;
    let account_id: i32 = get_col(row, "account_id")?;
    let slot: i16 = get_col(row, "slot")?;
    let name: String = get_col(row, "name")?;
    let sex: i16 = get_col(row, "sex")?;
    let job_class: i16 = get_col(row, "job_class")?;
    let base_level: i16 = get_col(row, "base_level")?;
    let job_level: i16 = get_col(row, "job_level")?;
    let hair_style: i16 = get_col(row, "hair_style")?;
    let hair_color: i16 = get_col(row, "hair_color")?;
    let last_map: String = get_col(row, "last_map")?;
    let last_x: i16 = get_col(row, "last_x")?;
    let last_y: i16 = get_col(row, "last_y")?;
    let last_dir: i16 = get_col(row, "last_dir")?;
    let delete_date: Option<i64> = get_col(row, "delete_date")?;
    let look = |col: &str| -> Result<u16, RepositoryError> {
        let v: i32 = get_col(row, col)?;
        // An out-of-range value (a hand-edited row) shows as no look.
        Ok(u16::try_from(v).unwrap_or(0))
    };
    let looks = CharacterLooks {
        weapon_type: look("weapon_type")?,
        shield: look("shield_view")?,
        head_top: look("head_top_view")?,
        head_mid: look("head_mid_view")?,
        head_bottom: look("head_bot_view")?,
        robe: look("robe_view")?,
    };

    Ok(CharacterSummary {
        char_id: CharId(char_id as u32),
        account_id: AccountId(account_id as u32),
        slot: slot as u8,
        name,
        sex: map_sex(sex),
        job_class: job_class as u16,
        base_level: base_level as u16,
        job_level: job_level as u16,
        hair_style: hair_style as u16,
        hair_color: hair_color as u16,
        last_map,
        last_pos: MapPosition {
            x: last_x as u16,
            y: last_y as u16,
            dir: last_dir as u8,
        },
        delete_date: delete_date.map(|t| t as u64),
        looks,
        select_stats: select_stats(row)?,
    })
}

#[async_trait]
impl CharacterRepository for PostgresRepo {
    async fn list_by_account(
        &self,
        account_id: AccountId,
    ) -> Result<Vec<CharacterSummary>, RepositoryError> {
        let rows = query(concat!(
            r#"
            SELECT "#,
            summary_columns!(),
            r#"
            FROM characters
            WHERE account_id = $1
            ORDER BY slot ASC
            "#
        ))
        .bind(account_id.0 as i32)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.iter().map(row_to_character_summary).collect()
    }

    async fn find_by_slot(
        &self,
        account_id: AccountId,
        slot: u8,
    ) -> Result<Option<CharacterSummary>, RepositoryError> {
        let row = query(concat!(
            r#"
            SELECT "#,
            summary_columns!(),
            r#"
            FROM characters
            WHERE account_id = $1 AND slot = $2
            "#
        ))
        .bind(account_id.0 as i32)
        .bind(slot as i16)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        match row {
            Some(r) => Ok(Some(row_to_character_summary(&r)?)),
            None => Ok(None),
        }
    }

    async fn find_by_id(
        &self,
        char_id: CharId,
    ) -> Result<Option<CharacterSummary>, RepositoryError> {
        let row = query(concat!(
            r#"
            SELECT "#,
            summary_columns!(),
            r#"
            FROM characters
            WHERE char_id = $1
            "#
        ))
        .bind(char_id.0 as i32)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        match row {
            Some(r) => Ok(Some(row_to_character_summary(&r)?)),
            None => Ok(None),
        }
    }

    async fn create_character(
        &self,
        req: CreateCharacterRequest,
        max_slots: u8,
    ) -> Result<CharacterSummary, CreateCharacterError> {
        validate_create_character_request(&req, max_slots)?;

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| CreateCharacterError::Repository(map_sqlx_error(e)))?;

        check_slot_and_name_availability(&mut tx, &req, max_slots).await?;

        let row = query(concat!(
            r#"
            INSERT INTO characters (
                account_id, slot, name, sex, job_class, base_level, job_level,
                hair_style, hair_color, last_map, last_x, last_y, last_dir,
                save_map, save_x, save_y, delete_date
            ) VALUES (
                $1, $2, $3, $4, $5, 1, 1,
                $6, $7, 'prontera.gat', 156, 191, 0,
                'prontera.gat', 156, 191, NULL
            )
            RETURNING "#,
            summary_columns!(),
            r#"
            "#
        ))
        .bind(req.account_id.0 as i32)
        .bind(req.slot as i16)
        .bind(&req.name)
        .bind(sex_to_i16(req.sex))
        .bind(req.starting_job as i16)
        .bind(req.hair_style as i16)
        .bind(req.hair_color as i16)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db_create_error)?;

        let summary = row_to_character_summary(&row)?;

        tx.commit()
            .await
            .map_err(|e| CreateCharacterError::Repository(map_sqlx_error(e)))?;

        Ok(summary)
    }

    async fn reserve_delete(
        &self,
        account_id: AccountId,
        char_id: CharId,
        delay_seconds: u32,
    ) -> Result<DeleteReserveOutcome, RepositoryError> {
        let row = query("SELECT account_id, delete_date FROM characters WHERE char_id = $1")
            .bind(char_id.0 as i32)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_sqlx_error)?;

        let row = match row {
            Some(r) => r,
            None => return Ok(DeleteReserveOutcome::NotFoundOrDbError),
        };

        let row_account_id: i32 = get_col(&row, "account_id")?;
        if row_account_id != account_id.0 as i32 {
            return Ok(DeleteReserveOutcome::NotFoundOrDbError);
        }

        let existing_delete_date: Option<i64> = get_col(&row, "delete_date")?;
        if existing_delete_date.is_some() {
            return Ok(DeleteReserveOutcome::Denied);
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let target_delete_date = now.saturating_add(delay_seconds as u64) as i64;

        let result = query(
            "UPDATE characters SET delete_date = $1 WHERE char_id = $2 AND delete_date IS NULL",
        )
        .bind(target_delete_date)
        .bind(char_id.0 as i32)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        if result.rows_affected() == 0 {
            return Ok(DeleteReserveOutcome::Denied);
        }

        Ok(DeleteReserveOutcome::Success {
            delete_delay_seconds: delay_seconds,
        })
    }

    async fn cancel_delete(
        &self,
        account_id: AccountId,
        char_id: CharId,
    ) -> Result<CancelDeleteOutcome, RepositoryError> {
        let row = query("SELECT account_id, delete_date FROM characters WHERE char_id = $1")
            .bind(char_id.0 as i32)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_sqlx_error)?;

        let row = match row {
            Some(r) => r,
            None => return Ok(CancelDeleteOutcome::NotFound),
        };

        let row_account_id: i32 = get_col(&row, "account_id")?;
        if row_account_id != account_id.0 as i32 {
            return Ok(CancelDeleteOutcome::NotFound);
        }

        let existing_delete_date: Option<i64> = get_col(&row, "delete_date")?;
        if existing_delete_date.is_none() {
            return Ok(CancelDeleteOutcome::NotReserved);
        }

        let result = query(
            "UPDATE characters SET delete_date = NULL WHERE char_id = $1 AND delete_date IS NOT NULL",
        )
        .bind(char_id.0 as i32)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        if result.rows_affected() == 0 {
            return Ok(CancelDeleteOutcome::NotReserved);
        }

        Ok(CancelDeleteOutcome::Success)
    }

    async fn update_position(
        &self,
        char_id: CharId,
        map_name: &str,
        pos: MapPosition,
    ) -> Result<(), RepositoryError> {
        let result = query(
            r#"
            UPDATE characters
            SET last_map = $1, last_x = $2, last_y = $3, last_dir = $4
            WHERE char_id = $5
            "#,
        )
        .bind(map_name)
        .bind(pos.x as i16)
        .bind(pos.y as i16)
        .bind(pos.dir as i16)
        .bind(char_id.0 as i32)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests;

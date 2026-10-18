//! PostgreSQL implementation of repository contracts using `sqlx`.

use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use sqlx::{PgPool, Row};

use ro_contracts::account::{Account, AccountRepository, AccountState, AuthError};
use ro_contracts::character::{
    CancelDeleteOutcome, CharacterRepository, CreateCharacterError, CreateCharacterRequest,
    DeleteReserveOutcome,
};
use ro_contracts::ids::{AccountId, CharId, CharacterSummary, MapPosition, Sex};
use ro_contracts::repository::RepositoryError;

fn map_account_state(val: i16) -> AccountState {
    match val {
        0 => AccountState::Active,
        1 => AccountState::Banned,
        2 => AccountState::Expired,
        3 => AccountState::Locked,
        _ => AccountState::Active,
    }
}

fn account_state_to_i16(state: AccountState) -> i16 {
    match state {
        AccountState::Active => 0,
        AccountState::Banned => 1,
        AccountState::Expired => 2,
        AccountState::Locked => 3,
    }
}

fn map_sex(val: i16) -> Sex {
    match val {
        0 => Sex::Female,
        1 => Sex::Male,
        2 => Sex::Server,
        _ => Sex::Male,
    }
}

fn sex_to_i16(sex: Sex) -> i16 {
    match sex {
        Sex::Female => 0,
        Sex::Male => 1,
        Sex::Server => 2,
    }
}

fn map_sqlx_error(err: sqlx::Error) -> RepositoryError {
    match err {
        sqlx::Error::RowNotFound => RepositoryError::NotFound,
        sqlx::Error::PoolTimedOut | sqlx::Error::Io(_) => RepositoryError::Unavailable(err.to_string()),
        sqlx::Error::Database(db_err) => {
            if db_err.is_unique_violation() {
                RepositoryError::Conflict(db_err.message().to_string())
            } else {
                RepositoryError::Internal(db_err.message().to_string())
            }
        }
        other => RepositoryError::Internal(other.to_string()),
    }
}

fn row_to_account(row: &sqlx::postgres::PgRow) -> Result<Account, RepositoryError> {
    let account_id: i32 = row
        .try_get("account_id")
        .map_err(|e| RepositoryError::Internal(e.to_string()))?;
    let username: String = row
        .try_get("username")
        .map_err(|e| RepositoryError::Internal(e.to_string()))?;
    let password_hash: String = row
        .try_get("password_hash")
        .map_err(|e| RepositoryError::Internal(e.to_string()))?;
    let sex: i16 = row
        .try_get("sex")
        .map_err(|e| RepositoryError::Internal(e.to_string()))?;
    let email: String = row
        .try_get("email")
        .map_err(|e| RepositoryError::Internal(e.to_string()))?;
    let group_id: i32 = row
        .try_get("group_id")
        .map_err(|e| RepositoryError::Internal(e.to_string()))?;
    let state: i16 = row
        .try_get("state")
        .map_err(|e| RepositoryError::Internal(e.to_string()))?;
    let unban_time: Option<i64> = row
        .try_get("unban_time")
        .map_err(|e| RepositoryError::Internal(e.to_string()))?;
    let expiration_time: Option<i64> = row
        .try_get("expiration_time")
        .map_err(|e| RepositoryError::Internal(e.to_string()))?;

    Ok(Account {
        id: AccountId(account_id as u32),
        username,
        password_hash,
        sex: map_sex(sex),
        email,
        group_id,
        state: map_account_state(state),
        unban_time: unban_time.map(|t| t as u64),
        expiration_time: expiration_time.map(|t| t as u64),
    })
}

fn row_to_character_summary(row: &sqlx::postgres::PgRow) -> Result<CharacterSummary, RepositoryError> {
    let char_id: i32 = row
        .try_get("char_id")
        .map_err(|e| RepositoryError::Internal(e.to_string()))?;
    let account_id: i32 = row
        .try_get("account_id")
        .map_err(|e| RepositoryError::Internal(e.to_string()))?;
    let slot: i16 = row
        .try_get("slot")
        .map_err(|e| RepositoryError::Internal(e.to_string()))?;
    let name: String = row
        .try_get("name")
        .map_err(|e| RepositoryError::Internal(e.to_string()))?;
    let sex: i16 = row
        .try_get("sex")
        .map_err(|e| RepositoryError::Internal(e.to_string()))?;
    let job_class: i16 = row
        .try_get("job_class")
        .map_err(|e| RepositoryError::Internal(e.to_string()))?;
    let base_level: i16 = row
        .try_get("base_level")
        .map_err(|e| RepositoryError::Internal(e.to_string()))?;
    let job_level: i16 = row
        .try_get("job_level")
        .map_err(|e| RepositoryError::Internal(e.to_string()))?;
    let hair_style: i16 = row
        .try_get("hair_style")
        .map_err(|e| RepositoryError::Internal(e.to_string()))?;
    let hair_color: i16 = row
        .try_get("hair_color")
        .map_err(|e| RepositoryError::Internal(e.to_string()))?;
    let last_map: String = row
        .try_get("last_map")
        .map_err(|e| RepositoryError::Internal(e.to_string()))?;
    let last_x: i16 = row
        .try_get("last_x")
        .map_err(|e| RepositoryError::Internal(e.to_string()))?;
    let last_y: i16 = row
        .try_get("last_y")
        .map_err(|e| RepositoryError::Internal(e.to_string()))?;
    let last_dir: i16 = row
        .try_get("last_dir")
        .map_err(|e| RepositoryError::Internal(e.to_string()))?;
    let delete_date: Option<i64> = row
        .try_get("delete_date")
        .map_err(|e| RepositoryError::Internal(e.to_string()))?;

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
    })
}

/// Authoritative PostgreSQL repository backed by a connection pool (`sqlx::PgPool`).
#[derive(Debug, Clone)]
pub struct PostgresRepo {
    pool: PgPool,
}

impl PostgresRepo {
    /// Wraps an existing PostgreSQL connection pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Connects to a PostgreSQL database using the provided connection string.
    pub async fn connect(database_url: &str) -> Result<Self, RepositoryError> {
        let pool = PgPool::connect(database_url)
            .await
            .map_err(|e| RepositoryError::Unavailable(e.to_string()))?;
        Ok(Self { pool })
    }

    /// Returns a reference to the underlying connection pool.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Constructs a repository instance from an existing connection pool.
    pub fn from_pool(pool: PgPool) -> Self {
        Self::new(pool)
    }

    /// Executes baseline schema initialization (`01_schema.sql`).
    pub async fn init_schema(&self) -> Result<(), RepositoryError> {
        let schema_sql = include_str!("../sql/01_schema.sql");
        sqlx::raw_sql(schema_sql)
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_error)?;
        Ok(())
    }

    /// Resets accounts and character tables by truncating them with identity restart.
    pub async fn reset_tables(&self) -> Result<(), RepositoryError> {
        sqlx::raw_sql("TRUNCATE TABLE characters, accounts RESTART IDENTITY CASCADE;")
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_error)?;
        Ok(())
    }

    /// Seeds baseline development data (`02_seed.sql`).
    pub async fn seed_test_data(&self) -> Result<(), RepositoryError> {
        let seed_sql = include_str!("../sql/02_seed.sql");
        sqlx::raw_sql(seed_sql)
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_error)?;
        Ok(())
    }

    /// Inserts or updates an account directly in the database.
    pub async fn insert_account(&self, account: &Account) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"
            INSERT INTO accounts (
                account_id, username, password_hash, sex, email, group_id, state, unban_time, expiration_time
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (account_id) DO UPDATE SET
                username = EXCLUDED.username,
                password_hash = EXCLUDED.password_hash,
                sex = EXCLUDED.sex,
                email = EXCLUDED.email,
                group_id = EXCLUDED.group_id,
                state = EXCLUDED.state,
                unban_time = EXCLUDED.unban_time,
                expiration_time = EXCLUDED.expiration_time
            "#,
        )
        .bind(account.id.0 as i32)
        .bind(&account.username)
        .bind(&account.password_hash)
        .bind(sex_to_i16(account.sex))
        .bind(&account.email)
        .bind(account.group_id)
        .bind(account_state_to_i16(account.state))
        .bind(account.unban_time.map(|t| t as i64))
        .bind(account.expiration_time.map(|t| t as i64))
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(())
    }
}

#[async_trait]
impl AccountRepository for PostgresRepo {
    async fn authenticate(&self, username: &str, password: &str) -> Result<Account, AuthError> {
        let row = sqlx::query(
            r#"
            SELECT account_id, username, password_hash, sex, email, group_id, state, unban_time, expiration_time
            FROM accounts
            WHERE username = $1
            "#,
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AuthError::Repository(map_sqlx_error(e)))?;

        let row = match row {
            Some(r) => r,
            None => return Err(AuthError::UnregisteredId),
        };

        let account = row_to_account(&row)?;

        match account.state {
            AccountState::Banned | AccountState::Locked => return Err(AuthError::AccountBanned),
            AccountState::Expired => return Err(AuthError::AccountExpired),
            AccountState::Active => {}
        }

        if account.password_hash != password {
            return Err(AuthError::IncorrectPassword);
        }

        Ok(account)
    }

    async fn find_by_id(&self, account_id: AccountId) -> Result<Option<Account>, AuthError> {
        let row = sqlx::query(
            r#"
            SELECT account_id, username, password_hash, sex, email, group_id, state, unban_time, expiration_time
            FROM accounts
            WHERE account_id = $1
            "#,
        )
        .bind(account_id.0 as i32)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AuthError::Repository(map_sqlx_error(e)))?;

        match row {
            Some(r) => Ok(Some(row_to_account(&r)?)),
            None => Ok(None),
        }
    }

    async fn find_by_username(&self, username: &str) -> Result<Option<Account>, AuthError> {
        let row = sqlx::query(
            r#"
            SELECT account_id, username, password_hash, sex, email, group_id, state, unban_time, expiration_time
            FROM accounts
            WHERE username = $1
            "#,
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AuthError::Repository(map_sqlx_error(e)))?;

        match row {
            Some(r) => Ok(Some(row_to_account(&r)?)),
            None => Ok(None),
        }
    }
}

#[async_trait]
impl CharacterRepository for PostgresRepo {
    async fn list_by_account(
        &self,
        account_id: AccountId,
    ) -> Result<Vec<CharacterSummary>, RepositoryError> {
        let rows = sqlx::query(
            r#"
            SELECT char_id, account_id, slot, name, sex, job_class, base_level, job_level,
                   hair_style, hair_color, last_map, last_x, last_y, last_dir, delete_date
            FROM characters
            WHERE account_id = $1
            ORDER BY slot ASC
            "#,
        )
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
        let row = sqlx::query(
            r#"
            SELECT char_id, account_id, slot, name, sex, job_class, base_level, job_level,
                   hair_style, hair_color, last_map, last_x, last_y, last_dir, delete_date
            FROM characters
            WHERE account_id = $1 AND slot = $2
            "#,
        )
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
        let row = sqlx::query(
            r#"
            SELECT char_id, account_id, slot, name, sex, job_class, base_level, job_level,
                   hair_style, hair_color, last_map, last_x, last_y, last_dir, delete_date
            FROM characters
            WHERE char_id = $1
            "#,
        )
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

        if req.name.starts_with(' ') || req.name.ends_with(' ') {
            return Err(CreateCharacterError::InvalidNameSymbols);
        }

        if req.name.starts_with('#') || !req.name.bytes().all(|b| (0x20..=0x7E).contains(&b)) {
            return Err(CreateCharacterError::InvalidNameSymbols);
        }

        if req.name.eq_ignore_ascii_case("server") {
            return Err(CreateCharacterError::NameAlreadyExists);
        }

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| CreateCharacterError::Repository(map_sqlx_error(e)))?;

        let existing_name = sqlx::query(
            "SELECT 1 FROM characters WHERE LOWER(name) = LOWER($1)",
        )
        .bind(&req.name)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| CreateCharacterError::Repository(map_sqlx_error(e)))?;

        if existing_name.is_some() {
            return Err(CreateCharacterError::NameAlreadyExists);
        }

        let existing_chars = sqlx::query(
            "SELECT slot FROM characters WHERE account_id = $1 FOR UPDATE",
        )
        .bind(req.account_id.0 as i32)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| CreateCharacterError::Repository(map_sqlx_error(e)))?;

        if existing_chars.len() >= max_slots as usize {
            return Err(CreateCharacterError::CreationDenied);
        }

        if existing_chars.iter().any(|row| {
            let slot: i16 = row.try_get("slot").unwrap_or(-1);
            slot == req.slot as i16
        }) {
            return Err(CreateCharacterError::CreationDenied);
        }

        let row = sqlx::query(
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
            RETURNING char_id, account_id, slot, name, sex, job_class, base_level, job_level,
                      hair_style, hair_color, last_map, last_x, last_y, last_dir, delete_date
            "#,
        )
        .bind(req.account_id.0 as i32)
        .bind(req.slot as i16)
        .bind(&req.name)
        .bind(sex_to_i16(req.sex))
        .bind(req.starting_job as i16)
        .bind(req.hair_style as i16)
        .bind(req.hair_color as i16)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(ref db_err) if db_err.is_unique_violation() => {
                let constraint = db_err.constraint().unwrap_or("");
                let msg = db_err.message();
                let is_name_violation = constraint.contains("name")
                    || constraint.contains("idx_characters_name_lower")
                    || msg.contains("characters_name_key")
                    || msg.contains("idx_characters_name_lower")
                    || msg.contains("name");
                if is_name_violation {
                    CreateCharacterError::NameAlreadyExists
                } else {
                    CreateCharacterError::CreationDenied
                }
            }
            other => CreateCharacterError::Repository(map_sqlx_error(other)),
        })?;

        tx.commit()
            .await
            .map_err(|e| CreateCharacterError::Repository(map_sqlx_error(e)))?;

        let summary = row_to_character_summary(&row)?;
        Ok(summary)
    }

    async fn reserve_delete(
        &self,
        account_id: AccountId,
        char_id: CharId,
        delay_seconds: u32,
    ) -> Result<DeleteReserveOutcome, RepositoryError> {
        let row = sqlx::query(
            "SELECT account_id, delete_date FROM characters WHERE char_id = $1",
        )
        .bind(char_id.0 as i32)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        let row = match row {
            Some(r) => r,
            None => return Ok(DeleteReserveOutcome::NotFoundOrDbError),
        };

        let row_account_id: i32 = row
            .try_get("account_id")
            .map_err(|e| RepositoryError::Internal(e.to_string()))?;
        if row_account_id != account_id.0 as i32 {
            return Ok(DeleteReserveOutcome::NotFoundOrDbError);
        }

        let existing_delete_date: Option<i64> = row
            .try_get("delete_date")
            .map_err(|e| RepositoryError::Internal(e.to_string()))?;
        if existing_delete_date.is_some() {
            return Ok(DeleteReserveOutcome::Denied);
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let target_delete_date = now.saturating_add(delay_seconds as u64) as i64;

        let result = sqlx::query(
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
        let row = sqlx::query(
            "SELECT account_id, delete_date FROM characters WHERE char_id = $1",
        )
        .bind(char_id.0 as i32)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        let row = match row {
            Some(r) => r,
            None => return Ok(CancelDeleteOutcome::NotFound),
        };

        let row_account_id: i32 = row
            .try_get("account_id")
            .map_err(|e| RepositoryError::Internal(e.to_string()))?;
        if row_account_id != account_id.0 as i32 {
            return Ok(CancelDeleteOutcome::NotFound);
        }

        let existing_delete_date: Option<i64> = row
            .try_get("delete_date")
            .map_err(|e| RepositoryError::Internal(e.to_string()))?;
        if existing_delete_date.is_none() {
            return Ok(CancelDeleteOutcome::NotReserved);
        }

        let result = sqlx::query(
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
        let result = sqlx::query(
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
mod tests {
    use super::*;

    #[test]
    fn test_parse_account_state() {
        assert_eq!(map_account_state(0), AccountState::Active);
        assert_eq!(map_account_state(1), AccountState::Banned);
        assert_eq!(map_account_state(2), AccountState::Expired);
        assert_eq!(map_account_state(3), AccountState::Locked);
        assert_eq!(map_account_state(99), AccountState::Active);
    }

    #[test]
    fn test_sex_conversions() {
        assert_eq!(map_sex(0), Sex::Female);
        assert_eq!(map_sex(1), Sex::Male);
        assert_eq!(map_sex(2), Sex::Server);
        assert_eq!(map_sex(99), Sex::Male);

        assert_eq!(sex_to_i16(Sex::Female), 0);
        assert_eq!(sex_to_i16(Sex::Male), 1);
        assert_eq!(sex_to_i16(Sex::Server), 2);
    }

    #[test]
    fn test_map_sqlx_error() {
        assert_eq!(
            map_sqlx_error(sqlx::Error::RowNotFound),
            RepositoryError::NotFound
        );
        assert_eq!(
            map_sqlx_error(sqlx::Error::PoolTimedOut),
            RepositoryError::Unavailable(sqlx::Error::PoolTimedOut.to_string())
        );
    }
}



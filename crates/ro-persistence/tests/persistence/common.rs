//! The Postgres connection and the fixtures the repository tests share.

use std::sync::atomic::{AtomicU32, Ordering};

use ro_core::{
    Account, AccountId, AccountState, CharId, CreateCharacterRequest, InventoryItem, ItemId,
    ItemOption, Sex,
};
use ro_persistence::{CharacterRepository, PostgresRepo};

pub static ACCOUNT_COUNTER: std::sync::OnceLock<AtomicU32> = std::sync::OnceLock::new();

pub fn next_account_id() -> AccountId {
    let counter = ACCOUNT_COUNTER.get_or_init(|| {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u32;
        let base = 2_100_000 + (now % 1_000_000) * 1000;
        AtomicU32::new(base)
    });
    AccountId(counter.fetch_add(1, Ordering::SeqCst))
}

pub fn test_db_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://ragnarok:ragnarok@localhost:5432/ragnarok".to_string())
}

pub async fn get_repo() -> PostgresRepo {
    PostgresRepo::connect(&test_db_url())
        .await
        .expect("failed to connect to test database")
}

pub async fn create_test_account(repo: &PostgresRepo, state: AccountState) -> Account {
    let id = next_account_id();
    let _ = sqlx::query("DELETE FROM characters WHERE account_id = $1")
        .bind(id.0 as i32)
        .execute(repo.pool())
        .await;
    let _ = sqlx::query("DELETE FROM accounts WHERE account_id = $1")
        .bind(id.0 as i32)
        .execute(repo.pool())
        .await;

    let username = format!("u_{}", id.0);
    let account = Account {
        id,
        username: username.clone(),
        password_hash: "password".to_string(),
        sex: Sex::Male,
        email: format!("{}@example.com", username),
        group_id: 0,
        state,
        unban_time: None,
        expiration_time: None,
    };
    repo.insert_account(&account)
        .await
        .expect("account insertion failed");
    account
}

// ---- Inventory (spec PER-INV-01..04) ----

/// A schema-initialized repo (the inventory table and counter column may be new) and a fresh
/// character on a fresh account.
pub async fn inventory_fixture(group_id: i32) -> (PostgresRepo, Account, CharId) {
    let repo = get_repo().await;
    repo.init_schema().await.expect("schema applies");
    let mut acc = create_test_account(&repo, AccountState::Active).await;
    acc.group_id = group_id;
    repo.insert_account(&acc).await.expect("group update");
    let req = CreateCharacterRequest {
        account_id: acc.id,
        name: format!("Inv_{}", acc.id.0),
        slot: 0,
        hair_color: 1,
        hair_style: 2,
        starting_job: 0,
        sex: Sex::Male,
    };
    let summary = repo.create_character(req, 9).await.expect("character");
    (repo, acc, summary.char_id)
}

pub fn full_item() -> InventoryItem {
    InventoryItem {
        item_id: ItemId(u32::MAX),
        amount: 30000,
        equip: u32::MAX,
        identified: false,
        refine: 20,
        attribute: 1,
        cards: [u32::MAX, 0xFF, 4001, 1],
        options: [
            ItemOption {
                id: i16::MAX,
                value: i16::MIN,
                param: u8::MAX,
            },
            ItemOption {
                id: 1,
                value: -1,
                param: 0,
            },
            ItemOption::default(),
            ItemOption::default(),
            ItemOption {
                id: 5,
                value: 5,
                param: 5,
            },
        ],
        expire_time: u32::MAX,
        favorite: true,
        bound: 4,
        unique_id: 0x8000_0001_0000_0002,
        equip_switch: 0x8000_0000,
        enchant_grade: 4,
    }
}

/// Column names of `characters` in `schema`.
pub async fn character_columns(repo: &PostgresRepo, schema: &str) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT column_name::text FROM information_schema.columns \
         WHERE table_schema = $1 AND table_name = 'characters'",
    )
    .bind(schema)
    .fetch_all(repo.pool())
    .await
    .expect("columns")
}

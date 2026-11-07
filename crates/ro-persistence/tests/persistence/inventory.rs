//! Inventory, play state and looks (PER-INV-01..04, PER-EQ-01).

use ro_core::{
    CharId, CharacterInventory, CharacterLooks, CharacterProgress, CharacterStatus, InventoryItem,
    ItemId,
};
use ro_persistence::{
    AccountRepository, CharacterRepository, InventoryRepository, RepositoryError,
};

use crate::common::*;

/// PER-INV-01: a round trip keeps every field, including u32::MAX cards and a u64 unique_id
/// with the top bit set.
#[tokio::test]
async fn per_inv_01_round_trip_keeps_every_field() {
    let (repo, _acc, char_id) = inventory_fixture(0).await;
    let inventory = CharacterInventory {
        slots: vec![Some(full_item()), Some(InventoryItem::new(ItemId(501), 5))],
        unique_item_counter: 7,
    };
    repo.save_play_state(
        char_id,
        &inventory,
        &CharacterLooks::default(),
        &CharacterProgress::default(),
        &[],
    )
    .await
    .expect("saves");
    let state = repo
        .load_play_state(char_id)
        .await
        .expect("loads")
        .expect("character exists");
    assert_eq!(state.inventory, inventory);
}

/// PER-INV-02: holes compact on load and slot order is kept.
#[tokio::test]
async fn per_inv_02_holes_compact() {
    let (repo, _acc, char_id) = inventory_fixture(0).await;
    let a = InventoryItem::new(ItemId(909), 3);
    let b = InventoryItem::new(ItemId(501), 1);
    let c = InventoryItem::new(ItemId(1201), 1);
    let inventory = CharacterInventory {
        slots: vec![
            None,
            Some(a.clone()),
            None,
            None,
            Some(b.clone()),
            Some(c.clone()),
        ],
        unique_item_counter: 0,
    };
    repo.save_play_state(
        char_id,
        &inventory,
        &CharacterLooks::default(),
        &CharacterProgress::default(),
        &[],
    )
    .await
    .expect("saves");
    let loaded = repo
        .load_play_state(char_id)
        .await
        .expect("loads")
        .expect("exists")
        .inventory;
    assert_eq!(loaded.slots, vec![Some(a), Some(b), Some(c)]);
}

/// PER-INV-03: a save replaces the rows atomically; the counter is persisted.
#[tokio::test]
async fn per_inv_03_save_replaces_rows() {
    let (repo, _acc, char_id) = inventory_fixture(0).await;
    let first = CharacterInventory {
        slots: vec![Some(InventoryItem::new(ItemId(909), 1)); 4],
        unique_item_counter: 3,
    };
    repo.save_play_state(
        char_id,
        &first,
        &CharacterLooks::default(),
        &CharacterProgress::default(),
        &[],
    )
    .await
    .expect("saves");
    let second = CharacterInventory {
        slots: vec![Some(InventoryItem::new(ItemId(501), 2))],
        unique_item_counter: 9,
    };
    repo.save_play_state(
        char_id,
        &second,
        &CharacterLooks::default(),
        &CharacterProgress::default(),
        &[],
    )
    .await
    .expect("saves");
    let loaded = repo
        .load_play_state(char_id)
        .await
        .expect("loads")
        .expect("exists")
        .inventory;
    assert_eq!(loaded, second);

    let empty = CharacterInventory {
        slots: Vec::new(),
        unique_item_counter: 9,
    };
    repo.save_play_state(
        char_id,
        &empty,
        &CharacterLooks::default(),
        &CharacterProgress::default(),
        &[],
    )
    .await
    .expect("saves");
    let loaded = repo
        .load_play_state(char_id)
        .await
        .expect("loads")
        .expect("exists");
    assert_eq!(loaded.inventory, empty);

    assert_eq!(
        repo.save_play_state(
            CharId(u32::MAX >> 1),
            &second,
            &CharacterLooks::default(),
            &CharacterProgress::default(),
            &[]
        )
        .await,
        Err(RepositoryError::NotFound),
        "unknown character"
    );
}

/// PER-INV-04: the play state carries the stats, points, exp, HP/SP and the account group (and DB-PERS of
/// `specs/map-entry-settings` §3); deleting a character cascades.
#[tokio::test]
async fn per_inv_04_play_state_and_cascade() {
    let (repo, _acc, char_id) = inventory_fixture(99).await;
    let status = CharacterStatus {
        str: 11,
        agi: 12,
        vit: 13,
        int: 14,
        dex: 15,
        luk: 16,
        job_level: 8,
        status_point: 7,
        skill_point: 3,
        base_exp: 5_000_000_000,
        job_exp: 42,
        hp: 31,
        max_hp: 45,
        sp: 9,
        max_sp: 12,
        ..CharacterStatus::default()
    };
    let progress = CharacterProgress {
        job_class: 4307,
        base_level: 150,
        status,
        change_level: [0; 3],
    };
    let inventory = CharacterInventory {
        slots: vec![Some(InventoryItem::new(ItemId(909), 1))],
        unique_item_counter: 1,
    };
    // PER-STAT-01: the progress saves with the inventory and loads back.
    repo.save_play_state(
        char_id,
        &inventory,
        &CharacterLooks::default(),
        &progress,
        &[],
    )
    .await
    .expect("saves");
    let state = repo
        .load_play_state(char_id)
        .await
        .expect("loads")
        .expect("exists");
    assert_eq!(state.status, status);
    let summary = CharacterRepository::find_by_id(&repo, char_id)
        .await
        .expect("query")
        .expect("exists");
    assert_eq!((summary.job_class, summary.base_level), (4307, 150));
    assert_eq!(state.group_id, 99);

    sqlx::query("DELETE FROM characters WHERE char_id = $1")
        .bind(char_id.0 as i32)
        .execute(repo.pool())
        .await
        .expect("delete");
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inventory WHERE char_id = $1")
        .bind(char_id.0 as i32)
        .fetch_one(repo.pool())
        .await
        .expect("count");
    assert_eq!(rows, 0, "inventory rows cascade");
    assert_eq!(repo.load_play_state(char_id).await.expect("loads"), None);
}

/// The seed makes the `test` account a GM (group 99).
#[tokio::test]
async fn per_inv_04_seed_sets_test_account_group() {
    let repo = get_repo().await;
    repo.init_schema().await.expect("schema");
    repo.seed_test_data().await.expect("seed");
    let account = repo
        .find_by_username("test")
        .await
        .expect("query")
        .expect("seeded");
    assert_eq!(account.group_id, 99);
}

/// More rows than the inventory holds are refused, not silently dropped (the next save would
/// delete them); a save with too many slots is refused too.
#[tokio::test]
async fn per_inv_02_over_capacity_is_refused() {
    let (repo, _acc, char_id) = inventory_fixture(0).await;
    let full = CharacterInventory {
        slots: vec![Some(InventoryItem::new(ItemId(909), 1)); 100],
        unique_item_counter: 0,
    };
    repo.save_play_state(
        char_id,
        &full,
        &CharacterLooks::default(),
        &CharacterProgress::default(),
        &[],
    )
    .await
    .expect("100 fits");
    let too_many = CharacterInventory {
        slots: vec![Some(InventoryItem::new(ItemId(909), 1)); 101],
        unique_item_counter: 0,
    };
    assert!(matches!(
        repo.save_play_state(
            char_id,
            &too_many,
            &CharacterLooks::default(),
            &CharacterProgress::default(),
            &[]
        )
        .await,
        Err(RepositoryError::Internal(_))
    ));
    sqlx::query("INSERT INTO inventory (char_id, nameid, amount) VALUES ($1, 909, 1)")
        .bind(char_id.0 as i32)
        .execute(repo.pool())
        .await
        .expect("101st row by hand");
    assert!(matches!(
        repo.load_play_state(char_id).await,
        Err(RepositoryError::Internal(_))
    ));
}

/// Concurrent saves of one character serialize: the result is one whole inventory, never both.
#[tokio::test]
async fn per_inv_03_concurrent_saves_serialize() {
    let (repo, _acc, char_id) = inventory_fixture(0).await;
    let a = CharacterInventory {
        slots: vec![Some(InventoryItem::new(ItemId(501), 1)); 30],
        unique_item_counter: 1,
    };
    let b = CharacterInventory {
        slots: vec![Some(InventoryItem::new(ItemId(909), 1)); 40],
        unique_item_counter: 2,
    };
    let bare = CharacterLooks::default();
    let progress = CharacterProgress::default();
    for _ in 0..10 {
        let (ra, rb) = tokio::join!(
            repo.save_play_state(char_id, &a, &bare, &progress, &[]),
            repo.save_play_state(char_id, &b, &bare, &progress, &[])
        );
        ra.expect("save a");
        rb.expect("save b");
        let loaded = repo
            .load_play_state(char_id)
            .await
            .expect("loads")
            .expect("exists")
            .inventory;
        assert!(loaded == a || loaded == b, "{} rows", loaded.slots.len());
    }
}

// ---- Equipment looks (spec PER-EQ-01..02) ----

/// PER-EQ-01: `save_play_state` writes the look columns with the inventory, and every summary
/// query returns them. A new character starts bare.
#[tokio::test]
async fn per_eq_01_looks_saved_with_inventory() {
    let (repo, acc, char_id) = inventory_fixture(0).await;
    let created = CharacterRepository::find_by_id(&repo, char_id)
        .await
        .expect("loads")
        .expect("exists");
    assert_eq!(created.looks, CharacterLooks::default());

    let looks = CharacterLooks {
        weapon_type: 28,
        shield: 1,
        head_top: 16,
        head_mid: 3,
        head_bottom: 4,
        robe: 2,
    };
    let inventory = CharacterInventory {
        slots: vec![Some(InventoryItem::new(ItemId(1201), 1))],
        unique_item_counter: 1,
    };
    repo.save_play_state(
        char_id,
        &inventory,
        &looks,
        &CharacterProgress::default(),
        &[],
    )
    .await
    .expect("saves");

    let by_id = CharacterRepository::find_by_id(&repo, char_id)
        .await
        .expect("loads")
        .expect("exists");
    assert_eq!(by_id.looks, looks);
    let by_slot = repo
        .find_by_slot(acc.id, 0)
        .await
        .expect("loads")
        .expect("exists");
    assert_eq!(by_slot.looks, looks);
    let listed = repo.list_by_account(acc.id).await.expect("lists");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].looks, looks);

    // A refused save (too many slots) leaves the looks alone.
    let too_many = CharacterInventory {
        slots: vec![Some(InventoryItem::new(ItemId(909), 1)); 101],
        unique_item_counter: 0,
    };
    assert!(repo
        .save_play_state(
            char_id,
            &too_many,
            &CharacterLooks::default(),
            &CharacterProgress::default(),
            &[]
        )
        .await
        .is_err());
    let after = CharacterRepository::find_by_id(&repo, char_id)
        .await
        .expect("loads")
        .expect("exists");
    assert_eq!(after.looks, looks);
}

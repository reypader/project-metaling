#![allow(clippy::unwrap_used, clippy::expect_used)] // test helpers aren't covered by clippy.toml allow-*-in-tests

use std::{
    net::SocketAddr,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use bytes::BytesMut;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tokio_util::codec::Decoder;

use char_server::{handle_client_session, CharServiceHandle};
use ro_core::{
    character::CharacterRepository,
    ids::{AccountId, Sex},
    CreateCharacterRequest, TARGET_PACKETVER,
};
use ro_packets::PacketOpcode;
use ro_packets::{PacketDecoder, RathenaPacketLengthTable};
use ro_persistence::PostgresRepo;

async fn setup_test_repo() -> Arc<dyn CharacterRepository> {
    let test_db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://ragnarok:ragnarok@localhost:5432/ragnarok".to_string());
    let repo = PostgresRepo::connect(&test_db_url).await.unwrap();
    let _ = repo.seed_test_data().await;
    Arc::new(repo)
}

#[tokio::test]
async fn test_char_handshake_via_channel_service() {
    let char_repo = setup_test_repo().await;

    if char_repo
        .find_by_slot(AccountId(2000000), 0)
        .await
        .unwrap()
        .is_none()
    {
        let unique_suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis()
            % 1000000;
        let req = CreateCharacterRequest {
            account_id: AccountId(2000000),
            slot: 0,
            name: format!("Hero_{}", unique_suffix),
            hair_color: 1,
            hair_style: 1,
            starting_job: 0,
            sex: Sex::Male,
        };
        char_repo
            .create_character(req, 15)
            .await
            .expect("Failed to create character in slot 0");
    }

    let service_handle = CharServiceHandle::new(char_repo);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_addr: SocketAddr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        let (socket, peer_addr) = listener.accept().await.unwrap();
        handle_client_session(socket, peer_addr, service_handle).await;
    });

    let mut client = TcpStream::connect(server_addr).await.unwrap();

    let mut ch_enter_bytes = vec![0x65, 0x00];
    ch_enter_bytes.extend_from_slice(&2000000u32.to_le_bytes());
    ch_enter_bytes.extend_from_slice(&1u32.to_le_bytes());
    ch_enter_bytes.extend_from_slice(&2u32.to_le_bytes());
    ch_enter_bytes.extend_from_slice(&20u16.to_le_bytes());
    ch_enter_bytes.push(1);

    client.write_all(&ch_enter_bytes).await.unwrap();
    client.flush().await.unwrap();

    let mut account_id_buf = [0u8; 4];
    client.read_exact(&mut account_id_buf).await.unwrap();
    assert_eq!(u32::from_le_bytes(account_id_buf), 2000000);

    let mut decoder = PacketDecoder::new(RathenaPacketLengthTable, TARGET_PACKETVER);
    let mut buffer = BytesMut::with_capacity(2048);

    let mut received_opcodes = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while received_opcodes.len() < 5 && tokio::time::Instant::now() < deadline {
        if let Ok(Ok(n)) =
            tokio::time::timeout(Duration::from_millis(100), client.read_buf(&mut buffer)).await
        {
            if n == 0 {
                break;
            }
            while let Some(raw) = decoder.decode(&mut buffer).unwrap() {
                received_opcodes.push(raw.opcode);
            }
        }
    }
    assert!(
        received_opcodes.len() >= 5,
        "Timed out waiting for initial opcodes"
    );
    assert_eq!(received_opcodes[0], PacketOpcode::HC_ACCEPT_ENTER2);
    assert_eq!(received_opcodes[1], PacketOpcode::HC_ACCEPT_ENTER);
    assert_eq!(received_opcodes[2], PacketOpcode(0x09a0));
    assert_eq!(received_opcodes[3], PacketOpcode::HC_BLOCK_CHARACTER);
    assert_eq!(received_opcodes[4], PacketOpcode::HC_SECOND_PASSWD_LOGIN);

    let ch_select_bytes = vec![0x66, 0x00, 0x00];
    client.write_all(&ch_select_bytes).await.unwrap();
    client.flush().await.unwrap();

    let mut notify_raw = None;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while notify_raw.is_none() && tokio::time::Instant::now() < deadline {
        if let Ok(Ok(n)) =
            tokio::time::timeout(Duration::from_millis(100), client.read_buf(&mut buffer)).await
        {
            if n == 0 {
                break;
            }
            while let Some(raw) = decoder.decode(&mut buffer).unwrap() {
                if raw.opcode == PacketOpcode::HC_NOTIFY_ZONESVR {
                    notify_raw = Some(raw);
                    break;
                }
            }
        }
    }
    let raw = notify_raw.expect("expected HC_NOTIFY_ZONESVR");
    assert_eq!(raw.opcode, PacketOpcode::HC_NOTIFY_ZONESVR);
}

#[tokio::test]
async fn test_full_login_sequence_char_flow() {
    let test_db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://ragnarok:ragnarok@localhost:5432/ragnarok".to_string());
    if let Ok(repo) = PostgresRepo::connect(&test_db_url).await {
        let _ = sqlx::query("DELETE FROM characters WHERE account_id = $1 AND slot = $2")
            .bind(2000000i32)
            .bind(4i16)
            .execute(repo.pool())
            .await;
    }

    let char_repo = setup_test_repo().await;
    let service_handle = CharServiceHandle::new(char_repo.clone());

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_addr: SocketAddr = listener.local_addr().unwrap();

    let srv = service_handle.clone();
    tokio::spawn(async move {
        while let Ok((socket, peer_addr)) = listener.accept().await {
            tokio::spawn(handle_client_session(socket, peer_addr, srv.clone()));
        }
    });

    let mut client1 = TcpStream::connect(server_addr).await.unwrap();

    let mut ch_enter_bytes = vec![0x65, 0x00];
    ch_enter_bytes.extend_from_slice(&2000000u32.to_le_bytes());
    ch_enter_bytes.extend_from_slice(&1u32.to_le_bytes());
    ch_enter_bytes.extend_from_slice(&2u32.to_le_bytes());
    ch_enter_bytes.extend_from_slice(&20u16.to_le_bytes());
    ch_enter_bytes.push(1);

    client1.write_all(&ch_enter_bytes).await.unwrap();
    client1.flush().await.unwrap();

    let mut account_id_buf = [0u8; 4];
    client1.read_exact(&mut account_id_buf).await.unwrap();
    assert_eq!(u32::from_le_bytes(account_id_buf), 2000000);

    let mut decoder = PacketDecoder::new(RathenaPacketLengthTable, TARGET_PACKETVER);
    let mut buffer = BytesMut::with_capacity(4096);

    let mut enter_opcodes = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while enter_opcodes.len() < 5 && tokio::time::Instant::now() < deadline {
        if let Ok(Ok(n)) =
            tokio::time::timeout(Duration::from_millis(100), client1.read_buf(&mut buffer)).await
        {
            if n == 0 {
                break;
            }
            while let Some(raw) = decoder.decode(&mut buffer).unwrap() {
                enter_opcodes.push(raw.opcode);
            }
        }
    }
    assert_eq!(
        enter_opcodes.len(),
        5,
        "Timed out waiting for enter opcodes"
    );
    assert_eq!(enter_opcodes[0], PacketOpcode::HC_ACCEPT_ENTER2);
    assert_eq!(enter_opcodes[1], PacketOpcode::HC_ACCEPT_ENTER);
    assert_eq!(enter_opcodes[2], PacketOpcode(0x09a0));
    assert_eq!(enter_opcodes[3], PacketOpcode::HC_BLOCK_CHARACTER);
    assert_eq!(enter_opcodes[4], PacketOpcode::HC_SECOND_PASSWD_LOGIN);

    client1.write_all(&[0xa1, 0x09]).await.unwrap();
    client1.flush().await.unwrap();

    let mut charlist_ack = None;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while charlist_ack.is_none() && tokio::time::Instant::now() < deadline {
        if let Ok(Ok(n)) =
            tokio::time::timeout(Duration::from_millis(100), client1.read_buf(&mut buffer)).await
        {
            if n == 0 {
                break;
            }
            while let Some(raw) = decoder.decode(&mut buffer).unwrap() {
                if raw.opcode == PacketOpcode(0x0b72) {
                    charlist_ack = Some(raw);
                    break;
                }
            }
        }
    }
    assert!(charlist_ack.is_some(), "Timed out waiting for charlist_ack");

    let unique_id = (SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
        % 1000000) as u32;
    let created_name = format!("tst_{}", unique_id);
    let mut make_char_bytes = vec![0x39, 0x0a];
    let mut name_bytes = [0u8; 24];
    name_bytes[..created_name.len()].copy_from_slice(created_name.as_bytes());
    make_char_bytes.extend_from_slice(&name_bytes);
    make_char_bytes.push(4);
    make_char_bytes.extend_from_slice(&0u16.to_le_bytes());
    make_char_bytes.extend_from_slice(&22u16.to_le_bytes());
    make_char_bytes.extend_from_slice(&0u32.to_le_bytes());
    make_char_bytes.push(0);

    client1.write_all(&make_char_bytes).await.unwrap();
    client1.flush().await.unwrap();

    let mut make_char_ack = None;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while make_char_ack.is_none() && tokio::time::Instant::now() < deadline {
        if let Ok(Ok(n)) =
            tokio::time::timeout(Duration::from_millis(100), client1.read_buf(&mut buffer)).await
        {
            if n == 0 {
                break;
            }
            while let Some(raw) = decoder.decode(&mut buffer).unwrap() {
                if raw.opcode == PacketOpcode::HC_ACCEPT_MAKECHAR {
                    make_char_ack = Some(raw);
                    break;
                }
            }
        }
    }
    let ack = make_char_ack.expect("expected HC_ACCEPT_MAKECHAR 0x0b6f");
    assert_eq!(ack.opcode, PacketOpcode::HC_ACCEPT_MAKECHAR);

    let ch_select_bytes = vec![0x66, 0x00, 0x04];
    client1.write_all(&ch_select_bytes).await.unwrap();
    client1.flush().await.unwrap();

    let mut notify_raw = None;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while notify_raw.is_none() && tokio::time::Instant::now() < deadline {
        if let Ok(Ok(n)) =
            tokio::time::timeout(Duration::from_millis(100), client1.read_buf(&mut buffer)).await
        {
            if n == 0 {
                break;
            }
            while let Some(raw) = decoder.decode(&mut buffer).unwrap() {
                if raw.opcode == PacketOpcode::HC_NOTIFY_ZONESVR {
                    notify_raw = Some(raw);
                    break;
                }
            }
        }
    }
    let raw = notify_raw.expect("expected HC_NOTIFY_ZONESVR 0x0ac5");
    assert_eq!(raw.opcode, PacketOpcode::HC_NOTIFY_ZONESVR);

    // The client initiates connection closure after receiving HC_NOTIFY_ZONESVR.
    drop(client1);

    let mut client2 = TcpStream::connect(server_addr).await.unwrap();
    let mut buffer2 = BytesMut::with_capacity(4096);

    client2.write_all(&ch_enter_bytes).await.unwrap();
    client2.flush().await.unwrap();

    let mut account_id_buf2 = [0u8; 4];
    client2.read_exact(&mut account_id_buf2).await.unwrap();
    assert_eq!(u32::from_le_bytes(account_id_buf2), 2000000);

    let mut enter_opcodes2 = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while enter_opcodes2.len() < 5 && tokio::time::Instant::now() < deadline {
        if let Ok(Ok(n)) =
            tokio::time::timeout(Duration::from_millis(100), client2.read_buf(&mut buffer2)).await
        {
            if n == 0 {
                break;
            }
            while let Some(raw) = decoder.decode(&mut buffer2).unwrap() {
                enter_opcodes2.push(raw.opcode);
            }
        }
    }
    assert_eq!(
        enter_opcodes2.len(),
        5,
        "Timed out waiting for client2 enter opcodes"
    );
    assert_eq!(enter_opcodes2[0], PacketOpcode::HC_ACCEPT_ENTER2);
    assert_eq!(enter_opcodes2[1], PacketOpcode::HC_ACCEPT_ENTER);
    assert_eq!(enter_opcodes2[2], PacketOpcode(0x09a0));
    assert_eq!(enter_opcodes2[3], PacketOpcode::HC_BLOCK_CHARACTER);
    assert_eq!(enter_opcodes2[4], PacketOpcode::HC_SECOND_PASSWD_LOGIN);

    client2.write_all(&[0xa1, 0x09]).await.unwrap();
    client2.flush().await.unwrap();

    let mut charlist_ack2 = None;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while charlist_ack2.is_none() && tokio::time::Instant::now() < deadline {
        if let Ok(Ok(n)) =
            tokio::time::timeout(Duration::from_millis(100), client2.read_buf(&mut buffer2)).await
        {
            if n == 0 {
                break;
            }
            while let Some(raw) = decoder.decode(&mut buffer2).unwrap() {
                if raw.opcode == PacketOpcode(0x0b72) {
                    charlist_ack2 = Some(raw);
                    break;
                }
            }
        }
    }
    assert!(
        charlist_ack2.is_some(),
        "Timed out waiting for client2 charlist_ack"
    );

    let chars = char_repo.list_by_account(AccountId(2000000)).await.unwrap();
    let test5_char = chars
        .iter()
        .find(|c| c.name == created_name)
        .expect("created character should exist");
    assert_eq!(test5_char.slot, 4);
    let test5_char_id = test5_char.char_id;

    let mut del_reserve_bytes = vec![0x27, 0x08];
    del_reserve_bytes.extend_from_slice(&test5_char_id.0.to_le_bytes());

    client2.write_all(&del_reserve_bytes).await.unwrap();
    client2.flush().await.unwrap();

    let mut del_ack = None;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while del_ack.is_none() && tokio::time::Instant::now() < deadline {
        if let Ok(Ok(n)) =
            tokio::time::timeout(Duration::from_millis(100), client2.read_buf(&mut buffer2)).await
        {
            if n == 0 {
                break;
            }
            while let Some(raw) = decoder.decode(&mut buffer2).unwrap() {
                if raw.opcode == PacketOpcode::HC_DELETE_CHAR3_RESERVED {
                    del_ack = Some(raw);
                    break;
                }
            }
        }
    }
    let del_raw = del_ack.expect("expected HC_DELETE_CHAR3_RESERVED 0x0828");
    assert_eq!(del_raw.opcode, PacketOpcode::HC_DELETE_CHAR3_RESERVED);
    assert_eq!(del_raw.payload.len(), 12);
    let returned_char_id = u32::from_le_bytes(del_raw.payload[0..4].try_into().unwrap());
    let returned_result = u32::from_le_bytes(del_raw.payload[4..8].try_into().unwrap());
    let returned_delay = u32::from_le_bytes(del_raw.payload[8..12].try_into().unwrap());
    assert_eq!(returned_char_id, test5_char_id.0);
    assert_eq!(returned_result, 1);
    assert_eq!(returned_delay, 86400);

    let mut del_cancel_bytes = vec![0x2b, 0x08];
    del_cancel_bytes.extend_from_slice(&test5_char_id.0.to_le_bytes());

    client2.write_all(&del_cancel_bytes).await.unwrap();
    client2.flush().await.unwrap();

    let mut cancel_ack = None;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while cancel_ack.is_none() && tokio::time::Instant::now() < deadline {
        if let Ok(Ok(n)) =
            tokio::time::timeout(Duration::from_millis(100), client2.read_buf(&mut buffer2)).await
        {
            if n == 0 {
                break;
            }
            while let Some(raw) = decoder.decode(&mut buffer2).unwrap() {
                if raw.opcode == PacketOpcode::HC_DELETE_CHAR3_CANCEL {
                    cancel_ack = Some(raw);
                    break;
                }
            }
        }
    }
    let cancel_raw = cancel_ack.expect("expected HC_DELETE_CHAR3_CANCEL 0x082c");
    assert_eq!(cancel_raw.opcode, PacketOpcode::HC_DELETE_CHAR3_CANCEL);
    let returned_cancel_char_id = u32::from_le_bytes(cancel_raw.payload[0..4].try_into().unwrap());
    let returned_cancel_result = u32::from_le_bytes(cancel_raw.payload[4..8].try_into().unwrap());
    assert_eq!(returned_cancel_char_id, test5_char_id.0);
    assert_eq!(returned_cancel_result, 1);
}

#[tokio::test]
async fn test_bin_cfg_persist_002_char_startup_failure_on_unreachable_db() {
    let unreachable_url = "postgres://invalid:invalid@127.0.0.1:5433/nonexistent";
    let connect_result = PostgresRepo::connect(unreachable_url).await;
    assert!(
        connect_result.is_err(),
        "Expected connection to unreachable DB to fail fast"
    );
}

#[tokio::test]
async fn test_bin_char_persist_lifecycle() {
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://ragnarok:ragnarok@localhost:5432/ragnarok".to_string());
    let repo = match PostgresRepo::connect(&db_url).await {
        Ok(r) => r,
        Err(err) => {
            eprintln!(
                "Skipping test_bin_char_persist_lifecycle: DB unavailable ({})",
                err
            );
            return;
        }
    };
    let _ = repo.seed_test_data().await;

    let _ = sqlx::query("DELETE FROM characters WHERE account_id = $1 AND slot = $2")
        .bind(2000000i32)
        .bind(2i16)
        .execute(repo.pool())
        .await;

    let unique_id = (SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
        % 1000000) as u32;
    let persist_name = format!("per_{}", unique_id);

    let service_handle = CharServiceHandle::new(Arc::new(repo.clone()));

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_addr: SocketAddr = listener.local_addr().unwrap();

    let srv = service_handle.clone();
    tokio::spawn(async move {
        while let Ok((socket, peer_addr)) = listener.accept().await {
            tokio::spawn(handle_client_session(socket, peer_addr, srv.clone()));
        }
    });

    // Client session 1
    let mut client1 = TcpStream::connect(server_addr).await.unwrap();

    let mut ch_enter_bytes = vec![0x65, 0x00];
    ch_enter_bytes.extend_from_slice(&2000000u32.to_le_bytes());
    ch_enter_bytes.extend_from_slice(&1u32.to_le_bytes());
    ch_enter_bytes.extend_from_slice(&2u32.to_le_bytes());
    ch_enter_bytes.extend_from_slice(&20u16.to_le_bytes());
    ch_enter_bytes.push(1);

    client1.write_all(&ch_enter_bytes).await.unwrap();
    client1.flush().await.unwrap();

    let mut account_id_buf = [0u8; 4];
    client1.read_exact(&mut account_id_buf).await.unwrap();
    assert_eq!(u32::from_le_bytes(account_id_buf), 2000000);

    let mut decoder = PacketDecoder::new(RathenaPacketLengthTable, TARGET_PACKETVER);
    let mut buffer = BytesMut::with_capacity(4096);

    let mut enter_opcodes = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while enter_opcodes.len() < 5 && tokio::time::Instant::now() < deadline {
        if let Ok(Ok(n)) =
            tokio::time::timeout(Duration::from_millis(100), client1.read_buf(&mut buffer)).await
        {
            if n == 0 {
                break;
            }
            while let Some(raw) = decoder.decode(&mut buffer).unwrap() {
                enter_opcodes.push(raw.opcode);
            }
        }
    }
    assert_eq!(
        enter_opcodes.len(),
        5,
        "Timed out waiting for enter opcodes"
    );
    assert_eq!(enter_opcodes[0], PacketOpcode::HC_ACCEPT_ENTER2);

    // Request character list
    client1.write_all(&[0xa1, 0x09]).await.unwrap();
    client1.flush().await.unwrap();

    let mut charlist_ack = None;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while charlist_ack.is_none() && tokio::time::Instant::now() < deadline {
        if let Ok(Ok(n)) =
            tokio::time::timeout(Duration::from_millis(100), client1.read_buf(&mut buffer)).await
        {
            if n == 0 {
                break;
            }
            while let Some(raw) = decoder.decode(&mut buffer).unwrap() {
                if raw.opcode == PacketOpcode(0x0b72) {
                    charlist_ack = Some(raw);
                    break;
                }
            }
        }
    }
    assert!(charlist_ack.is_some(), "Timed out waiting for charlist_ack");

    // Create character in slot 2
    let mut make_char_bytes = vec![0x39, 0x0a];
    let mut name_bytes = [0u8; 24];
    name_bytes[..persist_name.len()].copy_from_slice(persist_name.as_bytes());
    make_char_bytes.extend_from_slice(&name_bytes);
    make_char_bytes.push(2); // slot 2
    make_char_bytes.extend_from_slice(&0u16.to_le_bytes());
    make_char_bytes.extend_from_slice(&22u16.to_le_bytes());
    make_char_bytes.extend_from_slice(&0u32.to_le_bytes());
    make_char_bytes.push(0);

    client1.write_all(&make_char_bytes).await.unwrap();
    client1.flush().await.unwrap();

    let mut make_char_ack = None;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while make_char_ack.is_none() && tokio::time::Instant::now() < deadline {
        if let Ok(Ok(n)) =
            tokio::time::timeout(Duration::from_millis(100), client1.read_buf(&mut buffer)).await
        {
            if n == 0 {
                break;
            }
            while let Some(raw) = decoder.decode(&mut buffer).unwrap() {
                if raw.opcode == PacketOpcode::HC_ACCEPT_MAKECHAR {
                    make_char_ack = Some(raw);
                    break;
                }
            }
        }
    }
    let ack = make_char_ack.expect("expected HC_ACCEPT_MAKECHAR 0x0b6f");
    assert_eq!(ack.opcode, PacketOpcode::HC_ACCEPT_MAKECHAR);

    // Verify created character in PostgreSQL repository
    let chars = repo.list_by_account(AccountId(2000000)).await.unwrap();
    let persist_char = chars
        .iter()
        .find(|c| c.name == persist_name)
        .expect("persist char should exist");
    assert_eq!(persist_char.slot, 2);
    let created_char_id = persist_char.char_id;

    // Reserve deletion
    let mut del_reserve_bytes = vec![0x27, 0x08];
    del_reserve_bytes.extend_from_slice(&created_char_id.0.to_le_bytes());
    client1.write_all(&del_reserve_bytes).await.unwrap();
    client1.flush().await.unwrap();

    let mut del_ack = None;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while del_ack.is_none() && tokio::time::Instant::now() < deadline {
        if let Ok(Ok(n)) =
            tokio::time::timeout(Duration::from_millis(100), client1.read_buf(&mut buffer)).await
        {
            if n == 0 {
                break;
            }
            while let Some(raw) = decoder.decode(&mut buffer).unwrap() {
                if raw.opcode == PacketOpcode::HC_DELETE_CHAR3_RESERVED {
                    del_ack = Some(raw);
                    break;
                }
            }
        }
    }
    let del_raw = del_ack.expect("expected HC_DELETE_CHAR3_RESERVED 0x0828");
    assert_eq!(del_raw.opcode, PacketOpcode::HC_DELETE_CHAR3_RESERVED);

    // Cancel deletion
    let mut del_cancel_bytes = vec![0x2b, 0x08];
    del_cancel_bytes.extend_from_slice(&created_char_id.0.to_le_bytes());
    client1.write_all(&del_cancel_bytes).await.unwrap();
    client1.flush().await.unwrap();

    let mut cancel_ack = None;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while cancel_ack.is_none() && tokio::time::Instant::now() < deadline {
        if let Ok(Ok(n)) =
            tokio::time::timeout(Duration::from_millis(100), client1.read_buf(&mut buffer)).await
        {
            if n == 0 {
                break;
            }
            while let Some(raw) = decoder.decode(&mut buffer).unwrap() {
                if raw.opcode == PacketOpcode::HC_DELETE_CHAR3_CANCEL {
                    cancel_ack = Some(raw);
                    break;
                }
            }
        }
    }
    let cancel_raw = cancel_ack.expect("expected HC_DELETE_CHAR3_CANCEL 0x082c");
    assert_eq!(cancel_raw.opcode, PacketOpcode::HC_DELETE_CHAR3_CANCEL);

    // Select character in slot 2
    let ch_select_bytes = vec![0x66, 0x00, 0x02];
    client1.write_all(&ch_select_bytes).await.unwrap();
    client1.flush().await.unwrap();

    let mut notify_raw = None;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while notify_raw.is_none() && tokio::time::Instant::now() < deadline {
        if let Ok(Ok(n)) =
            tokio::time::timeout(Duration::from_millis(100), client1.read_buf(&mut buffer)).await
        {
            if n == 0 {
                break;
            }
            while let Some(raw) = decoder.decode(&mut buffer).unwrap() {
                if raw.opcode == PacketOpcode::HC_NOTIFY_ZONESVR {
                    notify_raw = Some(raw);
                    break;
                }
            }
        }
    }
    let raw = notify_raw.expect("expected HC_NOTIFY_ZONESVR 0x0ac5");
    assert_eq!(raw.opcode, PacketOpcode::HC_NOTIFY_ZONESVR);

    // Disconnect client 1
    drop(client1);

    // Reconnect with client 2 and verify persistence of created character
    let mut client2 = TcpStream::connect(server_addr).await.unwrap();
    let mut buffer2 = BytesMut::with_capacity(4096);

    client2.write_all(&ch_enter_bytes).await.unwrap();
    client2.flush().await.unwrap();

    let mut account_id_buf2 = [0u8; 4];
    client2.read_exact(&mut account_id_buf2).await.unwrap();
    assert_eq!(u32::from_le_bytes(account_id_buf2), 2000000);

    let mut enter_opcodes2 = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while enter_opcodes2.len() < 5 && tokio::time::Instant::now() < deadline {
        if let Ok(Ok(n)) =
            tokio::time::timeout(Duration::from_millis(100), client2.read_buf(&mut buffer2)).await
        {
            if n == 0 {
                break;
            }
            while let Some(raw) = decoder.decode(&mut buffer2).unwrap() {
                enter_opcodes2.push(raw.opcode);
            }
        }
    }
    assert_eq!(
        enter_opcodes2.len(),
        5,
        "Timed out waiting for client2 enter opcodes"
    );
    assert_eq!(enter_opcodes2[0], PacketOpcode::HC_ACCEPT_ENTER2);

    let chars_reconnect = repo.list_by_account(AccountId(2000000)).await.unwrap();
    assert!(chars_reconnect
        .iter()
        .any(|c| c.name == persist_name && c.slot == 2));
}

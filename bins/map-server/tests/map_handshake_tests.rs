#![allow(clippy::unwrap_used, clippy::expect_used)] // test helpers aren't covered by clippy.toml allow-*-in-tests

use std::sync::Arc;

use axum::{extract::State, Json};
use bytes::BytesMut;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tokio_util::codec::Decoder;

use map_server::{accept_transfer, handle_client_session, MapServiceHandle};
use ro_contracts::{
    character::CharacterRepository,
    ids::{AccountId, CharId, MapPosition},
    packet::PacketOpcode,
    PrepareMapTransferRequest, TARGET_PACKETVER,
};
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
async fn test_map_handshake_enter_actorinit_and_time() {
    let service_handle = MapServiceHandle::new(setup_test_repo().await);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        let (socket, peer_addr) = listener.accept().await.unwrap();
        handle_client_session(socket, peer_addr, service_handle).await;
    });

    let mut client = TcpStream::connect(server_addr).await.unwrap();

    let mut cz_enter_bytes = vec![0x36, 0x04];
    cz_enter_bytes.extend_from_slice(&2000000u32.to_le_bytes());
    cz_enter_bytes.extend_from_slice(&150001u32.to_le_bytes());
    cz_enter_bytes.extend_from_slice(&1u32.to_le_bytes());
    cz_enter_bytes.extend_from_slice(&12345u32.to_le_bytes());
    cz_enter_bytes.extend_from_slice(&[0u8; 4]);
    cz_enter_bytes.push(1);

    client.write_all(&cz_enter_bytes).await.unwrap();
    client.flush().await.unwrap();

    let mut decoder = PacketDecoder::new(RathenaPacketLengthTable, TARGET_PACKETVER);
    let mut buffer = BytesMut::with_capacity(2048);

    let mut burst_packets = Vec::new();
    while burst_packets.len() < 3 {
        let n = client.read_buf(&mut buffer).await.unwrap();
        assert!(n > 0);
        while let Some(raw) = decoder.decode(&mut buffer).unwrap() {
            burst_packets.push(raw);
        }
    }
    assert_eq!(burst_packets[0].opcode, PacketOpcode::ZC_AID);
    assert_eq!(&burst_packets[0].payload[0..4], &2000000u32.to_le_bytes());
    assert_eq!(
        burst_packets[1].opcode,
        PacketOpcode::ZC_INVENTORY_EXPANSION_INFO
    );
    assert_eq!(burst_packets[2].opcode, PacketOpcode::ZC_ACCEPT_ENTER);

    client.write_all(&[0x7d, 0x00]).await.unwrap();
    client.flush().await.unwrap();

    let mut init_opcodes = Vec::new();
    while init_opcodes.len() < 2 {
        let n = client.read_buf(&mut buffer).await.unwrap();
        assert!(n > 0);
        while let Some(raw) = decoder.decode(&mut buffer).unwrap() {
            init_opcodes.push(raw.opcode);
        }
    }
    assert_eq!(init_opcodes[0], PacketOpcode::ZC_NOTIFY_NEWENTRY11);
    assert_eq!(init_opcodes[1], PacketOpcode::ZC_PAR_CHANGE);

    let mut cz_time_bytes = vec![0x60, 0x03];
    cz_time_bytes.extend_from_slice(&55555u32.to_le_bytes());
    client.write_all(&cz_time_bytes).await.unwrap();
    client.flush().await.unwrap();

    let mut time_raw = None;
    while time_raw.is_none() {
        let n = client.read_buf(&mut buffer).await.unwrap();
        assert!(n > 0);
        if let Some(raw) = decoder.decode(&mut buffer).unwrap() {
            time_raw = Some(raw);
        }
    }
    let raw = time_raw.unwrap();
    assert_eq!(raw.opcode, PacketOpcode::ZC_NOTIFY_TIME);
}

#[tokio::test]
async fn test_map_restart_to_char_select() {
    let service_handle = MapServiceHandle::new(setup_test_repo().await);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        let (socket, peer_addr) = listener.accept().await.unwrap();
        handle_client_session(socket, peer_addr, service_handle).await;
    });

    let mut client = TcpStream::connect(server_addr).await.unwrap();

    let mut cz_enter_bytes = vec![0x36, 0x04];
    cz_enter_bytes.extend_from_slice(&2000000u32.to_le_bytes());
    cz_enter_bytes.extend_from_slice(&150002u32.to_le_bytes());
    cz_enter_bytes.extend_from_slice(&1u32.to_le_bytes());
    cz_enter_bytes.extend_from_slice(&12345u32.to_le_bytes());
    cz_enter_bytes.extend_from_slice(&[0u8; 4]);
    cz_enter_bytes.push(0);

    client.write_all(&cz_enter_bytes).await.unwrap();
    client.flush().await.unwrap();

    let mut decoder = PacketDecoder::new(RathenaPacketLengthTable, TARGET_PACKETVER);
    let mut buffer = BytesMut::with_capacity(2048);

    let mut burst_packets = Vec::new();
    while burst_packets.len() < 3 {
        let n = client.read_buf(&mut buffer).await.unwrap();
        assert!(n > 0);
        while let Some(raw) = decoder.decode(&mut buffer).unwrap() {
            burst_packets.push(raw);
        }
    }
    assert_eq!(burst_packets[0].opcode, PacketOpcode::ZC_AID);
    assert_eq!(&burst_packets[0].payload[0..4], &2000000u32.to_le_bytes());
    assert_eq!(
        burst_packets[1].opcode,
        PacketOpcode::ZC_INVENTORY_EXPANSION_INFO
    );
    assert_eq!(burst_packets[2].opcode, PacketOpcode::ZC_ACCEPT_ENTER);

    let restart_bytes = vec![0xb2, 0x00, 0x01];
    client.write_all(&restart_bytes).await.unwrap();
    client.flush().await.unwrap();

    let mut ack_raw = None;
    while ack_raw.is_none() {
        let n = client.read_buf(&mut buffer).await.unwrap();
        assert!(n > 0);
        if let Some(raw) = decoder.decode(&mut buffer).unwrap() {
            ack_raw = Some(raw);
        }
    }
    let raw = ack_raw.unwrap();
    assert_eq!(raw.opcode, PacketOpcode::ZC_RESTART_ACK);
    assert_eq!(raw.payload.as_ref(), &[1]);
}

#[tokio::test]
async fn test_accept_transfer_validation() {
    let service = MapServiceHandle::new(setup_test_repo().await);

    let valid_req = PrepareMapTransferRequest {
        token: "valid_token".to_string(),
        account_id: AccountId(2000000),
        char_id: CharId(150001),
        login_id1: 1,
        destination_map: "morocc.gat".to_string(),
        destination_pos: MapPosition {
            x: 100,
            y: 100,
            dir: 0,
        },
        character: None,
    };
    let res = accept_transfer(State(service.clone()), Json(valid_req))
        .await
        .0;
    assert!(res.accepted);

    let invalid_cases = vec![
        "",
        "morocc",
        "morocc.txt",
        "../prontera.gat",
        "invalid/map.gat",
        "a_very_long_map_name_that_exceeds_limits_and_is_not_standard.gat",
    ];

    for invalid_map in invalid_cases {
        let req = PrepareMapTransferRequest {
            token: "token".to_string(),
            account_id: AccountId(2000000),
            char_id: CharId(150002),
            login_id1: 1,
            destination_map: invalid_map.to_string(),
            destination_pos: MapPosition {
                x: 100,
                y: 100,
                dir: 0,
            },
            character: None,
        };
        let res = accept_transfer(State(service.clone()), Json(req)).await.0;
        assert!(
            !res.accepted,
            "Expected map '{}' to be rejected",
            invalid_map
        );
    }
}

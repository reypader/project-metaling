#![allow(clippy::unwrap_used, clippy::expect_used)] // test helpers aren't covered by clippy.toml allow-*-in-tests

use std::time::Duration;

use bytes::BytesMut;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tokio_util::codec::Decoder;

use map_server::{handle_client_session, map_data_loader, MapServiceHandle};
use ro_core::TARGET_PACKETVER;
use ro_packets::PacketOpcode;
use ro_packets::{PacketDecoder, RathenaPacketLengthTable};

#[tokio::test]
async fn test_bin_cfg_persist_002_map_startup_failure_on_unreachable_db() {
    let unreachable_url = "postgres://invalid:invalid@127.0.0.1:5433/nonexistent";
    let connect_result = ro_persistence::PostgresRepo::connect(unreachable_url).await;
    assert!(
        connect_result.is_err(),
        "Expected connection to unreachable DB to fail fast"
    );
}

#[tokio::test]
async fn test_bin_map_persist_lifecycle_and_coordinates() {
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://ragnarok:ragnarok@localhost:5432/ragnarok".to_string());
    let repo = ro_persistence::PostgresRepo::connect(&db_url).await.expect(
        "database must be running (docker compose -f tools/docker/docker-compose.yml up -d db)",
    );
    let _ = repo.seed_test_data().await;

    use ro_core::character::{CharacterRepository, CreateCharacterRequest};
    use ro_core::ids::{AccountId, MapPosition, Sex};

    let _ = sqlx::query("DELETE FROM characters WHERE account_id = $1 AND slot = $2")
        .bind(2000000i32)
        .bind(3i16)
        .execute(repo.pool())
        .await;

    let unique_suffix = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
        % 1000000) as u32;
    let create_req = CreateCharacterRequest {
        account_id: AccountId(2000000),
        name: format!("hero_{}", unique_suffix),
        slot: 3,
        hair_color: 1,
        hair_style: 1,
        starting_job: 0,
        sex: Sex::Male,
    };
    let created = repo
        .create_character(create_req, 15)
        .await
        .expect("Failed to create test character");
    let char_id = created.char_id;

    let custom_pos = MapPosition {
        x: 120,
        y: 140,
        dir: 4,
    };
    repo.update_position(char_id, "prontera.gat", custom_pos)
        .await
        .expect("Failed to set position");

    let repo_arc = std::sync::Arc::new(repo);
    let service = MapServiceHandle::new(repo_arc.clone(), map_data_loader(None));

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let srv = service.clone();

    tokio::spawn(async move {
        while let Ok((socket, peer_addr)) = listener.accept().await {
            tokio::spawn(handle_client_session(socket, peer_addr, srv.clone()));
        }
    });

    let mut client = TcpStream::connect(addr).await.unwrap();
    let mut cz_enter = Vec::with_capacity(23);
    cz_enter.extend_from_slice(&[0x36, 0x04]);
    cz_enter.extend_from_slice(&2000000u32.to_le_bytes());
    cz_enter.extend_from_slice(&char_id.0.to_le_bytes());
    cz_enter.extend_from_slice(&12345u32.to_le_bytes());
    cz_enter.extend_from_slice(&100000u32.to_le_bytes());
    cz_enter.extend_from_slice(&[0u8; 4]);
    cz_enter.push(1);
    client.write_all(&cz_enter).await.unwrap();
    client.flush().await.unwrap();

    let mut burst_buf = [0u8; 256];
    let mut total_read = 0;
    tokio::time::timeout(Duration::from_secs(5), async {
        while total_read < 23 {
            let n = client.read(&mut burst_buf[total_read..]).await.unwrap();
            assert!(n > 0);
            total_read += n;
        }
    })
    .await
    .expect("Timed out waiting for initial 23 bytes");
    assert_eq!(&burst_buf[0..2], &[0x83, 0x02], "Must receive ZC_AID first");

    client.write_all(&[0x7d, 0x00]).await.unwrap();
    client.flush().await.unwrap();

    let mut decoder = PacketDecoder::new(RathenaPacketLengthTable, TARGET_PACKETVER);
    let mut buffer = BytesMut::with_capacity(1024);
    // After ACTORINIT the server syncs weight (ZC_PAR_CHANGE); the spawner gets no entry
    // packet about itself.
    let mut received_par_change = false;
    let start = tokio::time::Instant::now();
    while start.elapsed() < Duration::from_millis(500) {
        if let Ok(Ok(n)) =
            tokio::time::timeout(Duration::from_millis(100), client.read_buf(&mut buffer)).await
        {
            if n == 0 {
                break;
            }
            while let Ok(Some(frame)) = decoder.decode(&mut buffer) {
                if frame.opcode == PacketOpcode::ZC_PAR_CHANGE {
                    received_par_change = true;
                    break;
                }
            }
            if received_par_change {
                break;
            }
        }
    }
    assert!(
        received_par_change,
        "Should receive ZC_PAR_CHANGE after ACTORINIT"
    );

    // Walk two cells so the persisted position differs from the seeded one.
    let walked_to = (custom_pos.x + 2, custom_pos.y);
    let mut cz_move = vec![0x5f, 0x03];
    cz_move.extend_from_slice(&ro_packets::pack_pos_dir(walked_to.0, walked_to.1, 0));
    client.write_all(&cz_move).await.unwrap();
    client.flush().await.unwrap();
    tokio::time::sleep(Duration::from_millis(600)).await;

    let restart_bytes = vec![0xb2, 0x00, 0x01];
    client.write_all(&restart_bytes).await.unwrap();
    client.flush().await.unwrap();

    let mut ack_raw = None;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    while ack_raw.is_none() && tokio::time::Instant::now() < deadline {
        while let Ok(Some(frame)) = decoder.decode(&mut buffer) {
            if frame.opcode == PacketOpcode::ZC_RESTART_ACK {
                ack_raw = Some(frame);
                break;
            }
        }
        if ack_raw.is_some() {
            break;
        }
        if let Ok(Ok(n)) =
            tokio::time::timeout(Duration::from_millis(100), client.read_buf(&mut buffer)).await
        {
            if n == 0 {
                break;
            }
        }
    }
    let ack = ack_raw.expect("Should receive ZC_RESTART_ACK packet");
    assert_eq!(ack.payload.as_ref(), &[1]);

    drop(client);

    let mut hydrated = None;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    while tokio::time::Instant::now() < deadline {
        if let Ok(Some(ch)) = repo_arc.find_by_id(char_id).await {
            if (ch.last_pos.x, ch.last_pos.y) == walked_to {
                hydrated = Some(ch);
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let hydrated =
        hydrated.expect("Character position should be updated on restart and disconnect");
    assert_eq!((hydrated.last_pos.x, hydrated.last_pos.y), walked_to);

    let transfer_pos = MapPosition {
        x: 200,
        y: 220,
        dir: 0,
    };
    let accepted = service.accept_transfer(char_id, "geffen.gat".to_string(), Some(transfer_pos));
    assert!(accepted);

    let mut transfer_hydrated = None;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    while tokio::time::Instant::now() < deadline {
        if let Ok(Some(ch)) = repo_arc.find_by_id(char_id).await {
            if ch.last_map == "geffen.gat" && ch.last_pos.x == 200 && ch.last_pos.y == 220 {
                transfer_hydrated = Some(ch);
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let transfer_hydrated =
        transfer_hydrated.expect("Character transfer map and position should be updated");
    assert_eq!(transfer_hydrated.last_map, "geffen.gat");
    assert_eq!(transfer_hydrated.last_pos.x, 200);
    assert_eq!(transfer_hydrated.last_pos.y, 220);

    let mut reconnect_client = TcpStream::connect(addr).await.unwrap();
    reconnect_client.write_all(&cz_enter).await.unwrap();
    reconnect_client.flush().await.unwrap();

    let mut reconnect_buf = [0u8; 256];
    let mut rec_read = 0;
    tokio::time::timeout(Duration::from_secs(5), async {
        while rec_read < 23 {
            let n = reconnect_client
                .read(&mut reconnect_buf[rec_read..])
                .await
                .unwrap();
            assert!(n > 0);
            rec_read += n;
        }
    })
    .await
    .expect("Timed out waiting for reconnect 23 bytes");
    assert_eq!(
        &reconnect_buf[0..2],
        &[0x83, 0x02],
        "Must receive ZC_AID on reconnect"
    );
    drop(reconnect_client);
}

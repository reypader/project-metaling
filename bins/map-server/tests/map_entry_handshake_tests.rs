use std::{net::SocketAddr, time::Duration};

use bytes::BytesMut;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tokio_util::codec::Decoder;

use map_server::{handle_client_session, MapServiceHandle};
use ro_contracts::{packet::PacketOpcode, TARGET_PACKETVER};
use ro_packets::{PacketDecoder, RathenaPacketLengthTable};

/// Spawns an isolated in-process `map-server` instance bound to an ephemeral loopback port.
async fn spawn_test_map_server() -> (SocketAddr, MapServiceHandle) {
    let service = MapServiceHandle::new();
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind ephemeral listener for map test");
    let addr = listener
        .local_addr()
        .expect("Failed to get local address for map test listener");
    let service_clone = service.clone();

    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((socket, peer_addr)) => {
                    tokio::spawn(handle_client_session(
                        socket,
                        peer_addr,
                        service_clone.clone(),
                    ));
                }
                Err(_) => break,
            }
        }
    });

    (addr, service)
}

/// Scenario `BIN-MAP-001`: Map Entry Handshake Atomic Burst
///
/// Verifies that on receiving `CZ_ENTER` (`0x0436`), the map server writes an atomic
/// 23-byte burst (`ZC_AID` + `ZC_INVENTORY_EXPANSION_INFO` + `ZC_ACCEPT_ENTER`)
/// and NEVER leaks raw un-framed `AccountId` bytes onto the TCP stream prior to `0x0283`.
#[tokio::test]
async fn test_map_entry_handshake_atomic_burst_without_raw_account_id() {
    let (addr, _service) = spawn_test_map_server().await;
    let mut client = TcpStream::connect(addr)
        .await
        .expect("Failed to connect to map-server");

    let account_id = 2000001u32;
    let char_id = 150001u32;
    let auth_code = 12345u32;
    let client_tick = 100000u32;

    let mut cz_enter = Vec::with_capacity(23);
    cz_enter.extend_from_slice(&[0x36, 0x04]); // Opcode CZ_ENTER (0x0436)
    cz_enter.extend_from_slice(&account_id.to_le_bytes());
    cz_enter.extend_from_slice(&char_id.to_le_bytes());
    cz_enter.extend_from_slice(&auth_code.to_le_bytes());
    cz_enter.extend_from_slice(&client_tick.to_le_bytes());
    cz_enter.extend_from_slice(&[0u8; 4]); // 4 bytes padding
    cz_enter.push(1); // Sex: 1 (Male)

    client
        .write_all(&cz_enter)
        .await
        .expect("Failed to write CZ_ENTER");
    client.flush().await.expect("Failed to flush CZ_ENTER");

    // Read server's handshake response
    let mut received_bytes = Vec::new();
    let mut buf = [0u8; 256];
    let start = tokio::time::Instant::now();
    while received_bytes.len() < 23 && start.elapsed() < Duration::from_millis(500) {
        let Ok(Ok(n)) =
            tokio::time::timeout(Duration::from_millis(100), client.read(&mut buf)).await
        else {
            break;
        };
        if n == 0 {
            break;
        }
        received_bytes.extend_from_slice(&buf[..n]);
    }

    // Negative Requirement: Zero un-framed raw 4-byte account_id bytes prior to 0x0283.
    // If the server erroneously echoes raw account_id [0x81, 0x84, 0x1e, 0x00],
    // received_bytes[0..2] will be [0x81, 0x84], failing this assertion.
    assert!(
        received_bytes.len() >= 2,
        "Expected at least 2 bytes received, got {}",
        received_bytes.len()
    );
    assert_eq!(
        &received_bytes[0..2],
        &[0x83, 0x02],
        "First packet must be ZC_AID (0x0283). Found leading bytes: {:?}",
        &received_bytes[..received_bytes.len().min(4)]
    );

    // Exact atomic 23-byte burst assertion
    assert_eq!(
        received_bytes.len(),
        23,
        "Map entry handshake burst must be exactly 23 bytes (ZC_AID + ZC_INVENTORY_EXPANSION_INFO + ZC_ACCEPT_ENTER)"
    );

    // 1. ZC_AID (0x0283, 6 bytes): [0x83, 0x02] + [account_id (u32 LE)]
    assert_eq!(&received_bytes[0..2], &[0x83, 0x02]);
    assert_eq!(&received_bytes[2..6], &account_id.to_le_bytes());

    // 2. ZC_INVENTORY_EXPANSION_INFO (0x0b18, 4 bytes): [0x18, 0x0b] + [expansion_size: 0 (i16 LE)]
    assert_eq!(&received_bytes[6..8], &[0x18, 0x0b]);
    assert_eq!(&received_bytes[8..10], &[0x00, 0x00]);

    // 3. ZC_ACCEPT_ENTER (0x02eb, 13 bytes): [0xeb, 0x02] + [startTime: u32 LE] + [posDir: 3 bytes] + [x_size: 5] + [y_size: 5] + [font: 0]
    assert_eq!(&received_bytes[10..12], &[0xeb, 0x02]);
    assert_eq!(received_bytes[19], 5, "Expected x_size = 5");
    assert_eq!(received_bytes[20], 5, "Expected y_size = 5");
    assert_eq!(&received_bytes[21..23], &[0x00, 0x00], "Expected font = 0");
}

/// Scenario `BIN-MAP-002`: Post-Entry Scene Initialization Handshake
///
/// Verifies that after entering the map, transmitting `CZ_NOTIFY_ACTORINIT` (`0x007d`)
/// causes the server to send parameter synchronization (`ZC_PAR_CHANGE`) to the character,
/// and broadcasts entity appearance (`ZC_NOTIFY_NEWENTRY` `0x09fe`) to observers in AoI.
#[tokio::test]
async fn test_post_entry_actorinit_spawns_and_syncs() {
    let (addr, _service) = spawn_test_map_server().await;

    // Connect observer player first
    let mut observer = TcpStream::connect(addr)
        .await
        .expect("Observer failed to connect");
    let mut observer_cz_enter = Vec::with_capacity(23);
    observer_cz_enter.extend_from_slice(&[0x36, 0x04]);
    observer_cz_enter.extend_from_slice(&2000002u32.to_le_bytes()); // account_id
    observer_cz_enter.extend_from_slice(&150002u32.to_le_bytes()); // char_id
    observer_cz_enter.extend_from_slice(&12345u32.to_le_bytes());
    observer_cz_enter.extend_from_slice(&100000u32.to_le_bytes());
    observer_cz_enter.extend_from_slice(&[0u8; 4]);
    observer_cz_enter.push(1);
    observer.write_all(&observer_cz_enter).await.unwrap();
    observer.flush().await.unwrap();

    // Drain observer's initial enter response
    let mut drain_buf = [0u8; 256];
    let _ = tokio::time::timeout(Duration::from_millis(100), observer.read(&mut drain_buf)).await;

    // Connect entering client
    let mut client = TcpStream::connect(addr)
        .await
        .expect("Client failed to connect");
    let mut cz_enter = Vec::with_capacity(23);
    cz_enter.extend_from_slice(&[0x36, 0x04]);
    cz_enter.extend_from_slice(&2000001u32.to_le_bytes());
    cz_enter.extend_from_slice(&150001u32.to_le_bytes());
    cz_enter.extend_from_slice(&12345u32.to_le_bytes());
    cz_enter.extend_from_slice(&100000u32.to_le_bytes());
    cz_enter.extend_from_slice(&[0u8; 4]);
    cz_enter.push(1);
    client.write_all(&cz_enter).await.unwrap();
    client.flush().await.unwrap();

    // Drain client's initial enter response
    let _ = tokio::time::timeout(Duration::from_millis(100), client.read(&mut drain_buf)).await;

    // Transmit CZ_NOTIFY_ACTORINIT (0x007d)
    client
        .write_all(&[0x7d, 0x00])
        .await
        .expect("Failed to write CZ_NOTIFY_ACTORINIT");
    client.flush().await.unwrap();

    // Joining client must receive parameter sync (ZC_PAR_CHANGE 0x00b0)
    let mut client_buf = BytesMut::with_capacity(1024);
    let mut decoder = PacketDecoder::new(RathenaPacketLengthTable, TARGET_PACKETVER);
    let mut received_par_change = false;

    let start = tokio::time::Instant::now();
    while start.elapsed() < Duration::from_millis(500) {
        if let Ok(Ok(n)) =
            tokio::time::timeout(Duration::from_millis(100), client.read_buf(&mut client_buf))
                .await
        {
            if n == 0 {
                break;
            }
            while let Ok(Some(frame)) = decoder.decode(&mut client_buf) {
                if frame.opcode == PacketOpcode::ZC_PAR_CHANGE
                    || frame.opcode == PacketOpcode::ZC_LONGPAR_CHANGE
                {
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
        "Joining client must receive ZC_PAR_CHANGE (0x00b0) on ActorInit"
    );

    // Nearby observer must receive ZC_NOTIFY_NEWENTRY (0x09fe) with GID matching char_id 150001
    let mut observer_buf = BytesMut::with_capacity(1024);
    let mut observer_decoder = PacketDecoder::new(RathenaPacketLengthTable, TARGET_PACKETVER);
    let mut received_newentry = false;

    let start = tokio::time::Instant::now();
    while start.elapsed() < Duration::from_millis(500) {
        if let Ok(Ok(n)) = tokio::time::timeout(
            Duration::from_millis(100),
            observer.read_buf(&mut observer_buf),
        )
        .await
        {
            if n == 0 {
                break;
            }
            while let Ok(Some(frame)) = observer_decoder.decode(&mut observer_buf) {
                if frame.opcode == PacketOpcode::ZC_NOTIFY_NEWENTRY {
                    let gid = if frame.payload.len() >= 5 {
                        u32::from_le_bytes(frame.payload[1..5].try_into().unwrap())
                    } else if frame.payload.len() >= 4 {
                        u32::from_le_bytes(frame.payload[0..4].try_into().unwrap())
                    } else {
                        0
                    };
                    if gid == 150001 {
                        received_newentry = true;
                        break;
                    }
                }
            }
            if received_newentry {
                break;
            }
        }
    }
    assert!(
        received_newentry,
        "Nearby observer must receive ZC_NOTIFY_NEWENTRY (0x09fe) AoI broadcast with GID 150001 when character spawns"
    );
}

/// Scenario `BIN-MAP-003`: Periodic Clock Synchronization
///
/// Verifies that transmitting `CZ_REQUEST_TIME` (`0x0360`) responds with `ZC_NOTIFY_TIME` (`0x007f`).
#[tokio::test]
async fn test_clock_synchronization_zc_notify_time() {
    let (addr, _service) = spawn_test_map_server().await;
    let mut client = TcpStream::connect(addr).await.expect("Failed to connect");

    // Complete entry handshake
    let mut cz_enter = Vec::with_capacity(23);
    cz_enter.extend_from_slice(&[0x36, 0x04]);
    cz_enter.extend_from_slice(&2000001u32.to_le_bytes());
    cz_enter.extend_from_slice(&150001u32.to_le_bytes());
    cz_enter.extend_from_slice(&12345u32.to_le_bytes());
    cz_enter.extend_from_slice(&100000u32.to_le_bytes());
    cz_enter.extend_from_slice(&[0u8; 4]);
    cz_enter.push(1);
    client.write_all(&cz_enter).await.unwrap();
    client.flush().await.unwrap();

    // Send CZ_REQUEST_TIME (0x0360, 6 bytes): [0x60, 0x03] + [client_tick = 50000]
    let client_tick = 50000u32;
    let mut cz_time = Vec::with_capacity(6);
    cz_time.extend_from_slice(&[0x60, 0x03]);
    cz_time.extend_from_slice(&client_tick.to_le_bytes());
    client
        .write_all(&cz_time)
        .await
        .expect("Failed to write CZ_REQUEST_TIME");
    client.flush().await.unwrap();

    // Decode packets from client stream using PacketDecoder
    let mut client_buf = BytesMut::with_capacity(1024);
    let mut decoder = PacketDecoder::new(RathenaPacketLengthTable, TARGET_PACKETVER);
    let mut received_time = None;

    let start = tokio::time::Instant::now();
    while start.elapsed() < Duration::from_millis(500) {
        if let Ok(Ok(n)) =
            tokio::time::timeout(Duration::from_millis(100), client.read_buf(&mut client_buf))
                .await
        {
            if n == 0 {
                break;
            }
            while let Ok(Some(frame)) = decoder.decode(&mut client_buf) {
                if frame.opcode == PacketOpcode::ZC_NOTIFY_TIME {
                    received_time = Some(frame);
                    break;
                }
            }
            if received_time.is_some() {
                break;
            }
        }
    }

    let time_frame =
        received_time.expect("Expected ZC_NOTIFY_TIME (0x007f) response from map-server");
    assert_eq!(
        time_frame.payload.len(),
        4,
        "ZC_NOTIFY_TIME payload must be 4 bytes"
    );
    let server_tick = u32::from_le_bytes(time_frame.payload[0..4].try_into().unwrap());
    assert!(server_tick > 0, "Server tick must be positive");
}

/// Scenario `BIN-MAP-004`: Rejection of Malformed / Truncated `CZ_ENTER`
///
/// Verifies that a truncated `CZ_ENTER` fragment followed by client disconnect
/// is cleanly closed by the server without sending partial frames or raw `AccountId` bytes.
#[tokio::test]
async fn test_malformed_cz_enter_disconnects_cleanly() {
    let (addr, _service) = spawn_test_map_server().await;
    let mut client = TcpStream::connect(addr).await.expect("Failed to connect");

    // Send truncated CZ_ENTER: 10 bytes instead of 23 bytes
    let truncated_cz_enter = vec![0x36, 0x04, 0x81, 0x84, 0x1e, 0x00, 0x51, 0x4a, 0x02, 0x00];
    client
        .write_all(&truncated_cz_enter)
        .await
        .expect("Failed to write truncated frame");
    client.flush().await.unwrap();

    // Send immediate EOF by shutting down writer half
    client
        .shutdown()
        .await
        .expect("Failed to shutdown client write half");

    // Server must disconnect cleanly without emitting raw account_id or partial packets
    let mut received = Vec::new();
    let mut buf = [0u8; 128];
    let read_result = tokio::time::timeout(Duration::from_millis(500), async {
        loop {
            match client.read(&mut buf).await {
                Ok(0) => break, // EOF reached cleanly
                Ok(n) => received.extend_from_slice(&buf[..n]),
                Err(_) => break,
            }
        }
    })
    .await;

    assert!(
        read_result.is_ok(),
        "Server hung without closing connection on malformed packet"
    );
    assert_eq!(
        received.len(),
        0,
        "Server must not send any response bytes on malformed CZ_ENTER; received: {:?}",
        received
    );
}

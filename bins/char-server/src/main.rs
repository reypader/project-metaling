mod config;
mod service;
mod session;

pub use config::CharConfig;

use std::{net::SocketAddr, sync::Arc};

use axum::{
    routing::{get, post},
    Json, Router,
};
use anyhow::Context;
use tokio::net::TcpListener;
use tracing::{error, info, warn};

use ro_contracts::{
    HealthResponse, PrepareMapTransferRequest, PrepareMapTransferResponse,
    RegisterMapServerRequest, RegisterMapServerResponse, ServerStatus, ServerType,
    TARGET_PACKETVER,
};
use ro_persistence::InMemoryStubRepo;

use service::CharServiceHandle;
use session::handle_client_session;

async fn health_check(
    axum::extract::State(service): axum::extract::State<CharServiceHandle>,
) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: ServerStatus::Ready,
        server_type: ServerType::Char,
        server_name: "metaling-char".to_string(),
        uptime_seconds: service.uptime_seconds(),
        active_connections: 0,
        target_packetver: TARGET_PACKETVER,
    })
}

async fn register_map_server(
    axum::extract::State(service): axum::extract::State<CharServiceHandle>,
    Json(payload): Json<RegisterMapServerRequest>,
) -> Json<RegisterMapServerResponse> {
    info!(
        "Registering map server {} at {}:{} with {} maps",
        payload.name,
        payload.client_host,
        payload.client_port,
        payload.maps.len()
    );

    let host = payload
        .client_host
        .parse::<std::net::Ipv4Addr>()
        .map(|addr| addr.octets())
        .unwrap_or([127, 0, 0, 1]);

    let registered_maps_count = payload.maps.len();
    let accepted = service.register_map_server(
        payload.name,
        payload.maps,
        host,
        payload.client_port,
    );

    Json(RegisterMapServerResponse {
        accepted,
        registered_maps_count,
    })
}

async fn prepare_transfer(
    Json(payload): Json<PrepareMapTransferRequest>,
) -> Json<PrepareMapTransferResponse> {
    info!(
        "Preparing map transfer for character id {} to map {}",
        payload.char_id.0, payload.destination_map
    );
    Json(PrepareMapTransferResponse { accepted: true })
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let config = CharConfig::from_env();
    info!(
        "Starting char-server (Target PACKETVER: {}, Client TCP: {}, Internal HTTP: {}, Advertised Port: {}, Default Map Port: {})",
        TARGET_PACKETVER, config.client_port, config.http_port, config.advertised_port, config.default_map_port
    );

    let char_repo = Arc::new(InMemoryStubRepo::new());
    let service_handle = CharServiceHandle::with_default_map_port(char_repo, config.default_map_port);

    let http_router = Router::new()
        .route("/api/v1/health", get(health_check))
        .route("/api/v1/maps/register", post(register_map_server))
        .route("/api/v1/transfers/prepare", post(prepare_transfer))
        .with_state(service_handle.clone());

    let http_addr: SocketAddr = format!("0.0.0.0:{}", config.http_port)
        .parse()
        .context("Failed to parse char-server HTTP address")?;
    let http_listener = TcpListener::bind(http_addr)
        .await
        .with_context(|| format!("Failed to bind char-server HTTP listener to {}", http_addr))?;
    info!("Character HTTP internal API listening on {}", http_addr);

    let client_addr: SocketAddr = format!("0.0.0.0:{}", config.client_port)
        .parse()
        .context("Failed to parse char-server client TCP address")?;
    let client_listener = TcpListener::bind(client_addr)
        .await
        .with_context(|| format!("Failed to bind char-server client TCP listener to {}", client_addr))?;
    info!("Character client TCP listener active on {}", client_addr);

    let http_task = tokio::spawn(async move {
        if let Err(err) = axum::serve(http_listener, http_router).await {
            error!("HTTP server failure: {}", err);
        }
    });

    let client_service = service_handle.clone();
    let client_task = tokio::spawn(async move {
        loop {
            match client_listener.accept().await {
                Ok((stream, peer_addr)) => {
                    tokio::spawn(handle_client_session(
                        stream,
                        peer_addr,
                        client_service.clone(),
                    ));
                }
                Err(err) => {
                    warn!("Transient error accepting client connection: {}", err);
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    continue;
                }
            }
        }
    });

    tokio::select! {
        _ = http_task => info!("HTTP server task terminated"),
        _ = client_task => info!("Client TCP listener task terminated"),
        _ = tokio::signal::ctrl_c() => info!("Shutdown signal received"),
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::BytesMut;
    use ro_contracts::{character::CharacterRepository, packet::PacketOpcode};
    use ro_packets::{PacketDecoder, RathenaPacketLengthTable};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpStream,
    };
    use tokio_util::codec::Decoder;

    #[tokio::test]
    async fn test_char_handshake_via_channel_service() {
        let service_handle = CharServiceHandle::default();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let server_addr = listener.local_addr().unwrap();

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
        while received_opcodes.len() < 5 {
            let n = client.read_buf(&mut buffer).await.unwrap();
            assert!(n > 0);
            while let Some(raw) = decoder.decode(&mut buffer).unwrap() {
                received_opcodes.push(raw.opcode);
            }
        }
        assert_eq!(received_opcodes[0], PacketOpcode::HC_ACCEPT_ENTER2);
        assert_eq!(received_opcodes[1], PacketOpcode::HC_ACCEPT_ENTER);
        assert_eq!(received_opcodes[2], PacketOpcode(0x09a0));
        assert_eq!(received_opcodes[3], PacketOpcode::HC_BLOCK_CHARACTER);
        assert_eq!(received_opcodes[4], PacketOpcode::HC_SECOND_PASSWD_LOGIN);

        let ch_select_bytes = vec![0x66, 0x00, 0x00];
        client.write_all(&ch_select_bytes).await.unwrap();
        client.flush().await.unwrap();

        let mut notify_raw = None;
        while notify_raw.is_none() {
            let n = client.read_buf(&mut buffer).await.unwrap();
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
        let raw = notify_raw.expect("expected HC_NOTIFY_ZONESVR");
        assert_eq!(raw.opcode, PacketOpcode::HC_NOTIFY_ZONESVR);
    }

    #[tokio::test]
    async fn test_full_login_sequence_char_flow() {
        let char_repo = Arc::new(InMemoryStubRepo::new());
        let service_handle = CharServiceHandle::new(char_repo.clone());

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let server_addr = listener.local_addr().unwrap();

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
        while enter_opcodes.len() < 5 {
            let n = client1.read_buf(&mut buffer).await.unwrap();
            assert!(n > 0);
            while let Some(raw) = decoder.decode(&mut buffer).unwrap() {
                enter_opcodes.push(raw.opcode);
            }
        }
        assert_eq!(enter_opcodes[0], PacketOpcode::HC_ACCEPT_ENTER2);
        assert_eq!(enter_opcodes[1], PacketOpcode::HC_ACCEPT_ENTER);
        assert_eq!(enter_opcodes[2], PacketOpcode(0x09a0));
        assert_eq!(enter_opcodes[3], PacketOpcode::HC_BLOCK_CHARACTER);
        assert_eq!(enter_opcodes[4], PacketOpcode::HC_SECOND_PASSWD_LOGIN);

        client1.write_all(&[0xa1, 0x09]).await.unwrap();
        client1.flush().await.unwrap();

        let mut charlist_ack = None;
        while charlist_ack.is_none() {
            let n = client1.read_buf(&mut buffer).await.unwrap();
            assert!(n > 0);
            while let Some(raw) = decoder.decode(&mut buffer).unwrap() {
                if raw.opcode == PacketOpcode(0x0b72) {
                    charlist_ack = Some(raw);
                    break;
                }
            }
        }
        assert!(charlist_ack.is_some());

        let mut make_char_bytes = vec![0x39, 0x0a];
        let mut name_bytes = [0u8; 24];
        name_bytes[..5].copy_from_slice(b"test5");
        make_char_bytes.extend_from_slice(&name_bytes);
        make_char_bytes.push(4);
        make_char_bytes.extend_from_slice(&0u16.to_le_bytes());
        make_char_bytes.extend_from_slice(&22u16.to_le_bytes());
        make_char_bytes.extend_from_slice(&0u32.to_le_bytes());
        make_char_bytes.push(0);

        client1.write_all(&make_char_bytes).await.unwrap();
        client1.flush().await.unwrap();

        let mut make_char_ack = None;
        while make_char_ack.is_none() {
            let n = client1.read_buf(&mut buffer).await.unwrap();
            assert!(n > 0);
            while let Some(raw) = decoder.decode(&mut buffer).unwrap() {
                if raw.opcode == PacketOpcode::HC_ACCEPT_MAKECHAR {
                    make_char_ack = Some(raw);
                    break;
                }
            }
        }
        let ack = make_char_ack.expect("expected HC_ACCEPT_MAKECHAR 0x0b6f");
        assert_eq!(ack.opcode, PacketOpcode::HC_ACCEPT_MAKECHAR);

        let ch_select_bytes = vec![0x66, 0x00, 0x04];
        client1.write_all(&ch_select_bytes).await.unwrap();
        client1.flush().await.unwrap();

        let mut notify_raw = None;
        while notify_raw.is_none() {
            let n = client1.read_buf(&mut buffer).await.unwrap();
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
        while enter_opcodes2.len() < 5 {
            let n = client2.read_buf(&mut buffer2).await.unwrap();
            assert!(n > 0);
            while let Some(raw) = decoder.decode(&mut buffer2).unwrap() {
                enter_opcodes2.push(raw.opcode);
            }
        }
        assert_eq!(enter_opcodes2[0], PacketOpcode::HC_ACCEPT_ENTER2);
        assert_eq!(enter_opcodes2[1], PacketOpcode::HC_ACCEPT_ENTER);
        assert_eq!(enter_opcodes2[2], PacketOpcode(0x09a0));
        assert_eq!(enter_opcodes2[3], PacketOpcode::HC_BLOCK_CHARACTER);
        assert_eq!(enter_opcodes2[4], PacketOpcode::HC_SECOND_PASSWD_LOGIN);

        client2.write_all(&[0xa1, 0x09]).await.unwrap();
        client2.flush().await.unwrap();

        let mut charlist_ack2 = None;
        while charlist_ack2.is_none() {
            let n = client2.read_buf(&mut buffer2).await.unwrap();
            assert!(n > 0);
            while let Some(raw) = decoder.decode(&mut buffer2).unwrap() {
                if raw.opcode == PacketOpcode(0x0b72) {
                    charlist_ack2 = Some(raw);
                    break;
                }
            }
        }
        assert!(charlist_ack2.is_some());

        let chars = char_repo
            .list_by_account(ro_contracts::ids::AccountId(2000000))
            .await
            .unwrap();
        let test5_char = chars
            .iter()
            .find(|c| c.name == "test5")
            .expect("test5 character should exist");
        assert_eq!(test5_char.slot, 4);
        let test5_char_id = test5_char.char_id;

        let mut del_reserve_bytes = vec![0x27, 0x08];
        del_reserve_bytes.extend_from_slice(&test5_char_id.0.to_le_bytes());

        client2.write_all(&del_reserve_bytes).await.unwrap();
        client2.flush().await.unwrap();

        let mut del_ack = None;
        while del_ack.is_none() {
            let n = client2.read_buf(&mut buffer2).await.unwrap();
            assert!(n > 0);
            while let Some(raw) = decoder.decode(&mut buffer2).unwrap() {
                if raw.opcode == PacketOpcode::HC_DELETE_CHAR3_RESERVED {
                    del_ack = Some(raw);
                    break;
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
        while cancel_ack.is_none() {
            let n = client2.read_buf(&mut buffer2).await.unwrap();
            assert!(n > 0);
            while let Some(raw) = decoder.decode(&mut buffer2).unwrap() {
                if raw.opcode == PacketOpcode::HC_DELETE_CHAR3_CANCEL {
                    cancel_ack = Some(raw);
                    break;
                }
            }
        }
        let cancel_raw = cancel_ack.expect("expected HC_DELETE_CHAR3_CANCEL 0x082c");
        assert_eq!(cancel_raw.opcode, PacketOpcode::HC_DELETE_CHAR3_CANCEL);
        let returned_cancel_char_id =
            u32::from_le_bytes(cancel_raw.payload[0..4].try_into().unwrap());
        let returned_cancel_result =
            u32::from_le_bytes(cancel_raw.payload[4..8].try_into().unwrap());
        assert_eq!(returned_cancel_char_id, test5_char_id.0);
        assert_eq!(returned_cancel_result, 1);
    }
}

mod config;

pub use config::MapConfig;

use std::net::SocketAddr;

use anyhow::Context;
use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use tokio::net::TcpListener;
use tracing::{error, info, warn};

use map_server::{handle_client_session, MapServiceHandle};
use ro_contracts::{
    HealthResponse, PrepareMapTransferRequest, PrepareMapTransferResponse, ServerStatus,
    ServerType, TARGET_PACKETVER,
};

async fn health_check(
    State(service): State<MapServiceHandle>,
) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: ServerStatus::Ready,
        server_type: ServerType::Map,
        server_name: "metaling-map".to_string(),
        uptime_seconds: service.uptime_seconds(),
        active_connections: 0,
        target_packetver: TARGET_PACKETVER,
    })
}

async fn accept_transfer(
    State(service): State<MapServiceHandle>,
    Json(payload): Json<PrepareMapTransferRequest>,
) -> Json<PrepareMapTransferResponse> {
    info!(
        "Map-server processing transfer reservation for character id {} on map {}",
        payload.char_id.0, payload.destination_map
    );

    let accepted = service.accept_transfer(
        payload.char_id,
        payload.destination_map,
        Some(payload.destination_pos),
    );

    Json(PrepareMapTransferResponse { accepted })
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let config = MapConfig::from_env();
    info!(
        "Starting map-server (Target PACKETVER: {}, Client TCP: {}, Internal HTTP: {}, Advertised Port: {})",
        TARGET_PACKETVER, config.client_port, config.http_port, config.advertised_port
    );

    let char_repo = std::sync::Arc::new(
        ro_persistence::PostgresRepo::connect(&config.database_url)
            .await
            .with_context(|| format!("Failed to connect to database at {}", config.database_url))?,
    );
    let service_handle = MapServiceHandle::new(char_repo);

    let http_router = Router::new()
        .route("/api/v1/health", get(health_check))
        .route("/api/v1/transfers/accept", post(accept_transfer))
        .with_state(service_handle.clone());

    let http_addr: SocketAddr = format!("0.0.0.0:{}", config.http_port)
        .parse()
        .context("Failed to parse map-server HTTP address")?;
    let http_listener = TcpListener::bind(http_addr)
        .await
        .with_context(|| format!("Failed to bind map-server HTTP listener to {}", http_addr))?;
    info!("Map HTTP internal API listening on {}", http_addr);

    let client_addr: SocketAddr = format!("0.0.0.0:{}", config.client_port)
        .parse()
        .context("Failed to parse map-server client TCP address")?;
    let client_listener = TcpListener::bind(client_addr)
        .await
        .with_context(|| format!("Failed to bind map-server client TCP listener to {}", client_addr))?;
    info!("Map client TCP listener active on {}", client_addr);

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
    use std::sync::Arc;
    use bytes::BytesMut;
    use ro_contracts::packet::PacketOpcode;
    use ro_packets::{PacketDecoder, RathenaPacketLengthTable};
    use ro_persistence::PostgresRepo;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpStream,
    };
    use tokio_util::codec::Decoder;

    async fn setup_test_repo() -> Arc<dyn ro_contracts::character::CharacterRepository> {
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
        assert_eq!(init_opcodes[0], PacketOpcode::ZC_NOTIFY_NEWENTRY);
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
        use ro_contracts::ids::{AccountId, CharId, MapPosition};

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
        let res = accept_transfer(State(service.clone()), Json(valid_req)).await.0;
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
            assert!(!res.accepted, "Expected map '{}' to be rejected", invalid_map);
        }
    }
}

mod service;
mod session;

use std::net::SocketAddr;

use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use tokio::net::TcpListener;
use tracing::{error, info};

use ro_contracts::{
    HealthResponse, PrepareMapTransferRequest, PrepareMapTransferResponse, ServerStatus,
    ServerType, TARGET_PACKETVER,
};

use service::MapServiceHandle;
use session::handle_client_session;

async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: ServerStatus::Ready,
        server_type: ServerType::Map,
        server_name: "metaling-map".to_string(),
        uptime_seconds: 0,
        active_connections: 0,
        target_packetver: TARGET_PACKETVER,
    })
}

async fn accept_transfer(
    State(service): State<MapServiceHandle>,
    Json(payload): Json<PrepareMapTransferRequest>,
) -> Json<PrepareMapTransferResponse> {
    info!(
        "Map-server accepted transfer reservation for character id {} on map {}",
        payload.char_id.0, payload.destination_map
    );

    service.accept_transfer(payload.char_id, payload.destination_map);

    Json(PrepareMapTransferResponse { accepted: true })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    info!(
        "Starting map-server (Target PACKETVER: {}, Client TCP: 5121, Internal HTTP: 5122)",
        TARGET_PACKETVER
    );

    let service_handle = MapServiceHandle::new();

    let http_router = Router::new()
        .route("/api/v1/health", get(health_check))
        .route("/api/v1/transfers/accept", post(accept_transfer))
        .with_state(service_handle.clone());

    let http_addr: SocketAddr = "0.0.0.0:5122".parse()?;
    let http_listener = TcpListener::bind(http_addr).await?;
    info!("Map HTTP internal API listening on {}", http_addr);

    let client_addr: SocketAddr = "0.0.0.0:5121".parse()?;
    let client_listener = TcpListener::bind(client_addr).await?;
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
                    error!("Error accepting client connection: {}", err);
                    break;
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
    use ro_contracts::packet::PacketOpcode;
    use ro_packets::{PacketDecoder, RathenaPacketLengthTable};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpStream,
    };
    use tokio_util::codec::Decoder;

    #[tokio::test]
    async fn test_map_handshake_enter_actorinit_and_time() {
        let service_handle = MapServiceHandle::new();

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

        let mut account_id_buf = [0u8; 4];
        client.read_exact(&mut account_id_buf).await.unwrap();
        assert_eq!(u32::from_le_bytes(account_id_buf), 2000000);

        let mut decoder = PacketDecoder::new(RathenaPacketLengthTable, TARGET_PACKETVER);
        let mut buffer = BytesMut::with_capacity(2048);

        let mut accept_raw = None;
        while accept_raw.is_none() {
            let n = client.read_buf(&mut buffer).await.unwrap();
            assert!(n > 0);
            if let Some(raw) = decoder.decode(&mut buffer).unwrap() {
                accept_raw = Some(raw);
            }
        }
        let raw = accept_raw.unwrap();
        assert_eq!(raw.opcode, PacketOpcode::ZC_ACCEPT_ENTER);

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
        let service_handle = MapServiceHandle::new();

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

        let mut account_id_buf = [0u8; 4];
        client.read_exact(&mut account_id_buf).await.unwrap();
        assert_eq!(u32::from_le_bytes(account_id_buf), 2000000);

        let mut decoder = PacketDecoder::new(RathenaPacketLengthTable, TARGET_PACKETVER);
        let mut buffer = BytesMut::with_capacity(2048);

        let mut accept_raw = None;
        while accept_raw.is_none() {
            let n = client.read_buf(&mut buffer).await.unwrap();
            assert!(n > 0);
            if let Some(raw) = decoder.decode(&mut buffer).unwrap() {
                accept_raw = Some(raw);
            }
        }
        let raw = accept_raw.unwrap();
        assert_eq!(raw.opcode, PacketOpcode::ZC_ACCEPT_ENTER);

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
        assert_eq!(raw.opcode, PacketOpcode(0x00b3));
        assert_eq!(raw.payload.as_ref(), &[1]);
    }
}

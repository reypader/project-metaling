mod service;
mod session;

use std::net::SocketAddr;

use axum::{
    routing::{get, post},
    Json, Router,
};
use tokio::net::TcpListener;
use tracing::{error, info};

use ro_contracts::{
    HealthResponse, PrepareMapTransferRequest, PrepareMapTransferResponse,
    RegisterMapServerRequest, RegisterMapServerResponse, ServerStatus, ServerType,
    TARGET_PACKETVER,
};

use service::CharServiceHandle;
use session::handle_client_session;

async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: ServerStatus::Ready,
        server_type: ServerType::Char,
        server_name: "metaling-char".to_string(),
        uptime_seconds: 0,
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

    let registered_maps_count = payload.maps.len();
    service.register_map_server(
        payload.name,
        payload.maps,
        [127, 0, 0, 1],
        payload.client_port,
    );

    Json(RegisterMapServerResponse {
        accepted: true,
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
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    info!(
        "Starting char-server (Target PACKETVER: {}, Client TCP: 6121, Internal HTTP: 6122)",
        TARGET_PACKETVER
    );

    let service_handle = CharServiceHandle::new();

    let http_router = Router::new()
        .route("/api/v1/health", get(health_check))
        .route("/api/v1/maps/register", post(register_map_server))
        .route("/api/v1/transfers/prepare", post(prepare_transfer))
        .with_state(service_handle.clone());

    let http_addr: SocketAddr = "0.0.0.0:6122".parse()?;
    let http_listener = TcpListener::bind(http_addr).await?;
    info!("Character HTTP internal API listening on {}", http_addr);

    let client_addr: SocketAddr = "0.0.0.0:6121".parse()?;
    let client_listener = TcpListener::bind(client_addr).await?;
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
    async fn test_char_handshake_via_channel_service() {
        let service_handle = CharServiceHandle::new();

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
        while received_opcodes.len() < 2 {
            let n = client.read_buf(&mut buffer).await.unwrap();
            assert!(n > 0);
            while let Some(raw) = decoder.decode(&mut buffer).unwrap() {
                received_opcodes.push(raw.opcode);
            }
        }
        assert_eq!(received_opcodes[0], PacketOpcode::HC_ACCEPT_ENTER);
        assert_eq!(received_opcodes[1], PacketOpcode(0x099d));

        let ch_select_bytes = vec![0x66, 0x00, 0x00];
        client.write_all(&ch_select_bytes).await.unwrap();
        client.flush().await.unwrap();

        let mut notify_raw = None;
        while notify_raw.is_none() {
            let n = client.read_buf(&mut buffer).await.unwrap();
            if n == 0 {
                break;
            }
            if let Some(raw) = decoder.decode(&mut buffer).unwrap() {
                notify_raw = Some(raw);
            }
        }
        let raw = notify_raw.expect("expected HC_NOTIFY_ZONESVR");
        assert_eq!(raw.opcode, PacketOpcode::HC_NOTIFY_ZONESVR);
    }
}

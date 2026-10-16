use std::net::SocketAddr;

use bytes::BytesMut;
use ro_contracts::{
    packet::{EventEncoder, PacketIntentParser, ServerEvent, SessionIntent},
    TARGET_PACKETVER,
};
use ro_packets::{PacketDecoder, Phase0EventEncoder, Phase0IntentParser, RathenaPacketLengthTable};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::mpsc,
};
use tokio_util::codec::Decoder;
use tracing::{debug, error, info, warn};

use crate::service::LoginServiceHandle;

pub async fn handle_client_session(
    socket: TcpStream,
    peer_addr: SocketAddr,
    service: LoginServiceHandle,
) {
    let session_id = service.allocate_session_id();
    info!(
        "Accepted login client connection {:?} from {}",
        session_id, peer_addr
    );

    let (mut reader, mut writer) = socket.into_split();
    let (outbound_tx, mut outbound_rx) = mpsc::channel::<ServerEvent>(16);
    let intent_tx = service.spawn_session_actor(session_id, outbound_tx);

    let writer_task = tokio::spawn(async move {
        let encoder = Phase0EventEncoder;
        while let Some(event) = outbound_rx.recv().await {
            info!(
                "Sending event to login client {:?} ({}): {:?}",
                session_id, peer_addr, event
            );
            match encoder.encode_event(&event, TARGET_PACKETVER) {
                Ok(bytes) => {
                    if let Err(err) = writer.write_all(&bytes).await {
                        error!("Failed to write event to {}: {}", peer_addr, err);
                        return;
                    }
                    if let Err(err) = writer.flush().await {
                        error!("Failed to flush event to {}: {}", peer_addr, err);
                        return;
                    }
                    if matches!(
                        event,
                        ServerEvent::LoginAccepted { .. } | ServerEvent::LoginRefused { .. }
                    ) {
                        info!(
                            "Completed login handshake for {:?} ({}). Closing socket.",
                            session_id, peer_addr
                        );
                        return;
                    }
                }
                Err(err) => {
                    error!("Failed to encode server event: {}", err);
                    return;
                }
            }
        }
    });

    let mut decoder = PacketDecoder::new(RathenaPacketLengthTable, TARGET_PACKETVER);
    let mut buffer = BytesMut::with_capacity(2048);
    let parser = Phase0IntentParser;

    loop {
        match reader.read_buf(&mut buffer).await {
            Ok(0) => {
                info!("Client {:?} ({}) closed connection", session_id, peer_addr);
                break;
            }
            Ok(bytes_read) => {
                debug!(
                    "Read {} bytes from client {:?} ({})",
                    bytes_read, session_id, peer_addr
                );
            }
            Err(err) => {
                error!(
                    "Socket read error from client {:?} ({}): {}",
                    session_id, peer_addr, err
                );
                break;
            }
        }

        while let Some(raw_packet) = match decoder.decode(&mut buffer) {
            Ok(frame) => frame,
            Err(err) => {
                error!("Packet framing error from {}: {}", peer_addr, err);
                let _ = writer_task.await;
                return;
            }
        } {
            match parser.parse_intent(&raw_packet, TARGET_PACKETVER) {
                Ok(Some(intent)) => {
                    info!(
                        "Session {:?} parsed client intent: {:?}",
                        session_id, intent
                    );
                    let session_intent = SessionIntent::new(session_id, None, None, intent);
                    if intent_tx.send(session_intent).await.is_err() {
                        let _ = writer_task.await;
                        return;
                    }
                }
                Ok(None) => {
                    debug!(
                        "Unhandled packet opcode={:#06x} len={}",
                        raw_packet.opcode.0,
                        raw_packet.payload_len()
                    );
                }
                Err(err) => {
                    warn!("Packet parsing error from {}: {}", peer_addr, err);
                }
            }
        }
    }

    drop(intent_tx);
    let _ = writer_task.await;
}

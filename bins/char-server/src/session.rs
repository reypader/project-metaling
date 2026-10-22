use std::net::SocketAddr;

use bytes::{Bytes, BytesMut};
use ro_core::{AccountId, ClientIntent, ServerEvent, SessionIntent, TARGET_PACKETVER};
use ro_packets::{
    EventEncoder, PacketDecoder, PacketIntentParser, Phase0EventEncoder, Phase0IntentParser,
    RathenaPacketLengthTable,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::mpsc,
};
use tokio_util::codec::Decoder;
use tracing::{debug, error, info, warn};

use crate::service::CharServiceHandle;

enum OutboundFrame {
    RawBytes(Bytes),
    Event(ServerEvent),
}

pub async fn handle_client_session(
    socket: TcpStream,
    peer_addr: SocketAddr,
    service: CharServiceHandle,
) {
    let session_id = service.allocate_session_id();
    info!(
        "Accepted char client connection {:?} from {}",
        session_id, peer_addr
    );

    let (mut reader, mut writer) = socket.into_split();
    let (frame_tx, mut frame_rx) = mpsc::channel::<OutboundFrame>(16);
    let (event_tx, mut event_rx) = mpsc::channel::<ServerEvent>(16);

    let intent_tx = service.spawn_session_actor(session_id, event_tx);

    let frame_tx_for_events = frame_tx.clone();
    let mut forwarder_task = tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            if frame_tx_for_events
                .send(OutboundFrame::Event(event))
                .await
                .is_err()
            {
                break;
            }
        }
    });

    let mut writer_task = tokio::spawn(async move {
        let encoder = Phase0EventEncoder;
        while let Some(frame) = frame_rx.recv().await {
            match frame {
                OutboundFrame::RawBytes(bytes) => {
                    if let Err(err) = writer.write_all(&bytes).await {
                        error!("Failed to write raw bytes to {}: {}", peer_addr, err);
                        return;
                    }
                    if let Err(err) = writer.flush().await {
                        error!("Failed to flush raw bytes to {}: {}", peer_addr, err);
                        return;
                    }
                }
                OutboundFrame::Event(event) => {
                    info!(
                        "Sending event to char client {:?} ({}): {:?}",
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
                            if matches!(event, ServerEvent::CharacterSelected { .. }) {
                                info!(
                                    "Character selected for session {:?} ({}). Socket remains open for closure initiated by client.",
                                    session_id, peer_addr
                                );
                            }
                        }
                        Err(err) => {
                            error!("Failed to encode server event: {}", err);
                            return;
                        }
                    }
                }
            }
        }
    });

    let mut decoder = PacketDecoder::new(RathenaPacketLengthTable, TARGET_PACKETVER);
    let mut buffer = BytesMut::with_capacity(2048);
    let parser = Phase0IntentParser;
    let mut authenticated_account: Option<AccountId> = None;

    'session_loop: loop {
        match reader.read_buf(&mut buffer).await {
            Ok(0) => {
                info!("Client {:?} ({}) closed connection", session_id, peer_addr);
                break 'session_loop;
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
                break 'session_loop;
            }
        }

        while let Some(raw_packet) = match decoder.decode(&mut buffer) {
            Ok(frame) => frame,
            Err(err) => {
                error!("Packet framing error from {}: {}", peer_addr, err);
                break 'session_loop;
            }
        } {
            match parser.parse_intent(&raw_packet, TARGET_PACKETVER) {
                Ok(Some(intent)) => {
                    info!(
                        "Session {:?} parsed client intent: {:?}",
                        session_id, intent
                    );

                    if let ClientIntent::EnterCharServer { account_id, .. } = &intent {
                        if authenticated_account.is_some() {
                            warn!(
                                "Session {:?} ({}) sent duplicate EnterCharServer after authentication; ignoring",
                                session_id, peer_addr
                            );
                            continue;
                        }
                        authenticated_account = Some(*account_id);
                        let echo = Bytes::copy_from_slice(&account_id.0.to_le_bytes());
                        if frame_tx.try_send(OutboundFrame::RawBytes(echo)).is_err() {
                            error!(
                                "Outbound queue full when echoing AccountId to {}",
                                peer_addr
                            );
                            break 'session_loop;
                        }
                    } else if authenticated_account.is_none() {
                        warn!(
                            "Session {:?} ({}) sent {:?} prior to EnterCharServer handshake",
                            session_id, peer_addr, intent
                        );
                        break 'session_loop;
                    }

                    let session_intent =
                        SessionIntent::new(session_id, authenticated_account, None, intent);
                    if intent_tx.send(session_intent).await.is_err() {
                        break 'session_loop;
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
    drop(frame_tx);

    if tokio::time::timeout(std::time::Duration::from_millis(500), &mut forwarder_task)
        .await
        .is_err()
    {
        forwarder_task.abort();
    }

    if tokio::time::timeout(std::time::Duration::from_millis(1000), &mut writer_task)
        .await
        .is_err()
    {
        warn!(
            "Session {:?} ({}) writer task timed out during termination",
            session_id, peer_addr
        );
        writer_task.abort();
    }
}

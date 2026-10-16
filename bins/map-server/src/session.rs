use std::net::SocketAddr;

use bytes::{Bytes, BytesMut};
use ro_contracts::{
    ids::{AccountId, CharId},
    packet::{ClientIntent, PacketIntentParser, SessionIntent},
    TARGET_PACKETVER,
};
use ro_packets::{PacketDecoder, Phase0IntentParser, RathenaPacketLengthTable};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::mpsc,
};
use tokio_util::codec::Decoder;
use tracing::{debug, error, info, warn};

use crate::service::{MapServiceHandle, WorldCommand};

pub async fn handle_client_session(
    socket: TcpStream,
    peer_addr: SocketAddr,
    service: MapServiceHandle,
) {
    let session_id = service.allocate_session_id();
    info!(
        "Accepted map client connection {:?} from {}",
        session_id, peer_addr
    );

    let (mut reader, mut writer) = socket.into_split();
    let (outbound_tx, mut outbound_rx) = mpsc::channel::<Bytes>(64);

    let writer_task = tokio::spawn(async move {
        while let Some(bytes) = outbound_rx.recv().await {
            if let Err(err) = writer.write_all(&bytes).await {
                error!(
                    "Failed to write outbound bytes to {:?} ({}): {}",
                    session_id, peer_addr, err
                );
                return;
            }
            if let Err(err) = writer.flush().await {
                error!(
                    "Failed to flush outbound bytes to {:?} ({}): {}",
                    session_id, peer_addr, err
                );
                return;
            }
        }
    });

    let mut decoder = PacketDecoder::new(RathenaPacketLengthTable, TARGET_PACKETVER);
    let mut buffer = BytesMut::with_capacity(2048);
    let parser = Phase0IntentParser;

    let mut authenticated_account: Option<AccountId> = None;
    let mut authenticated_char: Option<CharId> = None;
    let mut world_tx: Option<mpsc::Sender<WorldCommand>> = None;

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
                if let Some(tx) = &world_tx {
                    let _ = tx.try_send(WorldCommand::LeaveSession { session_id });
                }
                drop(outbound_tx);
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

                    match &intent {
                        ClientIntent::EnterMap {
                            account_id,
                            char_id,
                            ..
                        } => {
                            let entered_account_id = *account_id;
                            let entered_char_id = *char_id;
                            authenticated_account = Some(entered_account_id);
                            authenticated_char = Some(entered_char_id);

                            let echo = Bytes::copy_from_slice(&entered_account_id.0.to_le_bytes());
                            if outbound_tx.try_send(echo).is_err() {
                                error!(
                                    "Outbound queue full when echoing AccountId to {}",
                                    peer_addr
                                );
                                drop(outbound_tx);
                                let _ = writer_task.await;
                                return;
                            }

                            let target_world_tx = service.resolve_world_for_character(entered_char_id);
                            let session_intent = SessionIntent::new(
                                session_id,
                                authenticated_account,
                                authenticated_char,
                                intent,
                            );

                            if let Err(err) = target_world_tx
                                .send(WorldCommand::JoinSession {
                                    session_id,
                                    char_id: entered_char_id,
                                    outbound_tx: outbound_tx.clone(),
                                    initial_intent: session_intent,
                                })
                                .await
                            {
                                error!("Failed to join MapWorldActor: {}", err);
                                drop(outbound_tx);
                                let _ = writer_task.await;
                                return;
                            }
                            world_tx = Some(target_world_tx);
                        }
                        _ => {
                            let Some(tx) = &world_tx else {
                                warn!(
                                    "Session {:?} ({}) sent {:?} prior to EnterMap handshake",
                                    session_id, peer_addr, intent
                                );
                                drop(outbound_tx);
                                let _ = writer_task.await;
                                return;
                            };

                            let session_intent = SessionIntent::new(
                                session_id,
                                authenticated_account,
                                authenticated_char,
                                intent,
                            );
                            if let Err(err) =
                                tx.send(WorldCommand::PlayerIntent(session_intent)).await
                            {
                                error!("Failed to send intent to MapWorldActor: {}", err);
                                drop(outbound_tx);
                                let _ = writer_task.await;
                                return;
                            }
                        }
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

    if let Some(tx) = &world_tx {
        let _ = tx.try_send(WorldCommand::LeaveSession { session_id });
    }
    drop(outbound_tx);
    let _ = writer_task.await;
}

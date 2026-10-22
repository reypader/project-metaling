use std::net::SocketAddr;

use bytes::{Bytes, BytesMut};
use ro_core::{
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
use tokio_util::{codec::Decoder, sync::CancellationToken};
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
    let mut outbound_tx = Some(outbound_tx);

    let cancel_token = CancellationToken::new();
    let cancel_token_writer = cancel_token.clone();

    let mut writer_task = tokio::spawn(async move {
        while let Some(bytes) = outbound_rx.recv().await {
            if let Err(err) = writer.write_all(&bytes).await {
                error!(
                    "Failed to write outbound bytes to {:?} ({}): {}",
                    session_id, peer_addr, err
                );
                cancel_token_writer.cancel();
                return;
            }
            if let Err(err) = writer.flush().await {
                error!(
                    "Failed to flush outbound bytes to {:?} ({}): {}",
                    session_id, peer_addr, err
                );
                cancel_token_writer.cancel();
                return;
            }
        }
        cancel_token_writer.cancel();
    });

    let mut decoder = PacketDecoder::new(RathenaPacketLengthTable, TARGET_PACKETVER);
    let mut buffer = BytesMut::with_capacity(2048);
    let parser = Phase0IntentParser;

    let mut authenticated_account: Option<AccountId> = None;
    let mut authenticated_char: Option<CharId> = None;
    let mut world_tx: Option<mpsc::Sender<WorldCommand>> = None;

    'session_loop: loop {
        tokio::select! {
            _ = cancel_token.cancelled() => {
                info!(
                    "Session {:?} ({}) writer task finished or outbound queue closed; terminating session",
                    session_id, peer_addr
                );
                break 'session_loop;
            }
            read_result = reader.read_buf(&mut buffer) => {
                match read_result {
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

                    match &intent {
                        ClientIntent::EnterMap {
                            account_id,
                            char_id,
                            ..
                        } => {
                            if authenticated_account.is_some() {
                                warn!(
                                    "Session {:?} ({}) sent duplicate EnterMap after authentication; ignoring",
                                    session_id, peer_addr
                                );
                                continue;
                            }

                            let Some(tx) = outbound_tx.take() else {
                                warn!(
                                    "Session {:?} ({}) outbound sender unavailable for EnterMap",
                                    session_id, peer_addr
                                );
                                continue;
                            };

                            let entered_account_id = *account_id;
                            let entered_char_id = *char_id;
                            let Some((target_world_tx, character, spawn_pos)) = service
                                .resolve_entry(entered_account_id, entered_char_id)
                                .await
                            else {
                                // Unknown or foreign character: close without sending anything.
                                break 'session_loop;
                            };
                            authenticated_account = Some(entered_account_id);
                            authenticated_char = Some(entered_char_id);

                            // The actor answers with the entry burst, or drops `tx` (which ends
                            // this session) if the character can't enter.
                            if let Err(err) = target_world_tx
                                .send(WorldCommand::JoinSession {
                                    session_id,
                                    account_id: entered_account_id,
                                    character,
                                    spawn_pos,
                                    outbound_tx: tx,
                                })
                                .await
                            {
                                error!("Failed to join MapWorldActor: {}", err);
                                service.release_character(entered_char_id);
                                break 'session_loop;
                            }
                            world_tx = Some(target_world_tx);
                        }
                        _ => {
                            let Some(tx) = &world_tx else {
                                warn!(
                                    "Session {:?} ({}) sent {:?} prior to EnterMap handshake",
                                    session_id, peer_addr, intent
                                );
                                break 'session_loop;
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
                                break 'session_loop;
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
                    // Malformed or out-of-range client data (e.g. InvalidField): drop the session.
                    warn!(
                        "Closing {:?} ({}): bad client packet: {}",
                        session_id, peer_addr, err
                    );
                    break 'session_loop;
                }
            }
        }
    }

    cancel_token.cancel();

    if let Some(tx) = &world_tx {
        // No timeout: a lost leave would leave the character in the world. The actor always
        // drains its queue, so this completes.
        if tx
            .send(WorldCommand::LeaveSession { session_id })
            .await
            .is_err()
        {
            warn!(
                "Session {:?}: map actor gone before LeaveSession",
                session_id
            );
        }
    }
    drop(outbound_tx);

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

use std::net::SocketAddr;

use bytes::BytesMut;
use ro_contracts::{
    ids::SessionId,
    packet::{EventEncoder, PacketIntentParser, ServerEvent, SessionIntent},
    TARGET_PACKETVER,
};
use ro_packets::{PacketDecoder, Phase0EventEncoder, Phase0IntentParser, RathenaPacketLengthTable};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::TcpStream,
    sync::mpsc,
};
use tokio_util::codec::Decoder;
use tracing::{debug, error, info, warn};

use crate::service::LoginServiceHandle;

pub async fn handle_stream<R, W>(
    mut reader: R,
    mut writer: W,
    session_id: SessionId,
    intent_tx: mpsc::Sender<SessionIntent>,
    mut outbound_rx: mpsc::Receiver<ServerEvent>,
) where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let mut writer_task = tokio::spawn(async move {
        let encoder = Phase0EventEncoder;
        while let Some(event) = outbound_rx.recv().await {
            info!(
                "Sending event to login client {:?}: {:?}",
                session_id, event
            );
            match encoder.encode_event(&event, TARGET_PACKETVER) {
                Ok(bytes) => {
                    if let Err(err) = writer.write_all(&bytes).await {
                        error!("Failed to write event to client {:?}: {}", session_id, err);
                        return;
                    }
                    if let Err(err) = writer.flush().await {
                        error!("Failed to flush event to client {:?}: {}", session_id, err);
                        return;
                    }
                    if matches!(
                        event,
                        ServerEvent::LoginAccepted { .. } | ServerEvent::LoginRefused { .. }
                    ) {
                        let _ = writer.shutdown().await;
                        info!(
                            "Completed login handshake for {:?}. Closing socket.",
                            session_id
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

    let mut reader_task = tokio::spawn(async move {
        let mut decoder = PacketDecoder::new(RathenaPacketLengthTable, TARGET_PACKETVER);
        let mut buffer = BytesMut::with_capacity(2048);
        let parser = Phase0IntentParser;

        loop {
            match reader.read_buf(&mut buffer).await {
                Ok(0) => {
                    info!("Client {:?} closed connection", session_id);
                    break;
                }
                Ok(bytes_read) => {
                    debug!(
                        "Read {} bytes from client {:?}",
                        bytes_read, session_id
                    );
                }
                Err(err) => {
                    error!(
                        "Socket read error from client {:?}: {}",
                        session_id, err
                    );
                    break;
                }
            }

            while let Some(raw_packet) = match decoder.decode(&mut buffer) {
                Ok(frame) => frame,
                Err(err) => {
                    error!("Packet framing error for client {:?}: {}", session_id, err);
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
                        warn!("Packet parsing error for client {:?}: {}", session_id, err);
                    }
                }
            }
        }
    });

    tokio::select! {
        _ = &mut writer_task => {
            debug!("Writer task completed for session {:?}", session_id);
            reader_task.abort();
        }
        _ = &mut reader_task => {
            debug!("Reader task completed for session {:?}", session_id);
            writer_task.abort();
        }
    }
}

pub async fn handle_in_memory_session<R, W>(
    reader: R,
    writer: W,
    service: LoginServiceHandle,
) where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let session_id = service.allocate_session_id();
    let (outbound_tx, outbound_rx) = mpsc::channel::<ServerEvent>(16);
    let intent_tx = service.spawn_session_actor(session_id, outbound_tx);
    handle_stream(reader, writer, session_id, intent_tx, outbound_rx).await;
}

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

    let (reader, writer) = socket.into_split();
    let (outbound_tx, outbound_rx) = mpsc::channel::<ServerEvent>(16);
    let intent_tx = service.spawn_session_actor(session_id, outbound_tx);

    handle_stream(reader, writer, session_id, intent_tx, outbound_rx).await;
}

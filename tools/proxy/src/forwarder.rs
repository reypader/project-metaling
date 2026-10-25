use ro_core::ServerType;
use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tracing::{debug, error, info, warn};

use crate::logger::{
    PacketDirection, PacketLogger, PacketSequenceRecorder, SessionSequenceTracker,
};

static NEXT_SESSION_ID: AtomicUsize = AtomicUsize::new(1);

/// Writes and flushes `chunk` to `writer`. If that takes longer than [`SLOW_FORWARD`], writes a
/// note into the capture while it keeps waiting, so a stuck forward shows up even if it never ends.
async fn forward_chunk<W: AsyncWrite + Unpin>(
    writer: &mut W,
    chunk: &[u8],
    logger: &mut PacketLogger,
    peer: &str,
) -> std::io::Result<()> {
    let write = async {
        writer.write_all(chunk).await?;
        writer.flush().await
    };
    tokio::pin!(write);
    match tokio::time::timeout(SLOW_FORWARD, &mut write).await {
        Ok(result) => result,
        Err(_) => {
            logger.record_note(&format!(
                "forwarding {} bytes to the {peer} has been blocked for over {} ms; the {peer} may have stopped reading",
                chunk.len(),
                SLOW_FORWARD.as_millis()
            ));
            write.await
        }
    }
}

/// A forward slower than this gets a `[PROXY NOTE]` in the capture. A stuck write means the
/// receiving side stopped reading.
const SLOW_FORWARD: Duration = Duration::from_secs(1);

pub async fn run_proxy_server(
    listener: TcpListener,
    target_addr: String,
    server_type: ServerType,
    recorder: Arc<PacketSequenceRecorder>,
) -> Result<(), std::io::Error> {
    let local_addr = listener.local_addr()?;
    info!(
        "[{:?}] Proxy listener active on {} -> Forwarding to {}",
        server_type, local_addr, target_addr
    );

    loop {
        match listener.accept().await {
            Ok((client_stream, client_addr)) => {
                let session_id = NEXT_SESSION_ID.fetch_add(1, Ordering::SeqCst);
                let target = target_addr.clone();
                let rec = recorder.clone();
                tokio::spawn(async move {
                    handle_proxy_connection(
                        client_stream,
                        client_addr,
                        target,
                        server_type,
                        session_id,
                        rec,
                    )
                    .await;
                });
            }
            Err(err) => {
                warn!(
                    "[{:?}] Failed to accept connection: {}. Retrying in 100ms...",
                    server_type, err
                );
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                continue;
            }
        }
    }
}

pub async fn handle_proxy_connection(
    client_stream: TcpStream,
    client_addr: SocketAddr,
    target_addr: String,
    server_type: ServerType,
    session_id: usize,
    recorder: Arc<PacketSequenceRecorder>,
) {
    info!(
        "[{:?} Session #{}] Accepted client connection from {} -> connecting to target {}",
        server_type, session_id, client_addr, target_addr
    );

    let target_stream = match TcpStream::connect(&target_addr).await {
        Ok(stream) => stream,
        Err(err) => {
            error!(
                "[{:?} Session #{}] Failed to connect to target {} for client {}: {}",
                server_type, session_id, target_addr, client_addr, err
            );
            return;
        }
    };

    let (mut client_reader, mut client_writer) = client_stream.into_split();
    let (mut target_reader, mut target_writer) = target_stream.into_split();

    let tracker = Arc::new(SessionSequenceTracker::with_global(
        server_type,
        session_id,
        recorder,
    ));

    let client_to_target = {
        let target_addr = target_addr.clone();
        let tracker = tracker.clone();
        async move {
            let mut read_buf = [0u8; 8192];
            let mut logger = PacketLogger::new(
                server_type,
                session_id,
                PacketDirection::ClientToServer,
                tracker,
            );

            let stop_reason = loop {
                let n: usize = match client_reader.read(&mut read_buf).await {
                    Ok(0) => {
                        debug!(
                            "[{:?} Session #{}] Client {} sent EOF",
                            server_type, session_id, client_addr
                        );
                        break "client closed the connection (EOF)";
                    }
                    Ok(n) => n,
                    Err(err) => {
                        debug!(
                            "[{:?} Session #{}] Read error from client {}: {}",
                            server_type, session_id, client_addr, err
                        );
                        break "read error from client";
                    }
                };

                let chunk = &read_buf[..n];

                let forward_started = Instant::now();
                // 1. Immediately forward raw bytes to target as-is
                if let Err(err) =
                    forward_chunk(&mut target_writer, chunk, &mut logger, "server").await
                {
                    error!(
                        "[{:?} Session #{}] Failed to forward {} bytes from client {} to target {}: {}",
                        server_type, session_id, n, client_addr, target_addr, err
                    );
                    break "forwarding to server failed";
                }

                // 2. Feed into logger
                logger.feed_and_log(chunk);

                let forward_time = forward_started.elapsed();
                if forward_time >= SLOW_FORWARD {
                    logger.record_note(&format!(
                        "the {} bytes above took {} ms to forward to the server",
                        n,
                        forward_time.as_millis()
                    ));
                }
            };

            logger.flush();
            let _ = target_writer.shutdown().await;
            stop_reason
        }
    };

    let target_to_client = {
        let target_addr = target_addr.clone();
        let tracker = tracker.clone();
        async move {
            let mut read_buf = [0u8; 8192];
            let mut logger = PacketLogger::new(
                server_type,
                session_id,
                PacketDirection::ServerToClient,
                tracker,
            );

            let stop_reason = loop {
                let n: usize = match target_reader.read(&mut read_buf).await {
                    Ok(0) => {
                        debug!(
                            "[{:?} Session #{}] Target {} sent EOF to client {}",
                            server_type, session_id, target_addr, client_addr
                        );
                        break "server closed the connection (EOF)";
                    }
                    Ok(n) => n,
                    Err(err) => {
                        debug!(
                            "[{:?} Session #{}] Read error from target {}: {}",
                            server_type, session_id, target_addr, err
                        );
                        break "read error from server";
                    }
                };

                let chunk = &read_buf[..n];

                let forward_started = Instant::now();
                // 1. Immediately forward raw bytes to client as-is
                if let Err(err) =
                    forward_chunk(&mut client_writer, chunk, &mut logger, "client").await
                {
                    error!(
                        "[{:?} Session #{}] Failed to forward {} bytes from target {} to client {}: {}",
                        server_type, session_id, n, target_addr, client_addr, err
                    );
                    break "forwarding to client failed";
                }

                // 2. Feed into logger
                logger.feed_and_log(chunk);

                let forward_time = forward_started.elapsed();
                if forward_time >= SLOW_FORWARD {
                    logger.record_note(&format!(
                        "the {} bytes above took {} ms to forward to the client",
                        n,
                        forward_time.as_millis()
                    ));
                }
            };

            logger.flush();
            let _ = client_writer.shutdown().await;
            stop_reason
        }
    };

    tokio::select! {
        reason = client_to_target => {
            debug!("[{:?} Session #{}] Client -> Target stream finished for {}: {}", server_type, session_id, client_addr, reason);
            tracker.record_note(PacketDirection::ClientToServer, &format!("session closed: {reason}"));
        }
        reason = target_to_client => {
            debug!("[{:?} Session #{}] Target -> Client stream finished for {}: {}", server_type, session_id, client_addr, reason);
            tracker.record_note(PacketDirection::ServerToClient, &format!("session closed: {reason}"));
        }
    }

    info!(
        "[{:?} Session #{}] Connection closed for client {} <-> target {} (recorded {} packets)",
        server_type,
        session_id,
        client_addr,
        target_addr,
        tracker.packet_count()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map_logger(tracker: &Arc<SessionSequenceTracker>) -> PacketLogger {
        PacketLogger::new(
            ServerType::Map,
            1,
            PacketDirection::ServerToClient,
            tracker.clone(),
        )
    }

    #[tokio::test(start_paused = true)]
    async fn stuck_forward_is_noted_while_still_blocked() {
        // The far end never reads, so writing more than the pipe holds blocks forever.
        let (mut writer, _never_read) = tokio::io::duplex(8);
        let tracker = Arc::new(SessionSequenceTracker::new());
        let mut logger = map_logger(&tracker);

        let result = tokio::time::timeout(
            SLOW_FORWARD * 2,
            forward_chunk(&mut writer, &[0u8; 64], &mut logger, "client"),
        )
        .await;

        assert!(result.is_err(), "the write is still blocked");
        let flow = tracker.format_sequence_flow();
        assert!(
            flow.contains("forwarding 64 bytes to the client has been blocked for over 1000 ms"),
            "{flow}"
        );
    }

    #[tokio::test]
    async fn quick_forward_writes_no_note() {
        let (mut writer, mut reader) = tokio::io::duplex(64);
        let tracker = Arc::new(SessionSequenceTracker::new());
        let mut logger = map_logger(&tracker);

        forward_chunk(&mut writer, &[1, 2, 3], &mut logger, "client")
            .await
            .expect("forward succeeds");

        let mut got = [0u8; 3];
        reader.read_exact(&mut got).await.expect("bytes arrive");
        assert_eq!(got, [1, 2, 3]);
        assert_eq!(tracker.packet_count(), 0);
    }
}

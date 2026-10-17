use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use ro_contracts::http::ServerType;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tracing::{debug, error, info};

use crate::logger::{PacketDirection, PacketLogger, PacketSequenceRecorder, SessionSequenceTracker};

static NEXT_SESSION_ID: AtomicUsize = AtomicUsize::new(1);

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
                    handle_proxy_connection(client_stream, client_addr, target, server_type, session_id, rec).await;
                });
            }
            Err(err) => {
                error!("[{:?}] Failed to accept connection: {}", server_type, err);
                break;
            }
        }
    }

    Ok(())
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

            loop {
                let n = match client_reader.read(&mut read_buf).await {
                    Ok(0) => {
                        debug!("[{:?} Session #{}] Client {} sent EOF", server_type, session_id, client_addr);
                        break;
                    }
                    Ok(n) => n,
                    Err(err) => {
                        debug!("[{:?} Session #{}] Read error from client {}: {}", server_type, session_id, client_addr, err);
                        break;
                    }
                };

                let chunk = &read_buf[..n];

                // 1. Immediately forward raw bytes to target as-is
                if let Err(err) = target_writer.write_all(chunk).await {
                    error!(
                        "[{:?} Session #{}] Failed to forward {} bytes from client {} to target {}: {}",
                        server_type, session_id, n, client_addr, target_addr, err
                    );
                    break;
                }
                if let Err(err) = target_writer.flush().await {
                    error!(
                        "[{:?} Session #{}] Failed to flush bytes to target {}: {}",
                        server_type, session_id, target_addr, err
                    );
                    break;
                }

                // 2. Feed into logger
                logger.feed_and_log(chunk);
            }

            let _ = target_writer.shutdown().await;
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

            loop {
                let n = match target_reader.read(&mut read_buf).await {
                    Ok(0) => {
                        debug!("[{:?} Session #{}] Target {} sent EOF to client {}", server_type, session_id, target_addr, client_addr);
                        break;
                    }
                    Ok(n) => n,
                    Err(err) => {
                        debug!("[{:?} Session #{}] Read error from target {}: {}", server_type, session_id, target_addr, err);
                        break;
                    }
                };

                let chunk = &read_buf[..n];

                // 1. Immediately forward raw bytes to client as-is
                if let Err(err) = client_writer.write_all(chunk).await {
                    error!(
                        "[{:?} Session #{}] Failed to forward {} bytes from target {} to client {}: {}",
                        server_type, session_id, n, target_addr, client_addr, err
                    );
                    break;
                }
                if let Err(err) = client_writer.flush().await {
                    error!(
                        "[{:?} Session #{}] Failed to flush bytes to client {}: {}",
                        server_type, session_id, client_addr, err
                    );
                    break;
                }

                // 2. Feed into logger
                logger.feed_and_log(chunk);
            }

            let _ = client_writer.shutdown().await;
        }
    };

    tokio::select! {
        _ = client_to_target => {
            debug!("[{:?} Session #{}] Client -> Target stream finished for {}", server_type, session_id, client_addr);
        }
        _ = target_to_client => {
            debug!("[{:?} Session #{}] Target -> Client stream finished for {}", server_type, session_id, client_addr);
        }
    }

    info!(
        "[{:?} Session #{}] Connection closed for client {} <-> target {} (recorded {} packets)",
        server_type, session_id, client_addr, target_addr, tracker.packet_count()
    );
}

use std::{net::SocketAddr, sync::Arc, time::Duration};

use anyhow::Context;
use tokio::net::TcpListener;
use tracing::{error, info, warn};

use char_server::{create_http_router, handle_client_session, CharConfig, CharServiceHandle};
use ro_contracts::TARGET_PACKETVER;
use ro_persistence::PostgresRepo;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let config = CharConfig::from_env();
    info!(
        "Starting char-server (Target PACKETVER: {}, Client TCP: {}, Internal HTTP: {}, Advertised Port: {}, Default Map Port: {})",
        TARGET_PACKETVER, config.client_port, config.http_port, config.advertised_port, config.default_map_port
    );

    let char_repo = Arc::new(
        PostgresRepo::connect(&config.database_url)
            .await
            .with_context(|| format!("Failed to connect to database at {}", config.database_url))?,
    );
    let service_handle = CharServiceHandle::with_default_map_port(char_repo, config.default_map_port);

    let http_router = create_http_router(service_handle.clone());

    let http_addr: SocketAddr = format!("0.0.0.0:{}", config.http_port)
        .parse()
        .context("Failed to parse char-server HTTP address")?;
    let http_listener = TcpListener::bind(http_addr)
        .await
        .with_context(|| format!("Failed to bind char-server HTTP listener to {}", http_addr))?;
    info!("Character HTTP internal API listening on {}", http_addr);

    let client_addr: SocketAddr = format!("0.0.0.0:{}", config.client_port)
        .parse()
        .context("Failed to parse char-server client TCP address")?;
    let client_listener = TcpListener::bind(client_addr)
        .await
        .with_context(|| format!("Failed to bind char-server client TCP listener to {}", client_addr))?;
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
                    warn!("Transient error accepting client connection: {}", err);
                    tokio::time::sleep(Duration::from_millis(100)).await;
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

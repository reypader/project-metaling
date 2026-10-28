use std::{net::SocketAddr, sync::Arc, time::Duration};

use anyhow::Context;
use tokio::net::TcpListener;
use tracing::{error, info, warn};

use map_server::{
    create_http_router, handle_client_session, load_registries, map_data_loader, MapConfig,
    MapServiceHandle,
};
use ro_core::TARGET_PACKETVER;
use ro_persistence::PostgresRepo;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let config = MapConfig::from_env();
    info!(
        "Starting map-server (Target PACKETVER: {}, Client TCP: {}, Internal HTTP: {}, Advertised Port: {})",
        TARGET_PACKETVER, config.client_port, config.http_port, config.advertised_port
    );

    let char_repo = Arc::new(
        PostgresRepo::connect(&config.database_url)
            .await
            .with_context(|| format!("Failed to connect to database at {}", config.database_url))?,
    );
    if config.npc_data_dir.is_none() {
        warn!("NPC_DATA_DIR not set; maps will have no NPCs");
    }
    if config.mob_spawn_data_dir.is_none() {
        warn!("MOB_SPAWN_DATA_DIR not set; maps will have no mobs");
    }
    char_repo
        .init_schema()
        .await
        .context("Failed to initialize the database schema")?;
    if config.job_data_dir.is_none() {
        anyhow::bail!(
            "JOB_DATA_DIR is not set; the job data (crates/ro-db/data/job) is required for stats"
        );
    }
    let registries = load_registries(config.registry_paths())
        .context("Failed to load the registries or the battle config")?;
    let service_handle = MapServiceHandle::new(
        char_repo,
        map_data_loader(
            config.map_data_dir.clone(),
            config.npc_data_dir.clone(),
            config.mob_spawn_data_dir.clone(),
            registries,
        ),
    );

    let http_router = create_http_router(service_handle.clone());

    let http_addr: SocketAddr = format!("0.0.0.0:{}", config.http_port)
        .parse()
        .context("Failed to parse map-server HTTP address")?;
    let http_listener = TcpListener::bind(http_addr)
        .await
        .with_context(|| format!("Failed to bind map-server HTTP listener to {}", http_addr))?;
    info!("Map HTTP internal API listening on {}", http_addr);

    let client_addr: SocketAddr = format!("0.0.0.0:{}", config.client_port)
        .parse()
        .context("Failed to parse map-server client TCP address")?;
    let client_listener = TcpListener::bind(client_addr).await.with_context(|| {
        format!(
            "Failed to bind map-server client TCP listener to {}",
            client_addr
        )
    })?;
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

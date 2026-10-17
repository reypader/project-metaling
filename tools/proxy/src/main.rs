use std::sync::Arc;
use clap::Parser;
use ro_contracts::http::ServerType;
use ro_proxy::{
    config::ProxyArgs, forwarder::run_proxy_server, generate_log_filename, PacketSequenceRecorder,
};
use tokio::net::TcpListener;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = ProxyArgs::parse();
    let config = args
        .build_config()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

    info!("============================================================");
    info!(
        "Starting Ragnarok Online Packet Proxy (Target PACKETVER: {})",
        ro_contracts::TARGET_PACKETVER
    );
    info!(
        "  Login Proxy: {}:{} -> Target {}",
        config.bind_addr, config.login_listen_port, config.login_target
    );
    info!(
        "  Char Proxy:  {}:{} -> Target {}",
        config.bind_addr, config.char_listen_port, config.char_target
    );
    info!(
        "  Map Proxy:   {}:{} -> Target {}",
        config.bind_addr, config.map_listen_port, config.map_target
    );
    info!("============================================================");

    let recorder = Arc::new(PacketSequenceRecorder::new());

    let login_listener =
        TcpListener::bind(format!("{}:{}", config.bind_addr, config.login_listen_port)).await?;
    let char_listener =
        TcpListener::bind(format!("{}:{}", config.bind_addr, config.char_listen_port)).await?;
    let map_listener =
        TcpListener::bind(format!("{}:{}", config.bind_addr, config.map_listen_port)).await?;

    let login_target = config.login_target;
    let login_recorder = recorder.clone();
    let login_task = tokio::spawn(async move {
        if let Err(err) = run_proxy_server(login_listener, login_target, ServerType::Login, login_recorder).await {
            error!("[Login] Server listener error: {}", err);
        }
    });

    let char_target = config.char_target;
    let char_recorder = recorder.clone();
    let char_task = tokio::spawn(async move {
        if let Err(err) = run_proxy_server(char_listener, char_target, ServerType::Char, char_recorder).await {
            error!("[Char] Server listener error: {}", err);
        }
    });

    let map_target = config.map_target;
    let map_recorder = recorder.clone();
    let map_task = tokio::spawn(async move {
        if let Err(err) = run_proxy_server(map_listener, map_target, ServerType::Map, map_recorder).await {
            error!("[Map] Server listener error: {}", err);
        }
    });

    tokio::select! {
        _ = login_task => info!("Login proxy task terminated"),
        _ = char_task => info!("Char proxy task terminated"),
        _ = map_task => info!("Map proxy task terminated"),
        _ = tokio::signal::ctrl_c() => info!("Shutdown signal received, closing proxy"),
    }

    let now = std::time::SystemTime::now();
    let log_path = config
        .log_file
        .unwrap_or_else(|| generate_log_filename(now));

    match recorder.write_to_file(&log_path, now) {
        Ok(count) => {
            info!(
                "Saved packet sequence log ({} packets) to {}",
                count, log_path
            );
        }
        Err(err) => {
            error!(
                "Failed to write packet sequence log to {}: {}",
                log_path, err
            );
        }
    }

    Ok(())
}

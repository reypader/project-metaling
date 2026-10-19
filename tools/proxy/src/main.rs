use std::{io::ErrorKind, sync::Arc, time::SystemTime};
use anyhow::{Context, Result};
use clap::Parser;
use ro_contracts::{http::ServerType, TARGET_PACKETVER};
use ro_proxy::{
    config::{format_host_port, ProxyArgs},
    forwarder::run_proxy_server,
    generate_log_filename, PacketSequenceRecorder,
};
use tokio::net::TcpListener;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = ProxyArgs::parse();
    let config = args
        .build_config()
        .context("Failed to build proxy configuration from command-line arguments")?;

    let login_bind = format_host_port(&config.bind_addr, config.login_listen_port);
    let char_bind = format_host_port(&config.bind_addr, config.char_listen_port);
    let map_bind = format_host_port(&config.bind_addr, config.map_listen_port);

    info!("============================================================");
    info!(
        "Starting Ragnarok Online Packet Proxy (Target PACKETVER: {})",
        TARGET_PACKETVER
    );
    info!(
        "  Login Proxy: {} -> Target {}",
        login_bind, config.login_target
    );
    info!(
        "  Char Proxy:  {} -> Target {}",
        char_bind, config.char_target
    );
    info!(
        "  Map Proxy:   {} -> Target {}",
        map_bind, config.map_target
    );
    info!("============================================================");

    let recorder = Arc::new(PacketSequenceRecorder::new());

    let login_listener = match TcpListener::bind(&login_bind).await {
        Ok(listener) => listener,
        Err(err) => {
            if err.kind() == ErrorKind::AddrInUse {
                error!("Failed to bind Login listener on {}: Address already in use", login_bind);
            } else {
                error!("Failed to bind Login listener on {}: {}", login_bind, err);
            }
            return Err(err).with_context(|| format!("Failed to bind Login listener on {}", login_bind));
        }
    };
    let char_listener = match TcpListener::bind(&char_bind).await {
        Ok(listener) => listener,
        Err(err) => {
            if err.kind() == ErrorKind::AddrInUse {
                error!("Failed to bind Char listener on {}: Address already in use", char_bind);
            } else {
                error!("Failed to bind Char listener on {}: {}", char_bind, err);
            }
            return Err(err).with_context(|| format!("Failed to bind Char listener on {}", char_bind));
        }
    };
    let map_listener = match TcpListener::bind(&map_bind).await {
        Ok(listener) => listener,
        Err(err) => {
            if err.kind() == ErrorKind::AddrInUse {
                error!("Failed to bind Map listener on {}: Address already in use", map_bind);
            } else {
                error!("Failed to bind Map listener on {}: {}", map_bind, err);
            }
            return Err(err).with_context(|| format!("Failed to bind Map listener on {}", map_bind));
        }
    };

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

    let shutdown_signal = async {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};

            let mut sigterm = match signal(SignalKind::terminate()) {
                Ok(sig) => sig,
                Err(err) => {
                    error!("Failed to register SIGTERM handler: {}", err);
                    let _ = tokio::signal::ctrl_c().await;
                    return;
                }
            };
            tokio::select! {
                _ = tokio::signal::ctrl_c() => info!("Shutdown signal (SIGINT) received, closing proxy"),
                _ = sigterm.recv() => info!("Shutdown signal (SIGTERM) received, closing proxy"),
            }
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
            info!("Shutdown signal (SIGINT) received, closing proxy");
        }
    };

    tokio::select! {
        _ = login_task => info!("Login proxy task terminated"),
        _ = char_task => info!("Char proxy task terminated"),
        _ = map_task => info!("Map proxy task terminated"),
        _ = shutdown_signal => {},
    }

    let now = SystemTime::now();
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

use tracing::info;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    info!(
        "Starting map-server (Target PACKETVER: {}, Default Port: 5121)",
        ro_contracts::TARGET_PACKETVER
    );
}

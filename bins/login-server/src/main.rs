use tracing::info;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    info!(
        "Starting login-server (Target PACKETVER: {}, Default Port: 6900)",
        ro_contracts::TARGET_PACKETVER
    );
}

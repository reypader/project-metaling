use tracing::info;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    info!(
        "Starting char-server (Target PACKETVER: {}, Default Port: 6121)",
        ro_contracts::TARGET_PACKETVER
    );
}

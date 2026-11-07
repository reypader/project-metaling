//! A map-server on a free local port, backed by the in-memory repo.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use map_server::{handle_client_session, MapDataLoader, MapServiceHandle, MapServiceOptions};
use ro_core::{CharId, MapPosition};
use tokio::net::TcpListener;

use super::*;

pub struct TestServer {
    pub addr: SocketAddr,
    pub service: MapServiceHandle,
    pub repo: Arc<InMemoryRepo>,
}

impl TestServer {
    pub async fn start(chars: &[(u32, u16, u16)], walls: &[(u16, u16)]) -> Self {
        Self::start_with(InMemoryRepo::seeded(chars), walls).await
    }

    pub async fn start_with(repo: InMemoryRepo, walls: &[(u16, u16)]) -> Self {
        Self::start_with_loader(repo, grid_loader(walls)).await
    }

    pub async fn start_with_loader(repo: InMemoryRepo, loader: MapDataLoader) -> Self {
        Self::start_with_options(repo, loader, MapServiceOptions::default()).await
    }

    pub async fn start_with_options(
        repo: InMemoryRepo,
        loader: MapDataLoader,
        options: MapServiceOptions,
    ) -> Self {
        let repo = Arc::new(repo);
        let service = MapServiceHandle::with_options(repo.clone(), loader, options);
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("local addr");
        let srv = service.clone();
        tokio::spawn(async move {
            while let Ok((socket, peer)) = listener.accept().await {
                tokio::spawn(handle_client_session(socket, peer, srv.clone()));
            }
        });
        Self {
            addr,
            service,
            repo,
        }
    }

    /// Connects, enters as character `n`, reads the 23-byte burst and sends ACTORINIT. The
    /// friends list after the burst is consumed by the `collect`.
    pub async fn join(&self, n: u32) -> Client {
        let mut client = Client::connect(self.addr).await;
        client.send(&cz_enter(n)).await;
        let burst = client.read_exact_bytes(23).await;
        assert_eq!(&burst[0..2], &[0x83, 0x02], "burst starts with ZC_AID");
        client.send(&[0x7d, 0x00]).await;
        // Let the next tick process ACTORINIT.
        client.collect(Duration::from_millis(120)).await;
        client
    }

    pub async fn position(&self, n: u32) -> Option<MapPosition> {
        self.position_on(MAP, n).await
    }

    pub async fn position_on(&self, map: &str, n: u32) -> Option<MapPosition> {
        self.service
            .character_position(map, CharId(char_id(n)))
            .await
    }

    /// Polls until `n` stands at `cell` or `timeout` passes.
    pub async fn wait_position(&self, n: u32, cell: (u16, u16), timeout: Duration) -> bool {
        let deadline = tokio::time::Instant::now() + timeout;
        while tokio::time::Instant::now() < deadline {
            if self.position(n).await.map(|p| (p.x, p.y)) == Some(cell) {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        false
    }

    pub async fn aoi(&self, x: u16, y: u16) -> Vec<CharId> {
        let mut chars = self
            .service
            .aoi_characters(MAP, MapPosition { x, y, dir: 0 })
            .await;
        chars.sort();
        chars
    }
}

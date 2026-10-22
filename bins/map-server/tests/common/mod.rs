//! Shared harness for map-server tests: an in-memory character repo, an injected 200x200
//! grid, and a packet-level test client.
#![allow(clippy::unwrap_used, clippy::expect_used, dead_code)]

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use bytes::BytesMut;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::Decoder;

use map_server::{handle_client_session, MapServiceHandle, WalkabilityLoader};
use ro_core::{
    AccountId, CancelDeleteOutcome, CellType, CharId, CharacterRepository, CharacterSummary,
    CreateCharacterError, CreateCharacterRequest, DeleteReserveOutcome, MapPosition,
    MapWalkability, PacketOpcode, RawPacket, RepositoryError, Sex, TARGET_PACKETVER,
};
use ro_db::MemoryMapGrid;
use ro_packets::{pack_pos_dir, unpack_pos2, PacketDecoder, RathenaPacketLengthTable};

pub const MAP: &str = "prontera.gat";

pub fn account(n: u32) -> u32 {
    2_000_000 + n
}
pub fn char_id(n: u32) -> u32 {
    150_000 + n
}

/// Characters keyed by char id; positions update on `update_position`.
#[derive(Default)]
pub struct InMemoryRepo {
    chars: Mutex<HashMap<CharId, CharacterSummary>>,
    /// Simulated latency of `update_position` (a slow database).
    persist_delay: Mutex<Duration>,
}

impl InMemoryRepo {
    /// Characters `n` (char 15000n, account 200000n) named `Char<n>` at the given cells.
    pub fn seeded(chars: &[(u32, u16, u16)]) -> Self {
        let repo = Self::default();
        for &(n, x, y) in chars {
            repo.chars.lock().unwrap().insert(
                CharId(char_id(n)),
                CharacterSummary {
                    char_id: CharId(char_id(n)),
                    account_id: AccountId(account(n)),
                    slot: 0,
                    name: format!("Char{n}"),
                    sex: Sex::Female,
                    job_class: 0,
                    base_level: 10,
                    job_level: 1,
                    hair_style: 2,
                    hair_color: 3,
                    last_map: MAP.to_string(),
                    last_pos: MapPosition { x, y, dir: 0 },
                    delete_date: None,
                },
            );
        }
        repo
    }

    pub fn with_persist_delay(self, delay: Duration) -> Self {
        *self.persist_delay.lock().unwrap() = delay;
        self
    }

    pub fn stored(&self, n: u32) -> Option<(String, MapPosition)> {
        self.chars
            .lock()
            .unwrap()
            .get(&CharId(char_id(n)))
            .map(|c| (c.last_map.clone(), c.last_pos))
    }
}

#[async_trait]
impl CharacterRepository for InMemoryRepo {
    async fn list_by_account(
        &self,
        account_id: AccountId,
    ) -> Result<Vec<CharacterSummary>, RepositoryError> {
        Ok(self
            .chars
            .lock()
            .unwrap()
            .values()
            .filter(|c| c.account_id == account_id)
            .cloned()
            .collect())
    }
    async fn find_by_slot(
        &self,
        account_id: AccountId,
        slot: u8,
    ) -> Result<Option<CharacterSummary>, RepositoryError> {
        Ok(self
            .chars
            .lock()
            .unwrap()
            .values()
            .find(|c| c.account_id == account_id && c.slot == slot)
            .cloned())
    }
    async fn find_by_id(&self, id: CharId) -> Result<Option<CharacterSummary>, RepositoryError> {
        Ok(self.chars.lock().unwrap().get(&id).cloned())
    }
    async fn create_character(
        &self,
        _req: CreateCharacterRequest,
        _max_slots: u8,
    ) -> Result<CharacterSummary, CreateCharacterError> {
        Err(CreateCharacterError::CreationDenied)
    }
    async fn reserve_delete(
        &self,
        _account_id: AccountId,
        _char_id: CharId,
        _delay_seconds: u32,
    ) -> Result<DeleteReserveOutcome, RepositoryError> {
        Ok(DeleteReserveOutcome::Denied)
    }
    async fn cancel_delete(
        &self,
        _account_id: AccountId,
        _char_id: CharId,
    ) -> Result<CancelDeleteOutcome, RepositoryError> {
        Ok(CancelDeleteOutcome::NotFound)
    }
    async fn update_position(
        &self,
        id: CharId,
        map_name: &str,
        pos: MapPosition,
    ) -> Result<(), RepositoryError> {
        let delay = *self.persist_delay.lock().unwrap();
        if !delay.is_zero() {
            tokio::time::sleep(delay).await;
        }
        if let Some(c) = self.chars.lock().unwrap().get_mut(&id) {
            c.last_map = map_name.to_string();
            c.last_pos = pos;
        }
        Ok(())
    }
}

/// Loader handing every map the same 200x200 grid with `walls`.
pub fn grid_loader(walls: &[(u16, u16)]) -> WalkabilityLoader {
    let mut grid = MemoryMapGrid::all_walkable(200, 200).expect("valid dims");
    for &(x, y) in walls {
        assert!(grid.set(x, y, CellType::Wall));
    }
    let shared: Arc<dyn MapWalkability> = grid.into();
    Arc::new(move |_map: &str| Arc::clone(&shared))
}

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
        let repo = Arc::new(repo);
        let service = MapServiceHandle::new(repo.clone(), grid_loader(walls));
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

    /// Connects, enters as character `n`, reads the 23-byte burst and sends ACTORINIT.
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
        self.service
            .character_position(MAP, CharId(char_id(n)))
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

pub struct Client {
    stream: TcpStream,
    decoder: PacketDecoder<RathenaPacketLengthTable>,
    buf: BytesMut,
    /// Frames read but not yet consumed by `wait_for`.
    pending: Vec<RawPacket>,
}

impl Client {
    pub async fn connect(addr: SocketAddr) -> Self {
        Self {
            stream: TcpStream::connect(addr).await.expect("connect"),
            decoder: PacketDecoder::new(RathenaPacketLengthTable, TARGET_PACKETVER),
            buf: BytesMut::with_capacity(4096),
            pending: Vec::new(),
        }
    }

    pub async fn send(&mut self, bytes: &[u8]) {
        self.stream.write_all(bytes).await.expect("write");
        self.stream.flush().await.expect("flush");
    }

    pub async fn read_exact_bytes(&mut self, n: usize) -> Vec<u8> {
        let mut out = vec![0u8; n];
        tokio::time::timeout(Duration::from_secs(2), self.stream.read_exact(&mut out))
            .await
            .expect("timed out reading burst")
            .expect("read burst");
        out
    }

    /// Reads until EOF or `timeout`; returns the raw bytes and whether EOF was seen.
    pub async fn read_until_eof(&mut self, timeout: Duration) -> (Vec<u8>, bool) {
        let mut out = Vec::new();
        let mut chunk = [0u8; 512];
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(tokio::time::Instant::now());
            if left.is_zero() {
                return (out, false);
            }
            match tokio::time::timeout(left, self.stream.read(&mut chunk)).await {
                Ok(Ok(0)) | Ok(Err(_)) => return (out, true),
                Ok(Ok(n)) => out.extend_from_slice(&chunk[..n]),
                Err(_) => return (out, false),
            }
        }
    }

    async fn fill(&mut self, wait: Duration) -> bool {
        match tokio::time::timeout(wait, self.stream.read_buf(&mut self.buf)).await {
            Ok(Ok(0)) | Ok(Err(_)) => false,
            Ok(Ok(_)) => {
                while let Some(frame) = self.decoder.decode(&mut self.buf).expect("clean framing") {
                    self.pending.push(frame);
                }
                true
            }
            Err(_) => true,
        }
    }

    /// Returns every frame that arrives within `window` (plus anything already buffered).
    pub async fn collect(&mut self, window: Duration) -> Vec<RawPacket> {
        let deadline = tokio::time::Instant::now() + window;
        loop {
            let left = deadline.saturating_duration_since(tokio::time::Instant::now());
            if left.is_zero() || !self.fill(left).await {
                break;
            }
        }
        std::mem::take(&mut self.pending)
    }

    /// Waits for the first frame matching `pred`; earlier non-matching frames are kept.
    pub async fn wait_for(
        &mut self,
        timeout: Duration,
        pred: impl Fn(&RawPacket) -> bool,
    ) -> Option<RawPacket> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if let Some(i) = self.pending.iter().position(&pred) {
                return Some(self.pending.remove(i));
            }
            let left = deadline.saturating_duration_since(tokio::time::Instant::now());
            if left.is_zero() || !self.fill(left).await {
                return None;
            }
        }
    }

    pub async fn wait_opcode(&mut self, opcode: u16, timeout: Duration) -> Option<RawPacket> {
        self.wait_for(timeout, |f| f.opcode == PacketOpcode(opcode))
            .await
    }

    /// Half-closes: sends EOF but keeps reading.
    pub async fn close_write(&mut self) {
        let _ = self.stream.shutdown().await;
    }

    /// Closes the write side and drops the connection.
    pub async fn close(mut self) {
        let _ = self.stream.shutdown().await;
    }
}

// ---------- frame helpers ----------

/// Full frame length (opcode + payload) for assertions on packet sizes.
pub fn frame_len(frame: &RawPacket) -> usize {
    frame.payload.len() + 2 + if is_dynamic(frame.opcode.0) { 2 } else { 0 }
}

fn is_dynamic(op: u16) -> bool {
    (0x09fd..=0x09ff).contains(&op)
}

/// u32 at a FRAME offset (offset 0 = opcode). Handles the dynamic length prefix.
pub fn frame_u32(frame: &RawPacket, offset: usize) -> u32 {
    let base = if is_dynamic(frame.opcode.0) { 4 } else { 2 };
    let p = &frame.payload[offset - base..offset - base + 4];
    u32::from_le_bytes(p.try_into().unwrap())
}

pub fn frame_u16(frame: &RawPacket, offset: usize) -> u16 {
    let base = if is_dynamic(frame.opcode.0) { 4 } else { 2 };
    let p = &frame.payload[offset - base..offset - base + 2];
    u16::from_le_bytes(p.try_into().unwrap())
}

pub fn frame_u8(frame: &RawPacket, offset: usize) -> u8 {
    let base = if is_dynamic(frame.opcode.0) { 4 } else { 2 };
    frame.payload[offset - base]
}

/// AID of a unit entry packet (0x09fd/0x09fe/0x09ff).
pub fn entry_aid(frame: &RawPacket) -> u32 {
    frame_u32(frame, 5)
}

pub fn entry_gid(frame: &RawPacket) -> u32 {
    frame_u32(frame, 9)
}

/// (x0, y0, x1, y1, sx, sy) of a 0x0087 frame.
pub fn playermove(frame: &RawPacket) -> (u16, u16, u16, u16, u8, u8) {
    let mv: [u8; 6] = frame.payload[4..10].try_into().unwrap();
    unpack_pos2(&mv)
}

pub fn is_entry(frame: &RawPacket) -> bool {
    is_dynamic(frame.opcode.0)
}

// ---------- client packets ----------

pub fn cz_enter(n: u32) -> Vec<u8> {
    cz_enter_raw(account(n), char_id(n))
}

pub fn cz_enter_raw(account_id: u32, char_id: u32) -> Vec<u8> {
    let mut b = vec![0x36, 0x04];
    b.extend_from_slice(&account_id.to_le_bytes());
    b.extend_from_slice(&char_id.to_le_bytes());
    b.extend_from_slice(&12345u32.to_le_bytes());
    b.extend_from_slice(&100_000u32.to_le_bytes());
    b.extend_from_slice(&[0u8; 4]);
    b.push(1);
    b
}

pub fn cz_request_move(x: u16, y: u16) -> Vec<u8> {
    let mut b = vec![0x5f, 0x03];
    b.extend_from_slice(&pack_pos_dir(x, y, 0));
    b
}

pub fn cz_change_direction(head: u8, body: u8) -> Vec<u8> {
    vec![0x61, 0x03, head, 0, body]
}

pub fn cz_reqname(gid: u32) -> Vec<u8> {
    let mut b = vec![0x68, 0x03];
    b.extend_from_slice(&gid.to_le_bytes());
    b
}

pub fn cz_request_act(action: u8) -> Vec<u8> {
    let mut b = vec![0x37, 0x04];
    b.extend_from_slice(&0u32.to_le_bytes());
    b.push(action);
    b
}

pub fn cz_request_time(tick: u32) -> Vec<u8> {
    let mut b = vec![0x60, 0x03];
    b.extend_from_slice(&tick.to_le_bytes());
    b
}

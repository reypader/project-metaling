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

use map_server::{
    handle_client_session, MapData, MapDataLoader, MapServiceHandle, MapServiceOptions, Registries,
};
use ro_core::{
    AccountId, CharId, CharacterInventory, CharacterSummary, CreateCharacterRequest, MapPosition,
    Sex, TARGET_PACKETVER,
};
use ro_db::{CellType, MapGrid, MapMobSpawns, MapNpcs};
use ro_packets::{
    pack_pos_dir, unpack_pos2, PacketDecoder, PacketOpcode, RathenaPacketLengthTable, RawPacket,
};
use ro_persistence::{
    CancelDeleteOutcome, CharacterRepository, CreateCharacterError, DeleteReserveOutcome,
    InventoryRepository, PlayState, RepositoryError,
};

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
    /// Simulated latency of `update_position` and `save_inventory` (a slow database).
    persist_delay: Mutex<Duration>,
    /// Stored inventories, account groups and STR by character.
    inventories: Mutex<HashMap<CharId, CharacterInventory>>,
    groups: Mutex<HashMap<CharId, i32>>,
    /// Every inventory save, in order.
    pub inventory_saves: Mutex<Vec<(CharId, CharacterInventory)>>,
}

impl InMemoryRepo {
    /// Characters `n` (char 15000n, account 200000n) named `Char<n>` at the given cells.
    pub fn seeded(chars: &[(u32, u16, u16)]) -> Self {
        let on_map: Vec<_> = chars.iter().map(|&(n, x, y)| (n, MAP, x, y)).collect();
        Self::seeded_on(&on_map)
    }

    /// Like `seeded`, with each character's stored map.
    pub fn seeded_on(chars: &[(u32, &str, u16, u16)]) -> Self {
        let repo = Self::default();
        for &(n, map, x, y) in chars {
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
                    last_map: map.to_string(),
                    last_pos: MapPosition { x, y, dir: 0 },
                    delete_date: None,
                },
            );
        }
        repo
    }

    /// Moves character `n` to another account.
    pub fn set_account(&self, n: u32, account_id: u32) {
        if let Some(c) = self.chars.lock().unwrap().get_mut(&CharId(char_id(n))) {
            c.account_id = AccountId(account_id);
        }
    }

    pub fn with_persist_delay(self, delay: Duration) -> Self {
        *self.persist_delay.lock().unwrap() = delay;
        self
    }

    /// Makes character `n`'s account a GM (group 99).
    pub fn set_gm(&self, n: u32) {
        self.groups.lock().unwrap().insert(CharId(char_id(n)), 99);
    }

    pub fn set_inventory(&self, n: u32, inventory: CharacterInventory) {
        self.inventories
            .lock()
            .unwrap()
            .insert(CharId(char_id(n)), inventory);
    }

    pub fn stored_inventory(&self, n: u32) -> Option<CharacterInventory> {
        self.inventories
            .lock()
            .unwrap()
            .get(&CharId(char_id(n)))
            .cloned()
    }

    pub fn inventory_saves_of(&self, n: u32) -> usize {
        self.inventory_saves
            .lock()
            .unwrap()
            .iter()
            .filter(|(c, _)| *c == CharId(char_id(n)))
            .count()
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

#[async_trait]
impl InventoryRepository for InMemoryRepo {
    async fn load_play_state(&self, id: CharId) -> Result<Option<PlayState>, RepositoryError> {
        if !self.chars.lock().unwrap().contains_key(&id) {
            return Ok(None);
        }
        Ok(Some(PlayState {
            str: 1,
            group_id: self.groups.lock().unwrap().get(&id).copied().unwrap_or(0),
            inventory: self
                .inventories
                .lock()
                .unwrap()
                .get(&id)
                .cloned()
                .unwrap_or_default(),
        }))
    }

    async fn save_inventory(
        &self,
        id: CharId,
        inventory: &CharacterInventory,
    ) -> Result<(), RepositoryError> {
        let delay = *self.persist_delay.lock().unwrap();
        if !delay.is_zero() {
            tokio::time::sleep(delay).await;
        }
        // Like the database: rows load back without holes.
        let compact = CharacterInventory {
            slots: inventory.items().cloned().map(Some).collect(),
            unique_item_counter: inventory.unique_item_counter,
        };
        self.inventory_saves
            .lock()
            .unwrap()
            .push((id, inventory.clone()));
        self.inventories.lock().unwrap().insert(id, compact);
        Ok(())
    }
}

/// Loader handing every map the same 200x200 grid with `walls`, no NPCs and no mobs.
pub fn grid_loader(walls: &[(u16, u16)]) -> MapDataLoader {
    data_loader(walls, MapNpcs::default(), MapMobSpawns::default())
}

/// Loader handing every map the same 200x200 grid with `walls` and the same `npcs` and `mobs`.
pub fn data_loader(walls: &[(u16, u16)], npcs: MapNpcs, mobs: MapMobSpawns) -> MapDataLoader {
    let mut grid = MapGrid::all_walkable(200, 200).expect("valid dims");
    for &(x, y) in walls {
        assert!(grid.set(x, y, CellType::Wall));
    }
    let data = MapData {
        grid: Arc::new(grid),
        npcs: Arc::new(npcs),
        mobs: Arc::new(mobs),
        registries: test_registries(),
    };
    Arc::new(move |_map: &str| data.clone())
}

/// Loader handing every map a 200x200 all-walkable grid, with the NPCs listed for it (none for
/// maps not listed).
pub fn maps_loader(maps: Vec<(&'static str, MapNpcs)>) -> MapDataLoader {
    let grid = Arc::new(MapGrid::all_walkable(200, 200).expect("valid dims"));
    let maps: HashMap<&'static str, Arc<MapNpcs>> = maps
        .into_iter()
        .map(|(map, npcs)| (map, Arc::new(npcs)))
        .collect();
    let registries = test_registries();
    Arc::new(move |map: &str| MapData {
        grid: Arc::clone(&grid),
        npcs: maps.get(map).cloned().unwrap_or_default(),
        mobs: Arc::default(),
        registries: registries.clone(),
    })
}

/// The shipped item and mob registries (read from `crates/ro-db/data`, like the server does).
pub fn test_registries() -> Registries {
    static REGISTRIES: std::sync::OnceLock<Registries> = std::sync::OnceLock::new();
    REGISTRIES
        .get_or_init(|| {
            let data =
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/ro-db/data");
            map_server::load_registries(Some(&data.join("item")), Some(&data.join("mob")))
                .expect("shipped registries load")
        })
        .clone()
}

/// A `.gat` file of `width` x `height` walkable cells.
pub fn gat_file(width: u32, height: u32) -> Vec<u8> {
    let mut b = b"GRAT".to_vec();
    b.extend_from_slice(&[1, 2]);
    b.extend_from_slice(&width.to_le_bytes());
    b.extend_from_slice(&height.to_le_bytes());
    for _ in 0..width * height {
        b.extend_from_slice(&[0u8; 20]);
    }
    b
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

/// Opcodes with a u16 length field after the opcode, among those the tests inspect.
fn is_dynamic(op: u16) -> bool {
    (0x09fd..=0x09ff).contains(&op) || matches!(op, 0x0201 | 0x010f | 0x0b8d)
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

/// (map name, x, y) of a `0x0091` frame.
pub fn mapmove(frame: &RawPacket) -> (String, u16, u16) {
    let name = &frame.payload[0..16];
    let end = name.iter().position(|&b| b == 0).unwrap_or(16);
    (
        String::from_utf8_lossy(&name[..end]).into_owned(),
        frame_u16(frame, 18),
        frame_u16(frame, 20),
    )
}

/// A unit entry packet (0x09fd/0x09fe/0x09ff).
pub fn is_entry(frame: &RawPacket) -> bool {
    (0x09fd..=0x09ff).contains(&frame.opcode.0)
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

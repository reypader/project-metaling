use std::{
    collections::HashMap,
    net::Ipv4Addr,
    sync::{
        atomic::{AtomicU32, AtomicU64, Ordering},
        Arc, RwLock,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use ro_core::{
    AccountId, CharServerInfo, HealthResponse, RegisterCharServerRequest, ServerEvent,
    ServerHeartbeat, ServerStatus, ServerType, SessionId, SessionIntent, ValidateSessionRequest,
    ValidateSessionResponse, TARGET_PACKETVER,
};
use ro_persistence::AccountRepository;
use tokio::sync::mpsc;
use tracing::{info, warn};

use super::{
    registry_dto::{ActiveSessionTicket, RegisteredCharServer},
    session_actor::LoginSessionActor,
};

pub const MAX_CHAR_SERVERS: usize = 32;

/// Shared login service managing authentication, session tickets, and char-server registration.
#[derive(Clone)]
pub struct LoginService {
    account_repo: Arc<dyn AccountRepository>,
    next_session_id: Arc<AtomicU64>,
    next_login_id: Arc<AtomicU32>,
    char_servers: Arc<RwLock<HashMap<u32, RegisteredCharServer>>>,
    active_tickets: Arc<RwLock<HashMap<AccountId, ActiveSessionTicket>>>,
    start_time: Instant,
    session_ttl: Duration,
}

pub type LoginServiceHandle = LoginService;

impl LoginService {
    pub fn new(account_repo: Arc<dyn AccountRepository>) -> Self {
        Self::with_ttl(account_repo, Duration::from_secs(300))
    }

    pub fn with_ttl(account_repo: Arc<dyn AccountRepository>, session_ttl: Duration) -> Self {
        Self::with_ttl_and_char_port(account_repo, session_ttl, 6121)
    }

    pub fn with_ttl_and_char_port(
        account_repo: Arc<dyn AccountRepository>,
        session_ttl: Duration,
        advertised_char_port: u16,
    ) -> Self {
        let default_char_server = CharServerInfo {
            ip: [127, 0, 0, 1],
            port: advertised_char_port,
            name: "Metaling-Char".to_string(),
            users: 0,
            server_type: 0,
            new_server: 0,
        };
        let mut initial_servers = HashMap::new();
        initial_servers.insert(
            1,
            RegisteredCharServer {
                server_id: 1,
                info: default_char_server,
                internal_http_url: "http://127.0.0.1:6122".to_string(),
                max_users: 1000,
                last_heartbeat_ms: 0,
            },
        );

        Self {
            account_repo,
            next_session_id: Arc::new(AtomicU64::new(1)),
            next_login_id: Arc::new(AtomicU32::new(100_001)),
            char_servers: Arc::new(RwLock::new(initial_servers)),
            active_tickets: Arc::new(RwLock::new(HashMap::new())),
            start_time: Instant::now(),
            session_ttl,
        }
    }

    pub fn allocate_session_id(&self) -> SessionId {
        SessionId(self.next_session_id.fetch_add(1, Ordering::Relaxed))
    }

    pub fn generate_login_id(&self) -> u32 {
        loop {
            let counter = self.next_login_id.fetch_add(1, Ordering::Relaxed);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(0);
            let val = counter ^ nanos;
            if val != 0 {
                return val;
            }
        }
    }

    pub async fn process_login(
        &self,
        username: &str,
        password: &str,
        client_version: u32,
        client_type: u8,
    ) -> ServerEvent {
        info!(
            "Authenticating user '{}' (version={}, client_type={})",
            username, client_version, client_type
        );
        let account = match self.account_repo.authenticate(username, password).await {
            Ok(acc) => acc,
            Err(auth_err) => {
                info!(
                    "Authentication failed for user '{}': {:?} (wire code={})",
                    username,
                    auth_err,
                    auth_err.wire_error_code()
                );
                return ServerEvent::LoginRefused {
                    error_code: auth_err.wire_error_code(),
                };
            }
        };

        let login_id1 = self.generate_login_id();
        let login_id2 = self.generate_login_id();
        let expires_at = Instant::now() + self.session_ttl;

        let ticket = ActiveSessionTicket {
            account_id: account.id,
            login_id1,
            login_id2,
            sex: account.sex,
            expires_at,
        };

        let mut tickets = match self.active_tickets.write() {
            Ok(guard) => guard,
            Err(poisoned) => {
                warn!("Active tickets write lock poisoned during process_login. Recovering inner state.");
                poisoned.into_inner()
            }
        };
        let now = Instant::now();
        tickets.retain(|_, t| t.expires_at > now);
        tickets.insert(account.id, ticket);

        let char_servers = self.char_server_infos();
        info!(
            "Authentication successful for user '{}' (account_id={}), issued ticket ({}, {}) with {} char server(s)",
            username, account.id.0, login_id1, login_id2, char_servers.len()
        );

        ServerEvent::LoginAccepted {
            account_id: account.id,
            login_id1,
            login_id2,
            sex: account.sex,
            char_servers,
        }
    }

    pub fn validate_session(&self, req: &ValidateSessionRequest) -> ValidateSessionResponse {
        let mut tickets = match self.active_tickets.write() {
            Ok(guard) => guard,
            Err(poisoned) => {
                warn!("Active tickets write lock poisoned during validate_session. Recovering inner state.");
                poisoned.into_inner()
            }
        };

        if let Some(ticket) = tickets.get_mut(&req.account_id) {
            if ticket.expires_at <= Instant::now() {
                tickets.remove(&req.account_id);
                return ValidateSessionResponse {
                    valid: false,
                    account_id: req.account_id,
                    error_message: Some("Session ticket expired".to_string()),
                };
            }

            if ticket.login_id1 != req.login_id1
                || ticket.login_id2 != req.login_id2
                || ticket.sex != req.sex
            {
                return ValidateSessionResponse {
                    valid: false,
                    account_id: req.account_id,
                    error_message: Some("Session ticket credentials mismatch".to_string()),
                };
            }

            // Refresh and extend expiration timestamp
            ticket.expires_at = Instant::now() + self.session_ttl;
            ValidateSessionResponse {
                valid: true,
                account_id: req.account_id,
                error_message: None,
            }
        } else {
            ValidateSessionResponse {
                valid: false,
                account_id: req.account_id,
                error_message: Some("Session ticket not found".to_string()),
            }
        }
    }

    pub fn register_char_server(&self, req: RegisterCharServerRequest) -> bool {
        info!(
            "LoginService registering char server id={} name='{}' at {}:{}",
            req.server_id, req.name, req.client_host, req.client_port
        );
        let ip = req
            .client_host
            .parse::<Ipv4Addr>()
            .map(|addr| addr.octets())
            .unwrap_or([127, 0, 0, 1]);

        let mut servers = match self.char_servers.write() {
            Ok(guard) => guard,
            Err(poisoned) => {
                warn!("Char servers write lock poisoned during register_char_server. Recovering inner state.");
                poisoned.into_inner()
            }
        };

        if !servers.contains_key(&req.server_id) && servers.len() >= MAX_CHAR_SERVERS {
            warn!(
                "LoginService rejected char server id={}: capacity limit of {} char servers reached.",
                req.server_id, MAX_CHAR_SERVERS
            );
            return false;
        }

        let current_users = servers
            .get(&req.server_id)
            .map(|s| s.info.users)
            .unwrap_or(0);
        let info = CharServerInfo {
            ip,
            port: req.client_port,
            name: req.name,
            users: current_users,
            server_type: 0,
            new_server: 0,
        };

        servers.insert(
            req.server_id,
            RegisteredCharServer {
                server_id: req.server_id,
                info,
                internal_http_url: req.internal_http_url,
                max_users: req.max_users,
                last_heartbeat_ms: 0,
            },
        );
        true
    }

    pub fn update_heartbeat(&self, heartbeat: &ServerHeartbeat) -> bool {
        let mut servers = match self.char_servers.write() {
            Ok(guard) => guard,
            Err(poisoned) => {
                warn!("Char servers write lock poisoned during update_heartbeat. Recovering inner state.");
                poisoned.into_inner()
            }
        };

        if let Some(server) = servers.get_mut(&heartbeat.server_id) {
            server.info.users = heartbeat.user_count.min(u16::MAX as u32) as u16;
            server.last_heartbeat_ms = heartbeat.timestamp_ms;
            true
        } else {
            warn!(
                "Heartbeat received for unregistered char server id={}",
                heartbeat.server_id
            );
            false
        }
    }

    pub fn char_server_infos(&self) -> Vec<CharServerInfo> {
        let servers = match self.char_servers.read() {
            Ok(guard) => guard,
            Err(poisoned) => {
                warn!("Char servers read lock poisoned during char_server_infos. Recovering inner state.");
                poisoned.into_inner()
            }
        };
        let mut list: Vec<_> = servers.values().collect();
        list.sort_by_key(|s| s.server_id);
        list.into_iter().map(|s| s.info.clone()).collect()
    }

    pub fn health_response(&self) -> HealthResponse {
        let now = Instant::now();
        let active_connections = match self.active_tickets.read() {
            Ok(guard) => guard.values().filter(|t| t.expires_at > now).count(),
            Err(poisoned) => {
                warn!("Active tickets read lock poisoned during health_response. Recovering inner state.");
                poisoned
                    .into_inner()
                    .values()
                    .filter(|t| t.expires_at > now)
                    .count()
            }
        };
        HealthResponse {
            status: ServerStatus::Ready,
            server_type: ServerType::Login,
            server_name: "metaling-login".to_string(),
            uptime_seconds: self.start_time.elapsed().as_secs(),
            active_connections,
            target_packetver: TARGET_PACKETVER,
        }
    }

    pub fn spawn_session_actor(
        &self,
        session_id: SessionId,
        outbound_tx: mpsc::Sender<ServerEvent>,
    ) -> mpsc::Sender<SessionIntent> {
        let (intent_tx, intent_rx) = mpsc::channel(16);
        let actor = LoginSessionActor {
            session_id,
            service: self.clone(),
            outbound_tx,
            login_processed: false,
        };
        tokio::spawn(async move {
            actor.run(intent_rx).await;
        });
        intent_tx
    }
}

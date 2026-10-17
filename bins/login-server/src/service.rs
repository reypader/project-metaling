use std::{
    collections::HashMap,
    net::Ipv4Addr,
    sync::{
        atomic::{AtomicU32, AtomicU64, Ordering},
        Arc, RwLock,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use ro_contracts::{
    account::AccountRepository,
    http::{
        HealthResponse, RegisterCharServerRequest, ServerHeartbeat, ServerStatus, ServerType,
        ValidateSessionRequest, ValidateSessionResponse,
    },
    ids::{AccountId, CharServerInfo, SessionId, Sex},
    packet::{ClientIntent, ServerEvent, SessionIntent},
    TARGET_PACKETVER,
};
use tokio::sync::mpsc;
use tracing::{info, warn};

/// Active session ticket issued upon successful authentication.
#[derive(Debug, Clone)]
pub struct ActiveSessionTicket {
    #[allow(dead_code)]
    pub account_id: AccountId,
    pub login_id1: u32,
    pub login_id2: u32,
    pub sex: Sex,
    pub expires_at: Instant,
}

/// Registered character server state tracked by login-server.
#[derive(Debug, Clone)]
pub struct RegisteredCharServer {
    pub server_id: u32,
    pub info: CharServerInfo,
    #[allow(dead_code)]
    pub internal_http_url: String,
    #[allow(dead_code)]
    pub max_users: u32,
    pub last_heartbeat_ms: u64,
}

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
        let default_char_server = CharServerInfo {
            ip: [127, 0, 0, 1],
            port: 6121,
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
            Err(poisoned) => poisoned.into_inner(),
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
            Err(poisoned) => poisoned.into_inner(),
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
            Err(poisoned) => poisoned.into_inner(),
        };

        let current_users = servers.get(&req.server_id).map(|s| s.info.users).unwrap_or(0);
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
            Err(poisoned) => poisoned.into_inner(),
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
            Err(poisoned) => poisoned.into_inner(),
        };
        let mut list: Vec<_> = servers.values().collect();
        list.sort_by_key(|s| s.server_id);
        list.into_iter().map(|s| s.info.clone()).collect()
    }

    pub fn health_response(&self) -> HealthResponse {
        let now = Instant::now();
        let active_connections = match self.active_tickets.read() {
            Ok(guard) => guard.values().filter(|t| t.expires_at > now).count(),
            Err(poisoned) => poisoned
                .into_inner()
                .values()
                .filter(|t| t.expires_at > now)
                .count(),
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
        };
        tokio::spawn(async move {
            actor.run(intent_rx).await;
        });
        intent_tx
    }
}

impl Default for LoginService {
    fn default() -> Self {
        Self::new(Arc::new(ro_persistence::InMemoryStubRepo::new()))
    }
}

/// Ephemeral logic actor processing `SessionIntent` messages sequentially for one login client.
struct LoginSessionActor {
    session_id: SessionId,
    service: LoginService,
    outbound_tx: mpsc::Sender<ServerEvent>,
}

impl LoginSessionActor {
    async fn run(mut self, mut rx: mpsc::Receiver<SessionIntent>) {
        while let Some(session_intent) = rx.recv().await {
            if !self.handle_intent(session_intent).await {
                break;
            }
        }
    }

    async fn handle_intent(&mut self, session_intent: SessionIntent) -> bool {
        match session_intent.intent {
            ClientIntent::Login {
                ref username,
                ref password,
                client_version,
                client_type,
            } => {
                info!(
                    "Session {:?} processing login for user '{}' (version={}, client_type={})",
                    self.session_id, username, client_version, client_type
                );
                let event = self
                    .service
                    .process_login(username, password, client_version, client_type)
                    .await;

                if self.outbound_tx.try_send(event).is_err() {
                    warn!(
                        "Session {:?} outbound queue full or closed; dropping login session",
                        self.session_id
                    );
                }
                false
            }
            unhandled => {
                warn!(
                    "Session {:?} received unhandled login intent: {:?}",
                    self.session_id, unhandled
                );
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ro_persistence::InMemoryStubRepo;

    #[tokio::test]
    async fn test_process_login_success() {
        let repo = Arc::new(InMemoryStubRepo::new());
        let service = LoginService::new(repo);

        let event = service
            .process_login("test", "password", 0x80000000, 0)
            .await;

        match event {
            ServerEvent::LoginAccepted {
                account_id,
                login_id1,
                login_id2,
                sex,
                char_servers,
            } => {
                assert_eq!(account_id, AccountId(2000000));
                assert_eq!(sex, Sex::Male);
                assert_ne!(login_id1, 0);
                assert_ne!(login_id2, 0);
                assert_eq!(char_servers.len(), 1);
                assert_eq!(char_servers[0].port, 6121);
            }
            other => panic!("Expected LoginAccepted, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_process_login_bad_password() {
        let repo = Arc::new(InMemoryStubRepo::new());
        let service = LoginService::new(repo);

        let event = service
            .process_login("test", "passwords", 0x80000000, 0)
            .await;

        assert_eq!(event, ServerEvent::LoginRefused { error_code: 1 });
    }

    #[tokio::test]
    async fn test_process_login_unknown_user() {
        let repo = Arc::new(InMemoryStubRepo::new());
        let service = LoginService::new(repo);

        let event = service
            .process_login("unknown", "password", 0x80000000, 0)
            .await;

        assert_eq!(event, ServerEvent::LoginRefused { error_code: 0 });
    }

    #[tokio::test]
    async fn test_session_ticket_lifecycle_and_reauth() {
        let repo = Arc::new(InMemoryStubRepo::new());
        let service = LoginService::new(repo);

        let event = service
            .process_login("test", "password", 0x80000000, 0)
            .await;

        let (account_id, login_id1, login_id2, sex) = match event {
            ServerEvent::LoginAccepted {
                account_id,
                login_id1,
                login_id2,
                sex,
                ..
            } => (account_id, login_id1, login_id2, sex),
            _ => panic!("Expected LoginAccepted"),
        };

        // Initial validation (CH_ENTER)
        let validate_req = ValidateSessionRequest {
            account_id,
            login_id1,
            login_id2,
            sex,
        };
        let res1 = service.validate_session(&validate_req);
        assert!(res1.valid);
        assert_eq!(res1.account_id, account_id);
        assert!(res1.error_message.is_none());

        // Second validation with same ticket (Return to Character Selection)
        let res2 = service.validate_session(&validate_req);
        assert!(res2.valid);
        assert_eq!(res2.account_id, account_id);

        // Validation with incorrect credentials
        let bad_req = ValidateSessionRequest {
            account_id,
            login_id1: login_id1.wrapping_add(1),
            login_id2,
            sex,
        };
        let res3 = service.validate_session(&bad_req);
        assert!(!res3.valid);

        // Validation with incorrect sex
        let bad_sex_req = ValidateSessionRequest {
            account_id,
            login_id1,
            login_id2,
            sex: Sex::Female,
        };
        let res4 = service.validate_session(&bad_sex_req);
        assert!(!res4.valid);
    }

    #[tokio::test]
    async fn test_session_ticket_expiration() {
        let repo = Arc::new(InMemoryStubRepo::new());
        // Configure with 1ms TTL
        let service = LoginService::with_ttl(repo, Duration::from_millis(1));

        let event = service
            .process_login("test", "password", 0x80000000, 0)
            .await;

        let (account_id, login_id1, login_id2, sex) = match event {
            ServerEvent::LoginAccepted {
                account_id,
                login_id1,
                login_id2,
                sex,
                ..
            } => (account_id, login_id1, login_id2, sex),
            _ => panic!("Expected LoginAccepted"),
        };

        tokio::time::sleep(Duration::from_millis(10)).await;

        let validate_req = ValidateSessionRequest {
            account_id,
            login_id1,
            login_id2,
            sex,
        };
        let res = service.validate_session(&validate_req);
        assert!(!res.valid);
        assert_eq!(res.error_message.as_deref(), Some("Session ticket expired"));
    }

    #[tokio::test]
    async fn test_char_server_registration_and_heartbeat() {
        let repo = Arc::new(InMemoryStubRepo::new());
        let service = LoginService::new(repo);

        let reg_req = RegisterCharServerRequest {
            server_id: 1,
            name: "Updated-Char".to_string(),
            client_host: "127.0.0.1".to_string(),
            client_port: 6125,
            internal_http_url: "http://127.0.0.1:6126".to_string(),
            max_users: 500,
        };
        assert!(service.register_char_server(reg_req));

        let hb = ServerHeartbeat {
            server_id: 1,
            user_count: 99,
            timestamp_ms: 12345,
        };
        assert!(service.update_heartbeat(&hb));

        let infos = service.char_server_infos();
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].name, "Updated-Char");
        assert_eq!(infos[0].port, 6125);
        assert_eq!(infos[0].users, 99);
    }
}

#![allow(clippy::unwrap_used, clippy::expect_used)] // test helpers aren't covered by clippy.toml allow-*-in-tests

use std::{
    net::SocketAddr,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use clap::Parser;
use ro_contracts::http::ServerType;
use ro_proxy::{
    forwarder::run_proxy_server,
    logger::PacketDirection,
    PacketSequenceRecorder, ProxyArgs,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

static ENV_MUTEX: Mutex<()> = Mutex::new(());

/// RAII guard terminating spawned child processes on drop to prevent process leaks.
struct ChildGuard(Option<Child>);

impl ChildGuard {
    fn new(child: Child) -> Self {
        Self(Some(child))
    }

    fn id(&self) -> u32 {
        self.0.as_ref().expect("Child process unavailable").id()
    }

    fn try_wait(&mut self) -> std::io::Result<Option<std::process::ExitStatus>> {
        self.0.as_mut().expect("Child process unavailable").try_wait()
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Helper to find a free local TCP port.
fn get_free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("Failed to bind ephemeral port");
    listener.local_addr().expect("Failed to get local addr").port()
}

/// Helper to wait until a TCP port accepts connections.
async fn wait_for_port(port: u16, timeout: Duration) -> bool {
    let start = Instant::now();
    let addr: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
    while start.elapsed() < timeout {
        if TcpStream::connect(addr).await.is_ok() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    false
}

// ============================================================================
// DOM-PROXY-01: Environment Variable Target Overrides
// ============================================================================

#[test]
fn test_dom_proxy_01_env_var_target_overrides() {
    let _lock = ENV_MUTEX.lock().unwrap();

    // Set environment variables for proxy targets.
    std::env::set_var("PROXY_LOGIN_TARGET", "10.0.0.1:6910");
    std::env::set_var("PROXY_CHAR_TARGET", "10.0.0.2:6131");
    std::env::set_var("PROXY_MAP_TARGET", "10.0.0.3:5131");

    let args = ProxyArgs::try_parse_from(["proxy"]).expect("Failed to parse default arguments");
    let config = args.build_config().expect("Failed to build config");

    // Clean up environment variables.
    std::env::remove_var("PROXY_LOGIN_TARGET");
    std::env::remove_var("PROXY_CHAR_TARGET");
    std::env::remove_var("PROXY_MAP_TARGET");

    assert_eq!(config.login_target, "10.0.0.1:6910");
    assert_eq!(config.char_target, "10.0.0.2:6131");
    assert_eq!(config.map_target, "10.0.0.3:5131");
}

#[test]
fn test_dom_proxy_01_cli_flags_precedence_over_env_vars() {
    let _lock = ENV_MUTEX.lock().unwrap();

    // Set environment variables.
    std::env::set_var("PROXY_LOGIN_TARGET", "10.0.0.1:6910");
    std::env::set_var("PROXY_CHAR_TARGET", "10.0.0.2:6131");
    std::env::set_var("PROXY_MAP_TARGET", "10.0.0.3:5131");

    // Pass CLI flags overriding login and map, leaving char to environment.
    let args = ProxyArgs::try_parse_from([
        "proxy",
        "--login",
        "192.168.1.50:6999",
        "--map",
        "192.168.1.50:5999",
    ])
    .expect("Failed to parse arguments with CLI flags");
    let config = args.build_config().expect("Failed to build config");

    // Clean up environment variables.
    std::env::remove_var("PROXY_LOGIN_TARGET");
    std::env::remove_var("PROXY_CHAR_TARGET");
    std::env::remove_var("PROXY_MAP_TARGET");

    assert_eq!(config.login_target, "192.168.1.50:6999");
    assert_eq!(config.char_target, "10.0.0.2:6131");
    assert_eq!(config.map_target, "192.168.1.50:5999");
}

// ============================================================================
// DOM-PROXY-02: Clean Shutdown and Sequence Log Flush on Signals
// ============================================================================

#[cfg(unix)]
#[tokio::test]
async fn test_dom_proxy_02_sigterm_clean_shutdown_and_log_flush() {
    let login_port = get_free_port();
    let char_port = get_free_port();
    let map_port = get_free_port();

    let temp_log: PathBuf = std::env::temp_dir().join(format!("proxy_test_sigterm_{}.log", login_port));
    if temp_log.exists() {
        let _ = std::fs::remove_file(&temp_log);
    }

    let bin_path = env!("CARGO_BIN_EXE_proxy");
    let child = Command::new(bin_path)
        .arg("--bind")
        .arg("127.0.0.1")
        .arg("--listen-login-port")
        .arg(login_port.to_string())
        .arg("--listen-char-port")
        .arg(char_port.to_string())
        .arg("--listen-map-port")
        .arg(map_port.to_string())
        .arg("--log-file")
        .arg(temp_log.to_str().unwrap())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn proxy binary");

    let mut guard = ChildGuard::new(child);

    // Wait until login port is accepting connections.
    let ready = wait_for_port(login_port, Duration::from_secs(3)).await;
    assert!(ready, "Proxy binary did not start listening in time");

    // Send SIGTERM to the running proxy process.
    let pid = guard.id();
    let kill_status = Command::new("kill")
        .arg("-TERM")
        .arg(pid.to_string())
        .status()
        .expect("Failed to execute kill command");
    assert!(kill_status.success(), "Failed to send SIGTERM to proxy process");

    // Wait for the child process to terminate.
    let exit_status = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Ok(Some(status)) = guard.try_wait() {
                return status;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("Proxy did not terminate within timeout upon SIGTERM");

    assert!(
        exit_status.success(),
        "Proxy process failed to exit cleanly on SIGTERM, status: {:?}",
        exit_status
    );

    // Verify the log file was created and contains the header.
    assert!(temp_log.exists(), "Sequence log file was not created on SIGTERM");
    let content = std::fs::read_to_string(&temp_log).expect("Failed to read log file");
    assert!(
        content.contains("Ragnarok Online Packet Proxy - Global Sequence Log"),
        "Log file does not contain expected header"
    );

    let _ = std::fs::remove_file(&temp_log);
}

#[tokio::test]
async fn test_dom_proxy_02_sigint_clean_shutdown_and_log_flush() {
    let login_port = get_free_port();
    let char_port = get_free_port();
    let map_port = get_free_port();

    let temp_log: PathBuf = std::env::temp_dir().join(format!("proxy_test_sigint_{}.log", login_port));
    if temp_log.exists() {
        let _ = std::fs::remove_file(&temp_log);
    }

    let bin_path = env!("CARGO_BIN_EXE_proxy");
    let child = Command::new(bin_path)
        .arg("--bind")
        .arg("127.0.0.1")
        .arg("--listen-login-port")
        .arg(login_port.to_string())
        .arg("--listen-char-port")
        .arg(char_port.to_string())
        .arg("--listen-map-port")
        .arg(map_port.to_string())
        .arg("--log-file")
        .arg(temp_log.to_str().unwrap())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn proxy binary");

    let mut guard = ChildGuard::new(child);

    let ready = wait_for_port(login_port, Duration::from_secs(3)).await;
    assert!(ready, "Proxy binary did not start listening in time");

    // Send SIGINT to the running proxy process.
    let pid = guard.id();
    let kill_status = Command::new("kill")
        .arg("-INT")
        .arg(pid.to_string())
        .status()
        .expect("Failed to execute kill command");
    assert!(kill_status.success(), "Failed to send SIGINT to proxy process");

    let exit_status = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Ok(Some(status)) = guard.try_wait() {
                return status;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("Proxy did not terminate within timeout upon SIGINT");

    assert!(
        exit_status.success(),
        "Proxy process failed to exit cleanly on SIGINT, status: {:?}",
        exit_status
    );

    assert!(temp_log.exists(), "Sequence log file was not created on SIGINT");
    let content = std::fs::read_to_string(&temp_log).expect("Failed to read log file");
    assert!(
        content.contains("Ragnarok Online Packet Proxy - Global Sequence Log"),
        "Log file does not contain expected header"
    );

    let _ = std::fs::remove_file(&temp_log);
}

// ============================================================================
// DOM-PROXY-03: Clean Non-Zero Exit on EADDRINUSE
// ============================================================================

#[tokio::test]
async fn test_dom_proxy_03_eaddrinuse_clean_exit() {
    // Occupy a port with an existing listener.
    let occupied_listener = std::net::TcpListener::bind("127.0.0.1:0").expect("Failed to bind occupying listener");
    let occupied_port = occupied_listener.local_addr().expect("Failed to get port").port();

    let other_port1 = get_free_port();
    let other_port2 = get_free_port();

    let bin_path = env!("CARGO_BIN_EXE_proxy");
    let output = Command::new(bin_path)
        .arg("--bind")
        .arg("127.0.0.1")
        .arg("--listen-login-port")
        .arg(occupied_port.to_string())
        .arg("--listen-char-port")
        .arg(other_port1.to_string())
        .arg("--listen-map-port")
        .arg(other_port2.to_string())
        .output()
        .expect("Failed to execute proxy command");

    // The process must exit with a non-zero exit code.
    assert!(
        !output.status.success(),
        "Proxy must exit with non-zero exit code when port is in use"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined_logs = format!("{}\n{}", stdout, stderr);

    assert!(
        combined_logs.to_lowercase().contains("address already in use")
            || combined_logs.to_lowercase().contains("failed to bind"),
        "Output must report the port collision. Output: {}",
        combined_logs
    );
}

// ============================================================================
// DOM-PROXY-04: Concurrent Multi-Service Forwarding to Remapped Ports
// ============================================================================

#[tokio::test]
async fn test_dom_proxy_04_concurrent_multiservice_forwarding() {
    // 1. Start mock servers for Login, Char, Map on remapped ports.
    let mock_login_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let login_target_addr = mock_login_listener.local_addr().unwrap();

    let mock_char_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let char_target_addr = mock_char_listener.local_addr().unwrap();

    let mock_map_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let map_target_addr = mock_map_listener.local_addr().unwrap();

    let mock_login_task = tokio::spawn(async move {
        let (mut socket, _) = mock_login_listener.accept().await.unwrap();
        let mut buf = [0u8; 55];
        socket.read_exact(&mut buf).await.unwrap();
        assert_eq!(buf[0], 0x64);
        assert_eq!(buf[1], 0x00);
        // Reply with mock AC_ACCEPT_LOGIN
        socket.write_all(&[0x69, 0x00, 0x04, 0x00]).await.unwrap();
        socket.flush().await.unwrap();
    });

    let mock_char_task = tokio::spawn(async move {
        let (mut socket, _) = mock_char_listener.accept().await.unwrap();
        // Expect CH_ENTER (0x0065, fixed 17 bytes)
        let mut buf = [0u8; 17];
        socket.read_exact(&mut buf).await.unwrap();
        assert_eq!(buf[0], 0x65);
        assert_eq!(buf[1], 0x00);
        // Reply with 4-byte AccountId handshake echo followed by HC_ACK_CHARINFO_PER_PAGE
        socket.write_all(&[0x01, 0x00, 0x00, 0x00, 0x6b, 0x00, 0x04, 0x00]).await.unwrap();
        socket.flush().await.unwrap();
    });

    let mock_map_task = tokio::spawn(async move {
        let (mut socket, _) = mock_map_listener.accept().await.unwrap();
        // Expect CZ_ENTER (0x0436, fixed 23 bytes)
        let mut buf = [0u8; 23];
        socket.read_exact(&mut buf).await.unwrap();
        assert_eq!(buf[0], 0x36);
        assert_eq!(buf[1], 0x04);
        // Reply with 4-byte AccountId handshake echo followed by ZC_AUTH_ACK
        socket.write_all(&[0x01, 0x00, 0x00, 0x00, 0x73, 0x00, 0x04, 0x00]).await.unwrap();
        socket.flush().await.unwrap();
    });

    // 2. Start proxy listeners forwarding to respective mock targets.
    let proxy_login_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_login_addr = proxy_login_listener.local_addr().unwrap();

    let proxy_char_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_char_addr = proxy_char_listener.local_addr().unwrap();

    let proxy_map_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_map_addr = proxy_map_listener.local_addr().unwrap();

    let recorder = Arc::new(PacketSequenceRecorder::new());

    let rec1 = recorder.clone();
    let login_target_str = login_target_addr.to_string();
    let proxy_login_task = tokio::spawn(async move {
        let _ = run_proxy_server(proxy_login_listener, login_target_str, ServerType::Login, rec1).await;
    });

    let rec2 = recorder.clone();
    let char_target_str = char_target_addr.to_string();
    let proxy_char_task = tokio::spawn(async move {
        let _ = run_proxy_server(proxy_char_listener, char_target_str, ServerType::Char, rec2).await;
    });

    let rec3 = recorder.clone();
    let map_target_str = map_target_addr.to_string();
    let proxy_map_task = tokio::spawn(async move {
        let _ = run_proxy_server(proxy_map_listener, map_target_str, ServerType::Map, rec3).await;
    });

    // 3. Connect 3 clients concurrently to the remapped proxy ports.
    let client_login = tokio::spawn(async move {
        let mut stream = TcpStream::connect(proxy_login_addr).await.unwrap();
        let mut packet = vec![0x64, 0x00];
        packet.extend_from_slice(&[0u8; 53]);
        stream.write_all(&packet).await.unwrap();
        stream.flush().await.unwrap();

        let mut reply = [0u8; 4];
        stream.read_exact(&mut reply).await.unwrap();
        assert_eq!(reply[0], 0x69);
        assert_eq!(reply[1], 0x00);
    });

    let client_char = tokio::spawn(async move {
        let mut stream = TcpStream::connect(proxy_char_addr).await.unwrap();
        let mut packet = vec![0x65, 0x00];
        packet.extend_from_slice(&[0u8; 15]);
        stream.write_all(&packet).await.unwrap();
        stream.flush().await.unwrap();

        let mut reply = [0u8; 8];
        stream.read_exact(&mut reply).await.unwrap();
        assert_eq!(&reply[..4], &[0x01, 0x00, 0x00, 0x00]);
        assert_eq!(reply[4], 0x6b);
        assert_eq!(reply[5], 0x00);
    });

    let client_map = tokio::spawn(async move {
        let mut stream = TcpStream::connect(proxy_map_addr).await.unwrap();
        let mut packet = vec![0x36, 0x04];
        packet.extend_from_slice(&[0u8; 21]);
        stream.write_all(&packet).await.unwrap();
        stream.flush().await.unwrap();

        let mut reply = [0u8; 8];
        stream.read_exact(&mut reply).await.unwrap();
        assert_eq!(&reply[..4], &[0x01, 0x00, 0x00, 0x00]);
        assert_eq!(reply[4], 0x73);
        assert_eq!(reply[5], 0x00);
    });

    // 4. Await completion of all client exchanges and mock servers.
    tokio::try_join!(client_login, client_char, client_map).unwrap();
    tokio::try_join!(mock_login_task, mock_char_task, mock_map_task).unwrap();

    // Small delay to allow proxy tracking tasks to complete logging.
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Verify packets were recorded for all 3 server types.
    let entries = recorder.entries();
    let has_login = entries.iter().any(|e| e.server_type == ServerType::Login);
    let has_char = entries.iter().any(|e| e.server_type == ServerType::Char);
    let has_map = entries.iter().any(|e| e.server_type == ServerType::Map);

    assert!(has_login, "Recorder must contain Login server packets");
    assert!(has_char, "Recorder must contain Char server packets");
    assert!(has_map, "Recorder must contain Map server packets");

    let has_client_to_server = entries.iter().any(|e| e.direction == PacketDirection::ClientToServer);
    let has_server_to_client = entries.iter().any(|e| e.direction == PacketDirection::ServerToClient);
    assert!(has_client_to_server, "Recorder must contain client to server packets");
    assert!(has_server_to_client, "Recorder must contain server to client packets");

    proxy_login_task.abort();
    proxy_char_task.abort();
    proxy_map_task.abort();
}

use std::{sync::Arc, time::Duration};
use ro_contracts::http::ServerType;
use ro_proxy::{forwarder::handle_proxy_connection, PacketSequenceRecorder};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

#[tokio::test]
async fn test_proxy_end_to_end_packet_passthrough() {
    // 1. Mock Server
    let mock_server_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_addr = mock_server_listener.local_addr().unwrap();

    let server_task = tokio::spawn(async move {
        let (mut socket, _) = mock_server_listener.accept().await.unwrap();

        // Expect CA_LOGIN (55 bytes)
        let mut buf = [0u8; 55];
        socket.read_exact(&mut buf).await.unwrap();
        assert_eq!(buf[0], 0x64);
        assert_eq!(buf[1], 0x00);

        // Reply with mock AC_ACCEPT_LOGIN header + dummy payload
        let mut reply = vec![0x69, 0x00, 10, 0]; // 10 bytes dynamic packet
        reply.extend_from_slice(&[1, 2, 3, 4, 5, 6]);
        socket.write_all(&reply).await.unwrap();
        socket.flush().await.unwrap();
    });

    // 2. Proxy Listener
    let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = proxy_listener.local_addr().unwrap();

    let recorder = Arc::new(PacketSequenceRecorder::new());
    let target_str = server_addr.to_string();
    let rec = recorder.clone();
    let proxy_task = tokio::spawn(async move {
        let (client_socket, client_addr) = proxy_listener.accept().await.unwrap();
        handle_proxy_connection(client_socket, client_addr, target_str, ServerType::Login, 1, rec).await;
    });

    // 3. Mock Client connects to Proxy
    let mut client_socket = TcpStream::connect(proxy_addr).await.unwrap();

    // Send CA_LOGIN packet
    let mut ca_login = vec![0x64, 0x00];
    ca_login.extend_from_slice(&20211103u32.to_le_bytes());
    let mut username = [0u8; 24];
    username[..4].copy_from_slice(b"user");
    ca_login.extend_from_slice(&username);
    let mut password = [0u8; 24];
    password[..4].copy_from_slice(b"pass");
    ca_login.extend_from_slice(&password);
    ca_login.push(1);
    assert_eq!(ca_login.len(), 55);

    client_socket.write_all(&ca_login).await.unwrap();
    client_socket.flush().await.unwrap();

    // Read response from server via proxy
    let mut reply_buf = [0u8; 10];
    client_socket.read_exact(&mut reply_buf).await.unwrap();
    assert_eq!(reply_buf[0], 0x69);
    assert_eq!(reply_buf[1], 0x00);
    assert_eq!(&reply_buf[4..], &[1, 2, 3, 4, 5, 6]);

    server_task.await.unwrap();
    // Allow proxy tasks to complete cleanly
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(recorder.len() >= 2);
    proxy_task.abort();
}

#[tokio::test]
async fn test_proxy_unparseable_raw_passthrough_and_resync() {
    // 1. Mock Server
    let mock_server_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_addr = mock_server_listener.local_addr().unwrap();

    let server_task = tokio::spawn(async move {
        let (mut socket, _) = mock_server_listener.accept().await.unwrap();

        // Expect 7 arbitrary unparseable bytes
        let mut raw_buf = [0u8; 7];
        socket.read_exact(&mut raw_buf).await.unwrap();
        assert_eq!(&raw_buf, &[0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x11]);

        // Expect CZ_NOTIFY_ACTORINIT (2 bytes: 0x7d, 0x00)
        let mut init_buf = [0u8; 2];
        socket.read_exact(&mut init_buf).await.unwrap();
        assert_eq!(&init_buf, &[0x7d, 0x00]);

        // Reply with arbitrary bytes back to client
        socket.write_all(&[0x12, 0x34, 0x56, 0x78]).await.unwrap();
        socket.flush().await.unwrap();
    });

    // 2. Proxy Listener
    let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = proxy_listener.local_addr().unwrap();

    let recorder = Arc::new(PacketSequenceRecorder::new());
    let target_str = server_addr.to_string();
    let rec = recorder.clone();
    let proxy_task = tokio::spawn(async move {
        let (client_socket, client_addr) = proxy_listener.accept().await.unwrap();
        handle_proxy_connection(client_socket, client_addr, target_str, ServerType::Map, 2, rec).await;
    });

    // 3. Mock Client connects to Proxy
    let mut client_socket = TcpStream::connect(proxy_addr).await.unwrap();

    // Send arbitrary bytes
    client_socket
        .write_all(&[0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x11])
        .await
        .unwrap();
    client_socket.flush().await.unwrap();

    // Send valid packet immediately after
    client_socket.write_all(&[0x7d, 0x00]).await.unwrap();
    client_socket.flush().await.unwrap();

    // Read arbitrary server bytes back
    let mut reply_buf = [0u8; 4];
    client_socket.read_exact(&mut reply_buf).await.unwrap();
    assert_eq!(&reply_buf, &[0x12, 0x34, 0x56, 0x78]);

    server_task.await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(recorder.len() >= 2);
    proxy_task.abort();
}

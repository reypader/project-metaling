use super::*;
use crate::args::ProxyArgs;

#[test]
fn test_resolve_target_defaults() {
    let res = resolve_target(None, "127.0.0.1", 6900).unwrap();
    assert_eq!(res, "127.0.0.1:6900");

    let res = resolve_target(None, "192.168.1.100", 6121).unwrap();
    assert_eq!(res, "192.168.1.100:6121");
}

#[test]
fn test_resolve_target_host_and_port() {
    let res = resolve_target(Some("10.0.0.1:7000"), "127.0.0.1", 6900).unwrap();
    assert_eq!(res, "10.0.0.1:7000");

    let res = resolve_target(Some("rathena.local:6121"), "127.0.0.1", 6121).unwrap();
    assert_eq!(res, "rathena.local:6121");
}

#[test]
fn test_resolve_target_port_only() {
    let res = resolve_target(Some(":7000"), "192.168.1.50", 6900).unwrap();
    assert_eq!(res, "192.168.1.50:7000");

    let res = resolve_target(Some("7000"), "192.168.1.50", 6900).unwrap();
    assert_eq!(res, "192.168.1.50:7000");
}

#[test]
fn test_resolve_target_host_only() {
    let res = resolve_target(Some("remote-server"), "127.0.0.1", 5121).unwrap();
    assert_eq!(res, "remote-server:5121");
}

#[test]
fn test_resolve_target_invalid_port() {
    let res = resolve_target(Some("host:abc"), "127.0.0.1", 5121);
    assert!(res.is_err());
}

#[test]
fn test_build_config_from_args() {
    let args = ProxyArgs {
        target_ip: "192.168.1.200".to_string(),
        login: None,
        char_server: Some("10.0.0.2:6125".to_string()),
        map_server: Some("10.0.0.3:5125".to_string()),
        bind_addr: "0.0.0.0".to_string(),
        listen_login_port: 6900,
        listen_char_port: 6121,
        listen_map_port: 5121,
        log_file: None,
        full_payload: false,
    };

    let config = args.build_config().unwrap();
    assert_eq!(config.login_target, "192.168.1.200:6900");
    assert_eq!(config.char_target, "10.0.0.2:6125");
    assert_eq!(config.map_target, "10.0.0.3:5125");
    assert_eq!(config.log_file, None);
}

#[test]
fn test_build_config_with_custom_log_file() {
    let args = ProxyArgs {
        target_ip: "127.0.0.1".to_string(),
        login: None,
        char_server: None,
        map_server: None,
        bind_addr: "127.0.0.1".to_string(),
        listen_login_port: 6900,
        listen_char_port: 6121,
        listen_map_port: 5121,
        log_file: Some("/tmp/custom_proxy.log".to_string()),
        full_payload: false,
    };

    let config = args.build_config().unwrap();
    assert_eq!(config.log_file.as_deref(), Some("/tmp/custom_proxy.log"));
}

#[test]
fn test_build_config_with_empty_log_file() {
    let args = ProxyArgs {
        target_ip: "127.0.0.1".to_string(),
        login: None,
        char_server: None,
        map_server: None,
        bind_addr: "127.0.0.1".to_string(),
        listen_login_port: 6900,
        listen_char_port: 6121,
        listen_map_port: 5121,
        log_file: Some("   ".to_string()),
        full_payload: false,
    };

    let config = args.build_config().unwrap();
    assert_eq!(config.log_file, None);
}

#[test]
fn test_format_host_port_ipv4_and_ipv6() {
    assert_eq!(format_host_port("127.0.0.1", 6900), "127.0.0.1:6900");
    assert_eq!(format_host_port("localhost", 6900), "localhost:6900");
    assert_eq!(format_host_port("::1", 6900), "[::1]:6900");
    assert_eq!(format_host_port("[::1]", 6900), "[::1]:6900");
    assert_eq!(format_host_port("2001:db8::1", 5121), "[2001:db8::1]:5121");
    assert_eq!(format_host_port("", 6900), "127.0.0.1:6900");
}

#[test]
fn test_resolve_target_ipv6_bracketed_with_port() {
    let res = resolve_target(Some("[::1]:7000"), "127.0.0.1", 6900).unwrap();
    assert_eq!(res, "[::1]:7000");

    let res = resolve_target(Some("[2001:db8::1]:8000"), "127.0.0.1", 6900).unwrap();
    assert_eq!(res, "[2001:db8::1]:8000");
}

#[test]
fn test_resolve_target_ipv6_bracketed_without_port() {
    let res = resolve_target(Some("[::1]"), "127.0.0.1", 6900).unwrap();
    assert_eq!(res, "[::1]:6900");

    let res = resolve_target(Some("[2001:db8::1]"), "127.0.0.1", 5121).unwrap();
    assert_eq!(res, "[2001:db8::1]:5121");
}

#[test]
fn test_resolve_target_ipv6_unbracketed_without_port() {
    let res = resolve_target(Some("::1"), "127.0.0.1", 6900).unwrap();
    assert_eq!(res, "[::1]:6900");

    let res = resolve_target(Some("2001:db8::1"), "127.0.0.1", 5121).unwrap();
    assert_eq!(res, "[2001:db8::1]:5121");
}

#[test]
fn test_resolve_target_ipv6_default_ip() {
    let res = resolve_target(None, "::1", 6900).unwrap();
    assert_eq!(res, "[::1]:6900");

    let res = resolve_target(Some("7000"), "::1", 6900).unwrap();
    assert_eq!(res, "[::1]:7000");

    let res = resolve_target(Some(":7000"), "::1", 6900).unwrap();
    assert_eq!(res, "[::1]:7000");
}

#[test]
fn test_resolve_target_ipv6_invalid_formats() {
    let res = resolve_target(Some("[::1]:abc"), "127.0.0.1", 6900);
    assert!(res.is_err());

    let res = resolve_target(Some("[::1"), "127.0.0.1", 6900);
    assert!(res.is_err());

    let res = resolve_target(Some("[::1]extra"), "127.0.0.1", 6900);
    assert!(res.is_err());

    let res = resolve_target(Some("[]"), "127.0.0.1", 6900);
    assert!(res.is_err());

    let res = resolve_target(Some("[]:6900"), "127.0.0.1", 6900);
    assert!(res.is_err());
}

#[test]
fn test_build_config_with_ipv6_target() {
    let args = ProxyArgs {
        target_ip: "::1".to_string(),
        login: None,
        char_server: Some("[::1]:6125".to_string()),
        map_server: Some("::1".to_string()),
        bind_addr: "[::]".to_string(),
        listen_login_port: 6900,
        listen_char_port: 6121,
        listen_map_port: 5121,
        log_file: None,
        full_payload: false,
    };

    let config = args.build_config().unwrap();
    assert_eq!(config.login_target, "[::1]:6900");
    assert_eq!(config.char_target, "[::1]:6125");
    assert_eq!(config.map_target, "[::1]:5121");
}

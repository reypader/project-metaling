# Project Metaling: Ragnarok Online Packet Proxy (`ro-proxy`)

`ro-proxy` is a high-performance, transparent developer proxy and packet inspector designed to sit between a Ragnarok Online client and a server (either **Project Metaling** or an **rAthena** server).

It logs bidirectional wire protocol traffic, decodes recognized intents and events, and tracks the exact conversational sequence of packets exchanged during gameplay.

---

## 1. Core Architecture & Operating Principles

1. **Immediate Wire Pass-Through**:
   All incoming bytes from either client or server are forwarded immediately across the TCP sockets before any framing or parsing occurs. If an unknown opcode, malformed packet, or raw unpacketized byte sequence (such as the 4-byte `AccountId` handshake echo) arrives, network traffic is **never dropped, delayed, or corrupted**.

2. **Sequential Session Tracking**:
   Each client connection maintains a single monotonic sequence counter (`Seq #1`, `Seq #2`, `Seq #3`...) that tracks packets in both directions (`CLIENT -> SERVER` and `SERVER -> CLIENT`). When a session closes, the proxy outputs a chronological packet trace for the entire connection.

3. **Domain Codec Integration**:
   The proxy frames packets using `RathenaPacketLengthTable` from `ro-packets` and evaluates them against `TARGET_PACKETVER` (`20260219`). High-level intents and frames are decoded via `ro_packets::incoming::decode_frame` and `Phase0IntentParser`.

4. **Multi-Packet Chain Validation on Unmapped Packets**:
   If a packet opcode is not yet registered in `table.rs`, the proxy logs the raw opcode hex and employs chain validation (predicting follow-up packet boundaries) to avoid misinterpreting payload data as "phantom opcodes".

---

## 2. Default Ports & Routing

By default, the proxy assumes the target server is running elsewhere on the standard Ragnarok Online ports:

| Service | Default Listen Port (Proxy) | Default Target Port | Flag Override |
| :--- | :--- | :--- | :--- |
| **Login Server** | `0.0.0.0:6900` | `<target-ip>:6900` | `-l, --login <host:port>` |
| **Char Server** | `0.0.0.0:6121` | `<target-ip>:6121` | `-c, --char <host:port>` |
| **Map Server** | `0.0.0.0:5121` | `<target-ip>:5121` | `-m, --map <host:port>` |

---

## 3. CLI Arguments Reference

```text
Usage: proxy [OPTIONS]

Options:
  -t, --target-ip <TARGET_IP>
          Default target IP/host where servers are running [default: 127.0.0.1]
          (aliases: -i, --ip)
  -l, --login <LOGIN>
          Login server target in "host:port" format (defaults to <target-ip>:6900)
  -c, --char <CHAR_SERVER>
          Char server target in "host:port" format (defaults to <target-ip>:6121)
  -m, --map <MAP_SERVER>
          Map server target in "host:port" format (defaults to <target-ip>:5121)
      --bind <BIND_ADDR>
          Local IP address to bind proxy listeners [default: 0.0.0.0]
      --listen-login-port <PORT>
          Override proxy login listen port [default: 6900]
      --listen-char-port <PORT>
          Override proxy char listen port [default: 6121]
      --listen-map-port <PORT>
          Override proxy map listen port [default: 5121]
  -h, --help
          Print help
  -V, --version
          Print version
```

---

## 4. Common Usage Scenarios

### Scenario A: Spying on a Remote rAthena Server
Point your game client (`clientinfo.xml`) to the machine running the proxy (`127.0.0.1`), and point the proxy to the remote rAthena server IP:

```bash
cargo run -p ro-proxy -- -t 192.168.1.100
```
*All three services (Login, Char, Map) forward to `192.168.1.100:6900`, `:6121`, and `:5121`.*

### Scenario B: Testing Against Local Servers Running on Offset Ports
If your target servers are running on the same machine on different ports:

```bash
cargo run -p ro-proxy -- -l 127.0.0.1:6905 -c 127.0.0.1:6125 -m 127.0.0.1:5125
```
*The proxy listens on the default ports (`6900`, `6121`, `5121`) so standard clients connect without modification, and forwards to the custom server ports.*

### Scenario C: Custom Target Combination
You can combine `--target-ip` with individual `-l`, `-c`, or `-m` overrides:

```bash
cargo run -p ro-proxy -- -t 192.168.1.50 -m 192.168.1.60:5122
```
*Login and Char forward to `192.168.1.50:6900` and `:6121`, while Map forwards to `192.168.1.60:5122`.*

### Scenario D: Port-Only Shorthand
Port-only shorthand (e.g. `:7000` or `7000`) inherits the target IP:

```bash
cargo run -p ro-proxy -- -t 10.0.0.5 -l :6905 -c :6125 -m :5125
```

---

## 5. Log Output & Diagnostics

### Real-Time Packet Stream
Every packet is logged with its session identifier, sequence index, direction, opcode, length, and decoded intent/payload preview:

```text
INFO [LOGIN Session #1] [Seq #1  ] [>> CLIENT -> SERVER] 0x0064 len=55 => Decoded: Login { username: "user", client_version: 20211103, client_type: 1 }
INFO [LOGIN Session #1] [Seq #2  ] [<< SERVER -> CLIENT] 0x0069 len=75 => payload: [e9 03 00 00 ...] |....|
INFO [CHAR  Session #2] [Seq #1  ] [>> CLIENT -> SERVER] 0x0065 len=19 => Decoded: EnterCharServer { account_id: AccountId(2000001), ... }
INFO [CHAR  Session #2] [Seq #2  ] [<< SERVER -> CLIENT] 0x006b len=32 => payload: [01 02 03 ...] |...|
```

### Unmapped Opcode Notifications
When a packet opcode is not yet registered in `table.rs`:

```text
WARN [MAP Session #3] [Seq #5  ] [<< SERVER -> CLIENT] UNMAPPED OPCODE: 0x0abc (reason: Unknown or unregistered packet opcode: 0x0abc, consumed 6 bytes) (candidate dynamic len=6), payload: [01 02 03 04]
```

### End-of-Session Chronological Trace
When a client disconnects, the proxy outputs the entire conversational packet sequence:

```text
INFO [LOGIN Session #1] Connection closed for client 127.0.0.1:54321 <-> target 127.0.0.1:6900
     Chronological Packet Sequence (2 packets):
       Seq #1   [CLIENT -> SERVER] 0x0064 (len=55) => Decoded: Login { username: "user", ... }
       Seq #2   [SERVER -> CLIENT] 0x0069 (len=75) => payload: [e9 03 00 00 ...]
```

---

## 6. Controlling Log Verbosity

Use `RUST_LOG` to control tracing output:

```bash
# Default info logging
RUST_LOG=info cargo run -p ro-proxy -- -t 192.168.1.100

# Verbose debug logging (includes framing internals and socket read events)
RUST_LOG=debug cargo run -p ro-proxy -- -t 192.168.1.100
```

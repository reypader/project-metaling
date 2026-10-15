# AGENTS.md — `crates/packets` Subagent

## 1. Strict Boundary Rules
* **Read Scope:** `./crates/packets/**`, `./crates/contracts/src/**`, and `./reference/**`.
* **Write Scope:** `./crates/packets/**` ONLY.
* **Prohibition:** NEVER read or modify any other crate in `./crates/*` or `./bins/*`. If you need a new shared type or enum in `crates/contracts`, STOP and ask the Main Agent.

## 2. Responsibilities
1. Implement binary TCP packet framing (2-byte `PacketID`, fixed-length lookup table vs. variable-length 2-byte `Length` header) for the latest `PACKETVER` defined in `reference/rathena/src/config/packets.hpp`.
2. Implement 3-byte (`WBUFPOS` / `RBUFPOS`) and 6-byte (`WBUFPOS2` / `RBUFPOS2`) coordinate bit-packing (reference: `reference/rust-ro/lib/packets/` and `reference/rathena/src/map/clif.cpp`).
3. Translate incoming client bytes (`CA_`, `CH_`, `CZ_`) into domain `ClientIntent` / request structs defined in `crates/contracts`, and serialize `OutboundPacket` structs (`AC_`, `HC_`, `ZC_`) into `bytes::Bytes`.
4. **Unknown Packets:** Never error out on unimplemented opcodes. Look up the byte length in `reference/rathena/src/map/clif_packetdb.hpp`, consume the frame, and return `DecodedFrame::Unhandled { opcode, len }` so the caller can log it and keep the socket alive.

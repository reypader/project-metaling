# Coordinate and Movement Bit-Packing (`WBUFPOS` / `WBUFPOS2`)

Target client version: `PACKETVER = 20260219`.

## `WBUFPOS` (3-Byte Coordinate and Direction Packing)

* **Provenance:** `rathena` (`src/map/clif.cpp`, `WBUFPOS` / `RBUFPOS` macros) & `rust-ro` (`lib/packets/`), `PACKETVER = 20260219`.
* **Purpose:** Encodes a 10-bit `x` coordinate (`0..1023`), a 10-bit `y` coordinate (`0..1023`), and a 4-bit `dir` orientation (`0..15`) into 3 contiguous bytes (`[u8; 3]`).
* **Bit Layout:**
  * `byte[0] = (x >> 2) as u8`
  * `byte[1] = (((x & 0x03) << 6) | ((y >> 4) & 0x3f)) as u8`
  * `byte[2] = (((y & 0x0f) << 4) | (dir & 0x0f)) as u8`
* **Unpacking (`RBUFPOS`):**
  * `x = ((byte[0] as u16) << 2) | ((byte[1] as u16) >> 6)`
  * `y = (((byte[1] as u16) & 0x3f) << 4) | ((byte[2] as u16) >> 4)`
  * `dir = byte[2] & 0x0f`

## `WBUFPOS2` (6-Byte Movement Pair Packing)

* **Provenance:** `rathena` (`src/map/clif.cpp`, `WBUFPOS2` / `RBUFPOS2` macros) & `rust-ro` (`lib/packets/`), `PACKETVER = 20260219`.
* **Purpose:** Encodes origin coordinates `(x0, y0)` (10 bits each), destination coordinates `(x1, y1)` (10 bits each), and sub-cell offsets `(sx0, sy0)` (4 bits each, typically `8` for cell center) into 6 contiguous bytes (`[u8; 6]`).
* **Bit Layout:**
  * `byte[0] = (x0 >> 2) as u8`
  * `byte[1] = (((x0 & 0x03) << 6) | ((y0 >> 4) & 0x3f)) as u8`
  * `byte[2] = (((y0 & 0x0f) << 4) | ((x1 >> 6) & 0x0f)) as u8`
  * `byte[3] = (((x1 & 0x3f) << 2) | ((y1 >> 8) & 0x03)) as u8`
  * `byte[4] = (y1 & 0xff) as u8`
  * `byte[5] = (((sx0 & 0x0f) << 4) | (sy0 & 0x0f)) as u8`

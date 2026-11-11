# EPC wire protocol, version 1

The protocol runs over any reliable, ordered byte stream. Today that stream is Bluetooth RFCOMM; USB will use the same protocol later.

## Transport (Bluetooth)
- **Service UUID:** `540c3dea-5e95-4872-993a-c1e25e3b021d`.
- **Phone:** listens on a secure (bonded) RFCOMM socket and registers an SDP record for the UUID.
- **Laptop:** dials the phone through BlueZ `ConnectProfile(uuid)` and retries with backoff.
- **One connection at a time.** The phone serves connections one after another; a second dial waits until the current one ends. A dead link is detected by RFCOMM link loss.

## Framing
```
+----------------+---------------------------+
| u32 big-endian | N bytes UTF-8 JSON object |
|  length N      |                           |
+----------------+---------------------------+
```
- N must be between 1 and 1 MiB (1048576). A peer that receives a frame outside that range drops the connection.
- Every object has a string field `t` that gives the message type.
- Receivers **ignore** unknown `t` values and unknown fields. This lets either side be updated without breaking the other.

## Messages

| `t`        | Direction       | Fields                                          | Meaning |
|------------|-----------------|-------------------------------------------------|---------|
| `hello`    | both, sent first | `v` (int, 1), `name` (str)                     | Handshake. A `v` mismatch is logged. Version 1 has nothing incompatible yet; a future breaking version will drop the connection. |
| `cfg`      | laptop → phone  | `clipboard` (bool), `notifications` (bool)      | Current sync settings. Sent after `hello` and on every change. The phone skips work that is disabled, which saves battery. |
| `clip`     | both            | `text` (str)                                    | Clipboard contents. The receiver sets its clipboard and must not echo the text back. |
| `notif`    | phone → laptop  | `key` (str), `app` (str), `title` (str), `text` (str) | A notification was posted or updated. `key` is stable for the same notification, so a repeated `key` replaces the earlier one. |
| `notif_rm` | phone → laptop  | `key` (str)                                     | The notification was dismissed on the phone. |

## Echo suppression
Each side remembers the last clipboard text it **sent or received**. It only sends a `clip` when the local clipboard changes to a different text.

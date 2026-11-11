# EfficientPhoneConnectivity — design notes

Goal: a lightweight phone ↔ Linux laptop link. No bloat, first-class Linux.

## Scope (in order)
1. **Transport: Bluetooth** (RFCOMM, custom service UUID). USB comes later and reuses the same protocol.
2. **Shared clipboard** (text first).
3. **Notification mirroring** (phone → laptop).
4. *Deferred:* simple screen control.

## Targets
- Phone: Samsung, One UI 9 / Android 17 (minSdk ~31, target latest).
- Laptop: CachyOS, KDE Plasma on Wayland, BlueZ 5.88.

## Decisions
- **Laptop daemon in Rust.** You want it running while gaming and easy on battery, so it should be a single event-driven binary that sleeps when idle, with no interpreter and a small memory footprint.
  - BlueZ via D-Bus (`bluer` crate): register a `ProfileManager1` profile with the custom UUID.
  - Clipboard via Klipper D-Bus: `setClipboardContents`, `getClipboardContents`, and the `clipboardHistoryUpdated` signal. It is event-driven, so there is no polling.
  - Notifications via `org.freedesktop.Notifications`.
  - Only accepts the bonded phone's address.
- **Who listens:** the phone blocks in `accept()`, which costs it no CPU. The laptop dials and owns the retry and backoff, so reconnect cost stays off the phone. Still to check: BlueZ client-role behaviour.
- **Protocol:** length-prefixed typed frames over a plain byte stream, with a version handshake at the start. Echo suppression stops a received clip from being sent straight back.

## Android constraints (known up front)
- **Phone → laptop clipboard cannot be automatic.** Since Android 10, background apps can't read the clipboard. It needs a trigger: a Quick Settings tile, a notification action or a share target. Laptop → phone works in the background.
- **Notification access is granted by hand.** The user enables the NotificationListenerService in settings. A sideloaded app must first be allowed past "restricted settings".
- **The connection needs a foreground service** of type `connectedDevice`, plus the `BLUETOOTH_CONNECT` and `BLUETOOTH_SCAN` runtime permissions.

## Status (2026-11-11)
Bluetooth link, clipboard (both directions), notification mirroring and the tray settings menu all work. They were tested end to end on an S23 Ultra running Android 17.

**Toolchains:** Rust is in `~/.cargo` and the Android SDK in `~/Android/Sdk` (platform 37). The Gradle wrapper is in `android/`.

## Big idea: zero-install phone side
You install EPC **once on the laptop**, from GitHub, and nothing on the phone. The laptop sets up the phone by itself:
- **Phone part:** a tiny helper the laptop pushes over adb (USB or wireless debugging). It runs as Android's shell user, like scrcpy and Shizuku do, so there is no APK and no permission screens on the phone.
- **What shell gives us:** `READ_CLIPBOARD_IN_BACKGROUND`, `CAPTURE_VIDEO_OUTPUT` and `INJECT_EVENTS` (all checked on a One UI 9 / Android 17 phone). That covers automatic clipboard both ways (text and images), screen mirroring and screen control.
- **Limits:**
  - The helper dies when the phone reboots. The laptop restarts it the next time the phone is plugged in or reachable over wireless debugging.
  - Notification mirroring may still need a real NotificationListenerService, which means an APK. That needs looking into.
- **Not prototyped yet.** It replaces the current "install the APK" setup if it works.

## Ideas / later
- **USB transport.** Options are adb forward or AOA, reusing the same frame protocol.
- **Gallery and camera.** Browse and pull photos, or use the phone as a webcam. Not decided yet.
- **Screen control.**
- **`wl-clipboard` backend** for non-KDE desktops.
- **Laptop side:** reply to and dismiss notifications.

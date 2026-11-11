# EPC — Efficient Phone Connectivity

> **⚠️ Version 0.1. This project has just started and practically doesn't work yet.**
> It's an early prototype tested on one phone and one laptop. Expect breakage and missing features.
> Large parts of the design (see [DESIGN.md](DESIGN.md)) will change. Don't rely on it.

A small link between an Android phone and a Linux laptop over Bluetooth. It does two things today:
- **Shared clipboard:** text, in both directions.
- **Phone notifications** shown on the laptop.

How small it is:
- **Laptop daemon:** about 3.6 MB on disk and about 7 MB of RAM, on one thread. It does no work while idle, so it's fine to leave running while gaming.
- **Phone app:** 48 KB. It waits in Bluetooth `accept()`, so it costs nothing while idle.

```
laptop/    Rust daemon `epcd` + tray icon (KDE/Plasma, BlueZ)
android/   Android app (Kotlin, no AndroidX)
PROTOCOL.md  wire format (shared by Bluetooth now and USB later)
DESIGN.md    decisions and constraints
```

## Install

**Laptop:** this builds the daemon, installs it to `~/.local/bin`, adds it to login autostart and starts it.
```bash
laptop/install.sh
```

**Phone:** connect it over USB with debugging enabled.
```bash
cd android && ./gradlew installRelease
```

Then open **EPC** on the phone:
1. Grant the Bluetooth and notification permissions.
2. Allow notification access.
3. Allow it to run in the background.
4. Pair the phone with the laptop in Bluetooth settings.

The laptop finds the paired phone by itself.

## Use
- **Tray icon:** the phone icon in the system tray, crossed out when disconnected.
  - **Sync clipboard** and **Show phone notifications** switch each feature on or off for both sides.
  - The **Phone** menu picks which paired phone to use if you have several.
  - Settings are saved in `~/.config/epc/config.json`.
- **Laptop → phone clipboard** is automatic.
- **Phone → laptop clipboard** needs one tap. Android doesn't let background apps read the clipboard, so use one of these:
  - the **Send clipboard** button on the EPC notification,
  - the **Send clipboard** Quick Settings tile,
  - **Share → Send to laptop**.

## Notes
- **Reconnect cost:** when the phone is out of range, the laptop retries with backoff, up to once a minute. The phone never retries.
- **Clipboard needs Klipper:** the laptop clipboard goes through KDE's Klipper over D-Bus. Other desktops would need a `wl-clipboard` backend, which isn't written yet.
- **Notifications that aren't mirrored:**
  - ongoing notifications (media, downloads),
  - group summaries,
  - "local only" notifications,
  - notifications from EPC itself.

## License
[MIT](LICENSE). Anyone may use EPC for any purpose. It comes with no warranty, and you are responsible for how you use it.

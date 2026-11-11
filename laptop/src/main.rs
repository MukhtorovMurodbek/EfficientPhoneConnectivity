//! epcd — lightweight phone link daemon (Bluetooth, clipboard, notifications).
//! Single-threaded and fully event-driven: idle cost is zero wakeups.

mod config;
mod desktop;
mod link;
mod proto;
mod tray;

use bluer::Address;
use config::Config;
use desktop::{KlipperProxy, Mirror, NotificationsProxy};
use futures_util::StreamExt;
use ksni::TrayMethods;
use link::LinkEvent;
use proto::Msg;
use std::sync::{Arc, Mutex};
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::{Notify, mpsc};

#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    Starting,
    NoAdapter,
    AdapterOff,
    NoPhone,
    Connecting(String),
    Connected(String),
}

pub struct Shared {
    pub cfg: Mutex<Config>,
    pub phones: Mutex<Vec<(Address, String)>>,
    pub status: Mutex<Status>,
}

impl Shared {
    pub fn set_status(&self, s: Status) {
        *self.status.lock().unwrap() = s;
    }
    /// Configured phone, or the first paired phone when set to automatic.
    pub fn target(&self) -> Option<Address> {
        let phones = self.phones.lock().unwrap();
        match self.cfg.lock().unwrap().device.as_deref().and_then(link::parse_addr) {
            Some(a) => Some(a),
            None => phones.first().map(|(a, _)| *a),
        }
    }
}

pub enum Cmd {
    CfgChanged,
    Reconnect,
    Quit,
}

fn hostname() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname").map(|s| s.trim().to_string()).unwrap_or_else(|_| "Linux".into())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dbus = zbus::Connection::session().await?;
    if dbus.request_name("dev.epc.Daemon").await.is_err() {
        eprintln!("epcd: already running");
        return Ok(());
    }

    let shared = Arc::new(Shared {
        cfg: Mutex::new(Config::load()),
        phones: Mutex::new(Vec::new()),
        status: Mutex::new(Status::Starting),
    });
    let (ev_tx, mut ev_rx) = mpsc::unbounded_channel();
    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel();
    let kick = Arc::new(Notify::new());

    // At login we may start before Plasma's tray host; keep waiting for it instead of failing.
    let tray_svc = tray::Tray { shared: shared.clone(), cmd: cmd_tx.clone() }.assume_sni_available(true);
    let tray = match tray_svc.spawn().await {
        Ok(h) => Some(h),
        Err(e) => {
            eprintln!("epcd: no tray available ({e}); running headless");
            None
        }
    };

    let klipper = KlipperProxy::new(&dbus).await?;
    let mut clip_changes = klipper.receive_clipboard_history_updated().await?;
    let mut mirror = Mirror::new(NotificationsProxy::new(&dbus).await?);

    tokio::spawn(link::run(shared.clone(), ev_tx, kick.clone()));

    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;
    let me = hostname();
    let mut out: Option<mpsc::Sender<Msg>> = None;
    let mut last_clip = klipper.get_clipboard_contents().await.unwrap_or_default();

    let cfg_msg = |s: &Shared| {
        let c = s.cfg.lock().unwrap();
        Msg::Cfg { clipboard: c.clipboard, notifications: c.notifications }
    };
    let send = |out: &Option<mpsc::Sender<Msg>>, m: Msg| {
        if let Some(tx) = out {
            let _ = tx.try_send(m);
        }
    };

    loop {
        tokio::select! {
            Some(ev) = ev_rx.recv() => match ev {
                LinkEvent::Connected { tx } => {
                    out = Some(tx);
                    send(&out, Msg::Hello { v: proto::VERSION, name: me.clone() });
                    send(&out, cfg_msg(&shared));
                    refresh(&tray).await;
                }
                LinkEvent::Disconnected => {
                    out = None;
                    refresh(&tray).await;
                }
                LinkEvent::Refresh => refresh(&tray).await,
                LinkEvent::Msg(m) => {
                    let cfg = shared.cfg.lock().unwrap().clone();
                    let phone = match &*shared.status.lock().unwrap() {
                        Status::Connected(n) => n.clone(),
                        _ => "Phone".into(),
                    };
                    match m {
                        Msg::Hello { v, name } if v != proto::VERSION => {
                            eprintln!("epcd: {name} speaks protocol v{v}, expected v{}", proto::VERSION);
                        }
                        Msg::Clip { text } if cfg.clipboard && text != last_clip => {
                            last_clip = text.clone();
                            if let Err(e) = klipper.set_clipboard_contents(&text).await {
                                eprintln!("epcd: set clipboard: {e}");
                            }
                        }
                        Msg::Notif { key, app, title, text } if cfg.notifications => {
                            mirror.show(&phone, key, &app, &title, &text).await;
                        }
                        Msg::NotifRm { key } => mirror.remove(&key).await,
                        _ => {}
                    }
                }
            },
            Some(cmd) = cmd_rx.recv() => match cmd {
                Cmd::CfgChanged => {
                    send(&out, cfg_msg(&shared));
                    refresh(&tray).await;
                }
                Cmd::Reconnect => kick.notify_one(),
                Cmd::Quit => break,
            },
            Some(_) = clip_changes.next() => {
                if out.is_some() && shared.cfg.lock().unwrap().clipboard {
                    let text = klipper.get_clipboard_contents().await.unwrap_or_default();
                    if !text.is_empty() && text != last_clip && text.len() < proto::MAX_FRAME - 64 {
                        last_clip = text.clone();
                        send(&out, Msg::Clip { text });
                    }
                }
            }
            _ = sigterm.recv() => break,
            _ = sigint.recv() => break,
        }
    }
    Ok(())
}

async fn refresh(tray: &Option<ksni::Handle<tray::Tray>>) {
    if let Some(h) = tray {
        h.update(|_| {}).await;
    }
}

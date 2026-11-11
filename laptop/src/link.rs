//! Bluetooth link: the laptop dials the phone's RFCOMM service through BlueZ
//! (ConnectProfile does the SDP lookup), then speaks the frame protocol.
//! The phone just sits in accept(), so all retry cost lives on the laptop.

use crate::proto::{self, Msg, SERVICE_UUID};
use crate::{Shared, Status};
use bluer::rfcomm::{Profile, ProfileHandle, ReqError, Role, Stream};
use bluer::{Adapter, Address, Device, Session, Uuid};
use futures_util::StreamExt;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Notify, mpsc};
use tokio::time::sleep;

pub enum LinkEvent {
    Connected { tx: mpsc::Sender<Msg> },
    Msg(Msg),
    Disconnected,
    /// Shared state (phone list / status) changed; refresh the tray.
    Refresh,
}

const BACKOFF_MIN: Duration = Duration::from_secs(5);
/// Upper bound between dial attempts while the phone is away. Paging is cheap,
/// but rare enough here not to disturb Bluetooth audio while gaming.
const BACKOFF_MAX: Duration = Duration::from_secs(60);
const IDLE: Duration = Duration::from_secs(30);

pub async fn run(shared: Arc<Shared>, ev: mpsc::UnboundedSender<LinkEvent>, kick: Arc<Notify>) {
    loop {
        if let Err(e) = run_session(&shared, &ev, &kick).await {
            eprintln!("epcd: bluetooth: {e}");
        }
        shared.set_status(Status::NoAdapter);
        let _ = ev.send(LinkEvent::Refresh);
        tokio::select! { _ = sleep(IDLE) => {}, _ = kick.notified() => {} }
    }
}

async fn run_session(shared: &Shared, ev: &mpsc::UnboundedSender<LinkEvent>, kick: &Notify) -> bluer::Result<()> {
    let session = Session::new().await?;
    let adapter = session.default_adapter().await?;
    let uuid = Uuid::from_u128(SERVICE_UUID);
    let mut profile = session
        .register_profile(Profile {
            uuid,
            name: Some("EPC".into()),
            role: Some(Role::Client),
            require_authentication: Some(true),
            require_authorization: Some(false),
            auto_connect: Some(false),
            ..Default::default()
        })
        .await?;

    let mut backoff = BACKOFF_MIN;
    let mut last_err = String::new();
    loop {
        refresh_phones(&adapter, shared).await;
        let target = shared.target();
        let wait = match target {
            _ if !adapter.is_powered().await? => Some(Status::AdapterOff),
            None => Some(Status::NoPhone),
            Some(_) => None,
        };
        if let Some(st) = wait {
            shared.set_status(st);
            let _ = ev.send(LinkEvent::Refresh);
            tokio::select! { _ = sleep(IDLE) => {}, _ = kick.notified() => {} }
            continue;
        }
        let device = adapter.device(target.unwrap())?;
        let name = device.alias().await.unwrap_or_else(|_| device.address().to_string());
        shared.set_status(Status::Connecting(name.clone()));
        let _ = ev.send(LinkEvent::Refresh);

        match connect(&device, &mut profile, uuid).await {
            Ok(stream) => {
                eprintln!("epcd: connected to {name}");
                last_err.clear();
                backoff = BACKOFF_MIN;
                shared.set_status(Status::Connected(name.clone()));
                serve(stream, ev, kick).await;
                eprintln!("epcd: disconnected from {name}");
            }
            Err(e) => {
                if e != last_err {
                    eprintln!("epcd: connecting to {name}: {e} (retrying)");
                    last_err = e;
                }
                tokio::select! { _ = sleep(backoff) => {}, _ = kick.notified() => {} }
                backoff = (backoff * 2).min(BACKOFF_MAX);
            }
        }
    }
}

/// Paired devices that BlueZ classifies as phones.
async fn refresh_phones(adapter: &Adapter, shared: &Shared) {
    let mut phones = Vec::new();
    for addr in adapter.device_addresses().await.unwrap_or_default() {
        let Ok(d) = adapter.device(addr) else { continue };
        if d.is_paired().await.unwrap_or(false) && d.icon().await.ok().flatten().as_deref() == Some("phone") {
            let name = d.alias().await.unwrap_or_else(|_| addr.to_string());
            phones.push((addr, name));
        }
    }
    phones.sort();
    *shared.phones.lock().unwrap() = phones;
}

async fn connect(device: &Device, profile: &mut ProfileHandle, uuid: Uuid) -> Result<Stream, String> {
    let call = device.connect_profile(&uuid);
    tokio::pin!(call);
    let deadline = sleep(Duration::from_secs(30));
    tokio::pin!(deadline);
    let mut call_done = false;
    loop {
        tokio::select! {
            req = profile.next() => {
                let req = req.ok_or("profile unregistered")?;
                if req.device() != device.address() {
                    req.reject(ReqError::Rejected);
                    continue;
                }
                return req.accept().map_err(|e| e.to_string());
            }
            res = &mut call, if !call_done => {
                call_done = true;
                res.map_err(|e| e.to_string())?;
            }
            _ = &mut deadline => return Err("timed out".into()),
        }
    }
}

async fn serve(stream: Stream, ev: &mpsc::UnboundedSender<LinkEvent>, kick: &Notify) {
    let (mut rd, mut wr) = stream.into_split();
    let (tx, mut rx) = mpsc::channel::<Msg>(64);
    let _ = ev.send(LinkEvent::Connected { tx });
    let writer = async {
        while let Some(m) = rx.recv().await {
            proto::write_msg(&mut wr, &m).await?;
        }
        Ok::<_, std::io::Error>(())
    };
    let reader = async {
        loop {
            let m = proto::read_msg(&mut rd).await?;
            let _ = ev.send(LinkEvent::Msg(m));
        }
        #[allow(unreachable_code)]
        Ok::<_, std::io::Error>(())
    };
    tokio::select! {
        r = writer => if let Err(e) = r { eprintln!("epcd: write: {e}") },
        r = reader => if let Err(e) = r { eprintln!("epcd: read: {e}") },
        _ = kick.notified() => {}
    }
    let _ = ev.send(LinkEvent::Disconnected);
}

pub fn parse_addr(s: &str) -> Option<Address> {
    s.parse().ok()
}

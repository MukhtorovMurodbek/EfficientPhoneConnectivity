//! Desktop integration over the session bus: Klipper (KDE clipboard) and
//! org.freedesktop.Notifications. Both are event-driven — nothing polls.

use std::collections::HashMap;
use zbus::zvariant::Value;

#[zbus::proxy(
    interface = "org.kde.klipper.klipper",
    default_service = "org.kde.klipper",
    default_path = "/klipper"
)]
pub trait Klipper {
    #[zbus(name = "getClipboardContents")]
    fn get_clipboard_contents(&self) -> zbus::Result<String>;

    #[zbus(name = "setClipboardContents")]
    fn set_clipboard_contents(&self, s: &str) -> zbus::Result<()>;

    #[zbus(signal, name = "clipboardHistoryUpdated")]
    fn clipboard_history_updated(&self) -> zbus::Result<()>;
}

#[zbus::proxy(
    interface = "org.freedesktop.Notifications",
    default_service = "org.freedesktop.Notifications",
    default_path = "/org/freedesktop/Notifications"
)]
pub trait Notifications {
    #[allow(clippy::too_many_arguments)]
    fn notify(
        &self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: &[&str],
        hints: HashMap<&str, Value<'_>>,
        expire_timeout: i32,
    ) -> zbus::Result<u32>;

    fn close_notification(&self, id: u32) -> zbus::Result<()>;
}

/// Mirrors phone notifications, keeping phone key -> desktop id so updates
/// replace in place and dismissals on the phone close the desktop popup.
pub struct Mirror {
    proxy: NotificationsProxy<'static>,
    ids: HashMap<String, u32>,
}

impl Mirror {
    pub fn new(proxy: NotificationsProxy<'static>) -> Self {
        Self { proxy, ids: HashMap::new() }
    }

    pub async fn show(&mut self, phone: &str, key: String, app: &str, title: &str, text: &str) {
        let replaces = self.ids.get(&key).copied().unwrap_or(0);
        let summary = if title.is_empty() { app } else { title };
        let mut hints = HashMap::new();
        // No desktop-entry hint: it would make Plasma show "EPC" instead of the phone app's name.
        hints.insert("x-kde-origin-name", Value::from(phone));
        let app_name = if app.is_empty() { "Phone" } else { app };
        match self
            .proxy
            .notify(app_name, replaces, "smartphone", summary, &escape(text), &[], hints, -1)
            .await
        {
            Ok(id) => {
                self.ids.insert(key, id);
            }
            Err(e) => eprintln!("epcd: notify: {e}"),
        }
    }

    pub async fn remove(&mut self, key: &str) {
        if let Some(id) = self.ids.remove(key) {
            let _ = self.proxy.close_notification(id).await;
        }
    }
}

/// Notification bodies may contain a markup subset; show phone text literally.
fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

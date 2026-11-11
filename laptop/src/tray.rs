//! Tray icon (StatusNotifierItem). The menu *is* the settings UI.

use crate::{Cmd, Shared, Status};
use ksni::menu::{CheckmarkItem, RadioGroup, RadioItem, StandardItem, SubMenu};
use ksni::{MenuItem, ToolTip};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

pub struct Tray {
    pub shared: Arc<Shared>,
    pub cmd: UnboundedSender<Cmd>,
}

impl Tray {
    fn status_text(&self) -> String {
        match &*self.shared.status.lock().unwrap() {
            Status::Starting => "Starting…".into(),
            Status::NoAdapter => "Bluetooth unavailable".into(),
            Status::AdapterOff => "Bluetooth is off".into(),
            Status::NoPhone => "No paired phone".into(),
            Status::Connecting(n) => format!("Waiting for {n}"),
            Status::Connected(n) => format!("Connected to {n}"),
        }
    }

    fn toggle(&mut self, f: impl FnOnce(&mut crate::config::Config)) {
        let mut cfg = self.shared.cfg.lock().unwrap();
        f(&mut cfg);
        cfg.save();
        let _ = self.cmd.send(Cmd::CfgChanged);
    }
}

impl ksni::Tray for Tray {
    const MENU_ON_ACTIVATE: bool = true;

    fn id(&self) -> String {
        "epcd".into()
    }
    fn title(&self) -> String {
        "Phone".into()
    }
    fn category(&self) -> ksni::Category {
        ksni::Category::Hardware
    }
    fn icon_name(&self) -> String {
        "smartphone-symbolic".into()
    }
    fn overlay_icon_name(&self) -> String {
        match *self.shared.status.lock().unwrap() {
            Status::Connected(_) => String::new(),
            _ => "emblem-unavailable".into(),
        }
    }
    fn tool_tip(&self) -> ToolTip {
        ToolTip { title: "Phone".into(), description: self.status_text(), ..Default::default() }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let cfg = self.shared.cfg.lock().unwrap().clone();
        let phones = self.shared.phones.lock().unwrap().clone();

        let mut options = vec![RadioItem { label: "Automatic".into(), ..Default::default() }];
        options.extend(phones.iter().map(|(a, n)| RadioItem { label: format!("{n}  ({a})"), ..Default::default() }));
        let selected = cfg
            .device
            .as_deref()
            .and_then(|d| phones.iter().position(|(a, _)| a.to_string().eq_ignore_ascii_case(d)))
            .map_or(0, |i| i + 1);

        vec![
            StandardItem { label: self.status_text(), enabled: false, ..Default::default() }.into(),
            MenuItem::Separator,
            CheckmarkItem {
                label: "Sync clipboard".into(),
                checked: cfg.clipboard,
                activate: Box::new(|t: &mut Self| t.toggle(|c| c.clipboard = !c.clipboard)),
                ..Default::default()
            }
            .into(),
            CheckmarkItem {
                label: "Show phone notifications".into(),
                checked: cfg.notifications,
                activate: Box::new(|t: &mut Self| t.toggle(|c| c.notifications = !c.notifications)),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            SubMenu {
                label: "Phone".into(),
                icon_name: "smartphone-symbolic".into(),
                submenu: vec![
                    RadioGroup {
                        selected,
                        select: Box::new(move |t: &mut Self, i| {
                            let dev = if i == 0 { None } else { phones.get(i - 1).map(|(a, _)| a.to_string()) };
                            t.toggle(|c| c.device = dev);
                            let _ = t.cmd.send(Cmd::Reconnect);
                        }),
                        options,
                    }
                    .into(),
                ],
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|t: &mut Self| {
                    let _ = t.cmd.send(Cmd::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

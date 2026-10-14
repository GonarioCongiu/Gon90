//! Server delle notifiche (org.freedesktop.Notifications) e "lanci d'agenzia":
//! le schede che compaiono in alto a destra.
//!
//! Il server D-Bus gira nell'esecutore interno di zbus; ogni notifica arriva
//! al thread GTK tramite un canale. Se un altro demone (mako, dunst, swaync)
//! possiede già il nome, Folio si limita a mostrare il Notiziario vuoto.

use super::text;
use crate::glyph::{self, Glyph};
use crate::layer::{self, Keyboard, Level};
use crate::shell::shell;
use chrono::{DateTime, Local};
use gtk::glib;
use gtk::prelude::*;
use gtk4_layer_shell::{Edge, LayerShell};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use zbus::zvariant::OwnedValue;

const PATH: &str = "/org/freedesktop/Notifications";
const IFACE: &str = "org.freedesktop.Notifications";

#[derive(Debug, Clone)]
pub struct Note {
    pub id: u32,
    pub app: String,
    pub icon: String,
    pub summary: String,
    pub body: String,
    /// Coppie (chiave, etichetta).
    pub actions: Vec<(String, String)>,
    pub urgency: u8,
    pub desktop: Option<String>,
    pub timeout: i32,
    pub time: DateTime<Local>,
}

enum Msg {
    Notify(Note),
    Close(u32),
}

struct Server {
    tx: async_channel::Sender<Msg>,
    next: AtomicU32,
}

fn hint_str(hints: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    hints
        .get(key)
        .and_then(|v| v.downcast_ref::<&str>().ok().map(|s| s.to_string()))
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl Server {
    fn get_capabilities(&self) -> Vec<String> {
        ["body", "actions", "persistence", "icon-static"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    #[allow(clippy::too_many_arguments)]
    fn notify(
        &self,
        app_name: String,
        replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        hints: HashMap<String, OwnedValue>,
        expire_timeout: i32,
    ) -> u32 {
        let id = if replaces_id != 0 {
            replaces_id
        } else {
            self.next.fetch_add(1, Ordering::SeqCst)
        };
        let urgency = hints
            .get("urgency")
            .and_then(|v| v.downcast_ref::<u8>().ok())
            .unwrap_or(1);
        let icon = if app_icon.is_empty() {
            hint_str(&hints, "image-path").unwrap_or_default()
        } else {
            app_icon
        };
        let actions = actions
            .chunks(2)
            .filter(|c| c.len() == 2)
            .map(|c| (c[0].clone(), c[1].clone()))
            .collect();
        let note = Note {
            id,
            app: app_name,
            icon,
            summary,
            body,
            actions,
            urgency,
            desktop: hint_str(&hints, "desktop-entry"),
            timeout: expire_timeout,
            time: Local::now(),
        };
        let _ = self.tx.try_send(Msg::Notify(note));
        id
    }

    fn close_notification(&self, id: u32) {
        let _ = self.tx.try_send(Msg::Close(id));
    }

    fn get_server_information(&self) -> (String, String, String, String) {
        (
            "Folio".into(),
            "Folio".into(),
            folio_core::VERSION.into(),
            "1.2".into(),
        )
    }
}

pub struct Notifier {
    notes: RefCell<Vec<Note>>,
    dnd: Cell<bool>,
    conn: RefCell<Option<zbus::blocking::Connection>>,
    toast_window: gtk::Window,
    toast_box: gtk::Box,
    toasts: RefCell<Vec<(u32, gtk::Widget)>>,
}

impl Notifier {
    pub fn new() -> Rc<Self> {
        let toast_window = layer::surface("folio-toast", Level::Overlay, Keyboard::None);
        toast_window.set_anchor(Edge::Top, true);
        toast_window.set_anchor(Edge::Right, true);
        // Zona esclusiva 0: i margini partono dall'area libera, quindi i lanci
        // restano appena sotto la Testata ovunque la si sposti.
        toast_window.set_margin(Edge::Top, 14);
        toast_window.set_margin(Edge::Right, 14);
        toast_window.set_exclusive_zone(0);
        let toast_box = gtk::Box::new(gtk::Orientation::Vertical, 10);
        toast_box.add_css_class("toasts");
        toast_window.set_child(Some(&toast_box));

        let n = Rc::new(Self {
            notes: RefCell::new(Vec::new()),
            dnd: Cell::new(false),
            conn: RefCell::new(None),
            toast_window,
            toast_box,
            toasts: RefCell::new(Vec::new()),
        });

        let (tx, rx) = async_channel::unbounded::<Msg>();
        let (ctx, crx) = async_channel::bounded::<Option<zbus::blocking::Connection>>(1);
        std::thread::Builder::new()
            .name("folio-notify".into())
            .spawn(move || {
                let server = Server {
                    tx,
                    next: AtomicU32::new(1),
                };
                let built = zbus::blocking::connection::Builder::session()
                    .and_then(|b| b.name(IFACE))
                    .and_then(|b| b.serve_at(PATH, server))
                    .and_then(|b| b.build());
                match built {
                    Ok(conn) => {
                        log::info!("server delle notifiche attivo");
                        let _ = ctx.send_blocking(Some(conn));
                    }
                    Err(e) => {
                        log::warn!("notifiche: {e} (c'è già un altro demone? mako/dunst/swaync)");
                        let _ = ctx.send_blocking(None);
                    }
                }
            })
            .ok();
        glib::spawn_future_local(async move {
            if let Ok(conn) = crx.recv().await {
                if let Some(n) = shell().notifier.get() {
                    *n.conn.borrow_mut() = conn;
                }
            }
        });
        glib::spawn_future_local(async move {
            while let Ok(msg) = rx.recv().await {
                let Some(n) = shell().notifier.get().cloned() else {
                    continue;
                };
                match msg {
                    Msg::Notify(note) => n.add(note),
                    Msg::Close(id) => n.remove(id, 3),
                }
            }
        });
        n
    }

    pub fn count(&self) -> usize {
        self.notes.borrow().len()
    }

    pub fn notes(&self) -> Vec<Note> {
        self.notes.borrow().clone()
    }

    pub fn dnd(&self) -> bool {
        self.dnd.get()
    }

    pub fn set_dnd(&self, on: bool) {
        self.dnd.set(on);
        if on {
            self.clear_toasts();
        }
    }

    fn emit(&self, signal: &str, body: &(u32, impl serde::Serialize + zbus::zvariant::Type)) {
        if let Some(conn) = self.conn.borrow().as_ref() {
            if let Err(e) =
                conn.emit_signal(None::<zbus::names::BusName>, PATH, IFACE, signal, body)
            {
                log::debug!("segnale {signal}: {e}");
            }
        }
    }

    fn add(&self, note: Note) {
        {
            let mut notes = self.notes.borrow_mut();
            notes.retain(|n| n.id != note.id);
            notes.insert(0, note.clone());
            notes.truncate(80);
        }
        if !self.dnd.get() || note.urgency >= 2 {
            self.toast(&note);
        }
        self.changed();
    }

    /// `reason`: 1 scaduta, 2 chiusa dall'utente, 3 chiusa dall'app.
    pub fn remove(&self, id: u32, reason: u32) {
        self.notes.borrow_mut().retain(|n| n.id != id);
        self.drop_toast(id);
        self.emit("NotificationClosed", &(id, reason));
        self.changed();
    }

    pub fn clear(&self) {
        let ids: Vec<u32> = self.notes.borrow().iter().map(|n| n.id).collect();
        for id in ids {
            self.remove(id, 2);
        }
    }

    pub fn invoke(&self, id: u32, action: &str) {
        self.emit("ActionInvoked", &(id, action.to_string()));
        // Senza azione "default", il tocco porta in primo piano l'app che ha scritto.
        let note = self.notes.borrow().iter().find(|n| n.id == id).cloned();
        if let Some(note) = note {
            if action == "default" && !note.actions.iter().any(|(k, _)| k == "default") {
                let s = shell();
                let app = note
                    .desktop
                    .as_deref()
                    .and_then(|d| s.apps.borrow().get(d).cloned());
                if let Some(app) = app {
                    s.open_app(&app);
                }
            }
        }
        self.remove(id, 2);
    }

    fn changed(&self) {
        let s = shell();
        if let Some(bar) = s.bar.get() {
            bar.update_status();
        }
        if let Some(shade) = s.shade.get() {
            shade.refresh_notes();
        }
    }

    // ── Lanci (toast) ──

    fn toast(&self, note: &Note) {
        self.drop_toast(note.id);
        let max = shell().config.borrow().notifications.max_toasts.max(1);
        while self.toasts.borrow().len() >= max {
            let oldest = self.toasts.borrow().last().map(|(id, _)| *id);
            match oldest {
                Some(id) => self.drop_toast(id),
                None => break,
            }
        }
        let card = note_card(note, true);
        card.add_css_class("toast-card");
        self.toast_box.prepend(&card);
        self.toasts
            .borrow_mut()
            .insert(0, (note.id, card.clone().upcast()));
        self.toast_window.present();

        let timeout = if note.urgency >= 2 {
            None
        } else if note.timeout > 0 {
            Some(note.timeout as u64)
        } else {
            Some(shell().config.borrow().notifications.timeout_ms as u64)
        };
        if let Some(ms) = timeout {
            let id = note.id;
            glib::timeout_add_local_once(Duration::from_millis(ms), move || {
                if let Some(n) = shell().notifier.get() {
                    // Scaduta come lancio, ma resta nel Notiziario.
                    n.drop_toast(id);
                }
            });
        }
    }

    fn drop_toast(&self, id: u32) {
        let mut toasts = self.toasts.borrow_mut();
        if let Some(pos) = toasts.iter().position(|(i, _)| *i == id) {
            let (_, w) = toasts.remove(pos);
            self.toast_box.remove(&w);
        }
        if toasts.is_empty() {
            self.toast_window.set_visible(false);
        }
    }

    /// Ritira i lanci (le notifiche restano nel Notiziario).
    pub fn hide_toasts(&self) {
        self.clear_toasts();
    }

    fn clear_toasts(&self) {
        let ids: Vec<u32> = self.toasts.borrow().iter().map(|(i, _)| *i).collect();
        for id in ids {
            self.drop_toast(id);
        }
    }
}

fn note_icon(note: &Note) -> gtk::Widget {
    let s = shell();
    let from_app = note
        .desktop
        .as_deref()
        .and_then(|d| s.apps.borrow().get(d).map(|a| a.image(28)));
    if let Some(img) = from_app {
        return img.upcast();
    }
    let icon = note.icon.trim_start_matches("file://");
    if icon.starts_with('/') && std::path::Path::new(icon).exists() {
        let img = gtk::Image::from_file(icon);
        img.set_pixel_size(28);
        return img.upcast();
    }
    if !icon.is_empty() {
        let img = gtk::Image::from_icon_name(icon);
        img.set_pixel_size(28);
        img.add_css_class("app-icon");
        return img.upcast();
    }
    glyph::glyph(Glyph::Bell, 22).upcast()
}

/// Scheda di una notifica, usata sia come lancio sia nel Notiziario.
pub fn note_card(note: &Note, toast: bool) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
    card.add_css_class("note-card");
    if note.urgency >= 2 {
        card.add_css_class("critical");
    }
    card.set_size_request(if toast { 380 } else { -1 }, -1);

    let head = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    head.append(&note_icon(note));
    let kicker = text(
        &format!(
            "{} · {}",
            note.app.to_uppercase(),
            note.time.format("%H:%M")
        ),
        &["kicker"],
    );
    kicker.set_hexpand(true);
    kicker.set_xalign(0.0);
    kicker.set_ellipsize(gtk::pango::EllipsizeMode::End);
    head.append(&kicker);
    let close = glyph::button(Glyph::Close, 14, None);
    close.add_css_class("note-close");
    let id = note.id;
    close.connect_clicked(move |_| {
        if let Some(n) = shell().notifier.get() {
            n.remove(id, 2);
        }
    });
    head.append(&close);
    card.append(&head);

    let summary = text(&note.summary, &["serif", "note-summary"]);
    summary.set_xalign(0.0);
    summary.set_wrap(true);
    summary.set_max_width_chars(40);
    card.append(&summary);
    if !note.body.is_empty() {
        let body = text(&note.body, &["note-body"]);
        body.set_xalign(0.0);
        body.set_wrap(true);
        body.set_lines(if toast { 3 } else { 6 });
        body.set_ellipsize(gtk::pango::EllipsizeMode::End);
        body.set_max_width_chars(46);
        card.append(&body);
    }
    let visible_actions: Vec<_> = note
        .actions
        .iter()
        .filter(|(k, _)| k != "default")
        .collect();
    if !visible_actions.is_empty() {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        row.add_css_class("note-actions");
        for (key, label) in visible_actions {
            let b = gtk::Button::with_label(label);
            b.add_css_class("btn");
            b.add_css_class("chip");
            let key = key.clone();
            b.connect_clicked(move |_| {
                if let Some(n) = shell().notifier.get() {
                    n.invoke(id, &key);
                }
            });
            row.append(&b);
        }
        card.append(&row);
    }

    layer::on_tap(&card, move || {
        if let Some(n) = shell().notifier.get() {
            n.invoke(id, "default");
        }
    });
    // Swipe laterale: archivia.
    layer::on_swipe(&card, move |vx, vy| {
        if vx.abs() > 500.0 && vx.abs() > vy.abs() {
            if let Some(n) = shell().notifier.get() {
                n.remove(id, 2);
            }
        }
    });
    card
}

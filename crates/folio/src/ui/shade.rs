//! Il Notiziario: la tendina che scende dall'alto (come su un tablet Android,
//! notifiche e impostazioni rapide affiancate). Swipe verso l'alto o tocco
//! fuori dal pannello per chiudere.

use super::notify::note_card;
use super::{clear, rule, text};
use crate::glyph::{self, Glyph};
use crate::layer::{self, Keyboard, Level};
use crate::shell::{shell, Overlay};
use folio_core::{hypr, locale, sys};
use gtk::glib;
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

struct Toggle {
    root: gtk::Button,
    state: gtk::Label,
}

pub struct Shade {
    window: gtk::Window,
    revealer: gtk::Revealer,
    panel: gtk::Box,
    time: gtk::Label,
    date: gtk::Label,
    notes: gtk::Box,
    empty: gtk::Label,
    toggles: RefCell<Vec<(&'static str, Toggle)>>,
    brightness: gtk::Scale,
    volume: gtk::Scale,
    media: gtk::Box,
    media_title: gtk::Label,
    media_artist: gtk::Label,
    syncing: Cell<bool>,
    open: Cell<bool>,
    pending: RefCell<Option<glib::SourceId>>,
}

fn slider(g: Glyph, label: &str) -> (gtk::Box, gtk::Scale) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    row.add_css_class("qs-slider-row");
    row.append(&glyph::glyph(g, 20));
    let col = gtk::Box::new(gtk::Orientation::Vertical, 2);
    col.set_hexpand(true);
    let l = text(label, &["kicker"]);
    l.set_xalign(0.0);
    col.append(&l);
    let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 0.01);
    scale.set_draw_value(false);
    scale.add_css_class("qs-slider");
    scale.set_hexpand(true);
    col.append(&scale);
    row.append(&col);
    (row, scale)
}

impl Shade {
    pub fn new() -> Rc<Self> {
        let window = layer::surface("folio-shade", Level::Overlay, Keyboard::OnDemand);
        layer::fill_screen(&window);
        let scrim = gtk::Box::new(gtk::Orientation::Vertical, 0);
        scrim.add_css_class("shade-scrim");
        let revealer = gtk::Revealer::new();
        revealer.set_transition_type(gtk::RevealerTransitionType::SlideDown);
        revealer.set_transition_duration(280);
        revealer.set_valign(gtk::Align::Start);
        scrim.append(&revealer);
        let rest = gtk::Box::new(gtk::Orientation::Vertical, 0);
        rest.set_vexpand(true);
        scrim.append(&rest);
        window.set_child(Some(&scrim));

        let panel = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        panel.add_css_class("shade-panel");
        revealer.set_child(Some(&panel));

        // ── Colonna sinistra: data e notifiche ──
        let left = gtk::Box::new(gtk::Orientation::Vertical, 10);
        left.add_css_class("shade-left");
        left.set_hexpand(true);
        let time = text("", &["serif", "shade-time"]);
        time.set_xalign(0.0);
        left.append(&time);
        let date = text("", &["kicker", "shade-date"]);
        date.set_xalign(0.0);
        left.append(&date);
        left.append(&rule(false));
        let notes_head = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        notes_head.append(&text("Notifiche", &["serif", "section-title"]));
        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        notes_head.append(&spacer);
        let clear_btn = gtk::Button::with_label("Archivia tutto");
        clear_btn.add_css_class("btn");
        clear_btn.add_css_class("text-btn");
        clear_btn.connect_clicked(|_| {
            if let Some(n) = shell().notifier.get() {
                n.clear();
            }
        });
        notes_head.append(&clear_btn);
        left.append(&notes_head);
        let notes = gtk::Box::new(gtk::Orientation::Vertical, 10);
        let empty = text("Nessuna notizia. Buona lettura.", &["serif", "empty-note"]);
        empty.set_xalign(0.0);
        let notes_scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .min_content_height(320)
            .child(&notes)
            .build();
        left.append(&empty);
        left.append(&notes_scroll);
        panel.append(&left);
        panel.append(&rule(true));

        // ── Colonna destra: impostazioni rapide ──
        let right = gtk::Box::new(gtk::Orientation::Vertical, 14);
        right.add_css_class("shade-right");
        right.set_size_request(480, -1);
        let qs = gtk::Grid::new();
        qs.set_column_homogeneous(true);
        qs.set_row_spacing(8);
        qs.set_column_spacing(8);
        qs.add_css_class("qs-grid");
        right.append(&qs);

        let (b_row, brightness) = slider(Glyph::Sun, "LUMINOSITÀ");
        let (v_row, volume) = slider(Glyph::Volume, "VOLUME");
        if sys::brightness().is_some() {
            right.append(&b_row);
        }
        right.append(&v_row);

        let media = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        media.add_css_class("qs-media");
        let mt = gtk::Box::new(gtk::Orientation::Vertical, 2);
        mt.set_hexpand(true);
        let media_title = text("", &["serif", "media-title"]);
        media_title.set_xalign(0.0);
        media_title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        let media_artist = text("", &["kicker"]);
        media_artist.set_xalign(0.0);
        mt.append(&media_title);
        mt.append(&media_artist);
        media.append(&mt);
        for (g, act) in [
            (Glyph::Prev, "previous"),
            (Glyph::Play, "play-pause"),
            (Glyph::Next, "next"),
        ] {
            let b = glyph::button(g, 18, None);
            b.connect_clicked(move |_| sys::media(act));
            media.append(&b);
        }
        right.append(&media);

        right.append(&rule(false));
        let power = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        power.set_homogeneous(true);
        power.add_css_class("power-row");
        let s = shell();
        let cmds = s.config.borrow().commands.clone();
        for (g, label, cmd) in [
            (Glyph::Lock, "Blocca", cmds.lock.clone()),
            (Glyph::Sleep, "Sospendi", cmds.suspend.clone()),
            (Glyph::Logout, "Esci", cmds.logout.clone()),
            (Glyph::Restart, "Riavvia", cmds.reboot.clone()),
            (Glyph::Power, "Spegni", cmds.poweroff.clone()),
        ] {
            let b = glyph::button(g, 20, Some(label));
            b.add_css_class("power-btn");
            b.connect_clicked(move |_| {
                shell().close_overlays();
                let c = cmd.clone();
                // Lascia chiudere il pannello prima di bloccare o sospendere.
                glib::timeout_add_local_once(Duration::from_millis(250), move || sys::spawn(&c));
            });
            power.append(&b);
        }
        right.append(&power);
        panel.append(&right);

        let shade = Rc::new(Self {
            window,
            revealer,
            panel,
            time,
            date,
            notes,
            empty,
            toggles: RefCell::new(Vec::new()),
            brightness,
            volume,
            media,
            media_title,
            media_artist,
            syncing: Cell::new(false),
            open: Cell::new(false),
            pending: RefCell::new(None),
        });
        shade.build_toggles(&qs);

        // Cursori: si applica il valore con un piccolo ritardo (niente raffiche di processi).
        for (scale, which) in [(&shade.brightness, 0u8), (&shade.volume, 1u8)] {
            let w = Rc::downgrade(&shade);
            scale.connect_value_changed(move |sc| {
                let Some(sh) = w.upgrade() else { return };
                if sh.syncing.get() {
                    return;
                }
                if let Some(id) = sh.pending.borrow_mut().take() {
                    id.remove();
                }
                let v = sc.value();
                let id = glib::timeout_add_local_once(Duration::from_millis(70), move || {
                    if let Some(sh) = shell().shade.get() {
                        sh.pending.borrow_mut().take();
                    }
                    if which == 0 {
                        sys::set_brightness(v);
                    } else {
                        sys::set_volume(v);
                    }
                });
                *sh.pending.borrow_mut() = Some(id);
            });
        }

        // Chiusura: tocco sotto il pannello o swipe verso l'alto.
        layer::on_tap(&rest, || shell().command("shade hide"));
        layer::on_swipe(&shade.panel, |vx, vy| {
            if vy < -600.0 && vy.abs() > vx.abs() {
                shell().command("shade hide");
            }
        });
        let w = Rc::downgrade(&shade);
        layer::close_on_escape(&shade.window, move || {
            if let Some(s) = w.upgrade() {
                s.close();
            }
        });
        shade
    }

    fn build_toggles(&self, grid: &gtk::Grid) {
        let make = |g: Glyph, label: &str| -> Toggle {
            let b = gtk::Box::new(gtk::Orientation::Vertical, 6);
            let gl = glyph::glyph(g, 22);
            gl.set_halign(gtk::Align::Start);
            gl.set_margin_bottom(6);
            b.append(&gl);
            let l = text(label, &["qs-label"]);
            l.set_xalign(0.0);
            b.append(&l);
            let state = text("", &["kicker", "qs-state"]);
            state.set_xalign(0.0);
            state.set_ellipsize(gtk::pango::EllipsizeMode::End);
            b.append(&state);
            let root = gtk::Button::builder().child(&b).build();
            root.add_css_class("btn");
            root.add_css_class("qs-tile");
            Toggle { root, state }
        };
        let defs: [(&'static str, Glyph, &str); 9] = [
            ("wifi", Glyph::Wifi, "Wi-Fi"),
            ("bt", Glyph::Bluetooth, "Bluetooth"),
            ("dnd", Glyph::BellOff, "Non disturbare"),
            ("dark", Glyph::Moon, "Edizione serale"),
            ("osk", Glyph::Keyboard, "Tastiera"),
            ("rotate", Glyph::Rotate, "Ruota schermo"),
            ("layout", Glyph::Layout, "Impagina"),
            ("shot", Glyph::Camera, "Istantanea"),
            ("dock", Glyph::Dock, "Dock"),
        ];
        let mut toggles = self.toggles.borrow_mut();
        for (i, (key, g, label)) in defs.into_iter().enumerate() {
            let t = make(g, label);
            t.root.connect_clicked(move |_| on_toggle(key));
            grid.attach(&t.root, (i % 3) as i32, (i / 3) as i32, 1, 1);
            toggles.push((key, t));
        }
    }

    /// Allinea interruttori e cursori allo stato del sistema.
    pub fn sync(&self) {
        let s = shell();
        let sys = s.sys.borrow().clone();
        let dnd = s.notifier.get().map(|n| n.dnd()).unwrap_or(false);
        let dark = s.is_dark();
        for (key, t) in self.toggles.borrow().iter() {
            let (on, state): (bool, String) = match *key {
                "wifi" => match &sys.wifi {
                    Some(w) => (
                        w.enabled,
                        if w.enabled {
                            w.ssid.clone().unwrap_or_else(|| "Non connesso".into())
                        } else {
                            "Spento".into()
                        },
                    ),
                    None => (false, "Non disponibile".into()),
                },
                "bt" => match sys.bluetooth {
                    Some(on) => (on, if on { "Acceso".into() } else { "Spento".into() }),
                    None => (false, "Non disponibile".into()),
                },
                "dnd" => (
                    dnd,
                    if dnd {
                        "Silenzio".into()
                    } else {
                        "Spento".into()
                    },
                ),
                "dark" => (
                    dark,
                    if dark {
                        "Inchiostro".into()
                    } else {
                        "Carta".into()
                    },
                ),
                "osk" => (false, "Mostra / nascondi".into()),
                "rotate" => (false, rotation_label()),
                "layout" => (false, "Sposta gli elementi".into()),
                "shot" => (false, "Tutto lo schermo".into()),
                "dock" => (false, "Mostra / nascondi".into()),
                _ => (false, String::new()),
            };
            if on {
                t.root.add_css_class("on");
            } else {
                t.root.remove_css_class("on");
            }
            t.state.set_text(&state.to_uppercase());
        }

        self.syncing.set(true);
        if let Some(b) = sys.brightness {
            if (self.brightness.value() - b).abs() > 0.02 && self.pending.borrow().is_none() {
                self.brightness.set_value(b);
            }
        }
        if let Some(v) = &sys.volume {
            if (self.volume.value() - v.level).abs() > 0.02 && self.pending.borrow().is_none() {
                self.volume.set_value(v.level.min(1.0));
            }
        }
        self.syncing.set(false);

        match &sys.music {
            Some(m) => {
                self.media.set_visible(true);
                self.media_title.set_text(&m.title);
                self.media_artist.set_text(&m.artist.to_uppercase());
            }
            None => self.media.set_visible(false),
        }
        let now = chrono::Local::now();
        self.time.set_text(&now.format("%H:%M").to_string());
        self.date.set_text(
            &format!("{} · {}", locale::long_date(&now), locale::edition(&now)).to_uppercase(),
        );
    }

    pub fn refresh_notes(&self) {
        clear(&self.notes);
        let list = shell()
            .notifier
            .get()
            .map(|n| n.notes())
            .unwrap_or_default();
        self.empty.set_visible(list.is_empty());
        for note in list {
            self.notes.append(&note_card(&note, false));
        }
    }
}

fn rotation_label() -> String {
    let s = shell();
    let t = s
        .hypr
        .borrow()
        .focused_monitor()
        .map(|m| m.transform)
        .unwrap_or(0);
    match t % 4 {
        0 => "Orizzontale".into(),
        1 => "Verticale".into(),
        2 => "Capovolto".into(),
        _ => "Verticale inverso".into(),
    }
}

fn on_toggle(key: &str) {
    let s = shell();
    let sys_snap = s.sys.borrow().clone();
    match key {
        "wifi" => sys::set_wifi(!sys_snap.wifi.map(|w| w.enabled).unwrap_or(false)),
        "bt" => sys::set_bluetooth(!sys_snap.bluetooth.unwrap_or(false)),
        "dnd" => {
            if let Some(n) = s.notifier.get() {
                n.set_dnd(!n.dnd());
            }
        }
        "dark" => s.set_theme("toggle"),
        "osk" => s.osk.toggle(),
        "rotate" => rotate(),
        "layout" => {
            s.close_overlays();
            s.command("layout show");
        }
        "shot" => {
            s.close_overlays();
            let cmd = s.config.borrow().commands.screenshot.clone();
            glib::timeout_add_local_once(Duration::from_millis(400), move || sys::spawn(&cmd));
        }
        "dock" => s.command("dock toggle"),
        _ => {}
    }
    if let Some(sh) = s.shade.get() {
        // I comandi di sistema impiegano un attimo: si risincronizza poco dopo.
        sh.sync();
        glib::timeout_add_local_once(Duration::from_millis(700), || {
            if let Some(sh) = shell().shade.get() {
                sh.sync();
            }
        });
    }
}

/// Ruota schermo e touchscreen insieme (Hyprland ≥ 0.55, configurazione Lua).
pub fn rotate() {
    let s = shell();
    let Some(m) = s.hypr.borrow().focused_monitor().cloned() else {
        return;
    };
    let next = (m.transform + 1) % 4;
    let lua = format!(
        "hl.monitor({{ output = {}, mode = \"preferred\", position = \"auto\", scale = {}, transform = {} }}) \
         hl.config({{ input = {{ touchdevice = {{ transform = {} }} }} }})",
        hypr::lua_str(&m.name),
        if m.scale > 0.0 { format!("{:.2}", m.scale) } else { "\"auto\"".into() },
        next,
        next
    );
    if !hypr::eval(&lua) {
        log::warn!("rotazione non riuscita (richiede Hyprland ≥ 0.55)");
    }
    s.refresh_hypr();
}

impl Overlay for Shade {
    fn is_open(&self) -> bool {
        self.open.get()
    }

    fn open(&self) {
        if self.open.replace(true) {
            return;
        }
        if let Some(n) = shell().notifier.get() {
            n.hide_toasts();
        }
        self.sync();
        self.refresh_notes();
        layer::on_monitor(&self.window);
        self.window.present();
        self.revealer.set_reveal_child(true);
    }

    fn close(&self) {
        if !self.open.replace(false) {
            return;
        }
        self.revealer.set_reveal_child(false);
        let w = self.window.downgrade();
        glib::timeout_add_local_once(Duration::from_millis(300), move || {
            let closed = shell().shade.get().map(|s| !s.open.get()).unwrap_or(true);
            if let (Some(w), true) = (w.upgrade(), closed) {
                w.set_visible(false);
            }
        });
    }
}

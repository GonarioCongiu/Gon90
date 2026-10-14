//! La Testata: barra di stato in stile masthead. Orizzontale sui bordi alto e
//! basso, verticale su quelli laterali, libera ovunque in modalità Impaginazione.
//!
//! Tocchi: testata → Indice · numeri → spazi di lavoro · titolo → comandi
//! finestra · stato e ora → Notiziario. Trascinare verso il basso apre il
//! Notiziario come la tendina di Android. Tenere premuto → Impaginazione.

use super::{clear, rule, text, Placeable};
use crate::glyph::{self, Glyph};
use crate::layer::{self, Keyboard, Level};
use crate::shell::shell;
use folio_core::hypr::{self, Dispatch};
use folio_core::locale;
use folio_core::state::{Edge, Placement};
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub struct Bar {
    window: gtk::Window,
    root: gtk::Box,
    workspaces: RefCell<Option<gtk::Box>>,
    title: RefCell<Option<gtk::Label>>,
    clock: RefCell<Option<gtk::Label>>,
    date: RefCell<Option<gtk::Label>>,
    status: RefCell<Option<gtk::Box>>,
    placement: Cell<Placement>,
    ws_signature: RefCell<String>,
}

const DEFAULT: Placement = Placement::docked(Edge::Top, true);

impl Bar {
    pub fn new() -> Rc<Self> {
        let window = layer::surface("folio-bar", Level::Top, Keyboard::None);
        let root = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        root.add_css_class("bar");
        window.set_child(Some(&root));
        let placement = shell().state.borrow().bar.unwrap_or(DEFAULT);
        let bar = Rc::new(Self {
            window,
            root,
            workspaces: RefCell::new(None),
            title: RefCell::new(None),
            clock: RefCell::new(None),
            date: RefCell::new(None),
            status: RefCell::new(None),
            placement: Cell::new(placement),
            ws_signature: RefCell::new(String::new()),
        });

        // Tenere premuto un pannello apre l'Impaginazione.
        layer::on_long_press(&bar.root, |_, _| shell().command("layout show"));
        // Trascinare la testata verso il centro dello schermo apre il Notiziario.
        let weak = Rc::downgrade(&bar);
        layer::on_swipe(&bar.root, move |vx, vy| {
            let Some(bar) = weak.upgrade() else { return };
            let p = bar.placement.get();
            let inward = match p.edge {
                Edge::Top => vy > 350.0,
                Edge::Bottom => vy < -350.0,
                Edge::Left => vx > 350.0,
                Edge::Right => vx < -350.0,
                Edge::Float => vy > 350.0,
            };
            if inward {
                shell().command("shade show");
            }
        });

        bar.rebuild();
        layer::on_monitor(&bar.window);
        bar.window.present();
        bar
    }

    fn vertical(&self) -> bool {
        self.placement.get().is_vertical()
    }

    pub fn rebuild(&self) {
        clear(&self.root);
        let vertical = self.vertical();
        let cfg = shell().config.borrow().bar.clone();
        let orientation = if vertical {
            gtk::Orientation::Vertical
        } else {
            gtk::Orientation::Horizontal
        };
        self.root.set_orientation(orientation);
        self.root
            .set_css_classes(&["bar", if vertical { "vertical" } else { "horizontal" }]);
        if self.placement.get().edge == Edge::Float || !self.placement.get().stretch {
            self.root.add_css_class("floating");
        }
        if vertical {
            self.root.set_size_request(cfg.size + 12, -1);
        } else {
            self.root.set_size_request(-1, cfg.size);
        }

        let center = gtk::CenterBox::new();
        center.set_orientation(orientation);
        center.set_hexpand(true);
        center.set_vexpand(true);
        let align = |w: &gtk::Widget| {
            if vertical {
                w.set_halign(gtk::Align::Center);
            } else {
                w.set_valign(gtk::Align::Center);
            }
        };

        // ── Inizio: testata e spazi di lavoro ──
        let start = gtk::Box::new(orientation, if vertical { 10 } else { 18 });
        let masthead_text = if vertical {
            cfg.masthead
                .chars()
                .next()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "F".into())
        } else {
            cfg.masthead.clone()
        };
        let masthead = text(&masthead_text, &["serif", "masthead"]);
        let mh = gtk::Button::builder().child(&masthead).build();
        mh.add_css_class("btn");
        mh.connect_clicked(|_| shell().command("home toggle"));
        start.append(&mh);
        start.append(&rule(!vertical));
        let ws = gtk::Box::new(orientation, if vertical { 2 } else { 4 });
        ws.add_css_class("ws-strip");
        start.append(&ws);
        align(start.upcast_ref());
        align(ws.upcast_ref());
        *self.workspaces.borrow_mut() = Some(ws);
        self.ws_signature.borrow_mut().clear();
        center.set_start_widget(Some(&start));

        // ── Centro: il titolo della finestra attiva, come il titolo di un articolo ──
        if !vertical && cfg.show_title {
            let title = text("", &["serif", "bar-title"]);
            title.set_ellipsize(gtk::pango::EllipsizeMode::End);
            title.set_max_width_chars(60);
            let tb = gtk::Button::builder().child(&title).build();
            tb.add_css_class("btn");
            tb.connect_clicked(|_| shell().command("chrome toggle"));
            align(tb.upcast_ref());
            center.set_center_widget(Some(&tb));
            *self.title.borrow_mut() = Some(title);
        } else {
            *self.title.borrow_mut() = None;
        }

        // ── Fine: stato, data, ora → Notiziario ──
        let end = gtk::Box::new(orientation, if vertical { 8 } else { 14 });
        let status = gtk::Box::new(orientation, if vertical { 8 } else { 12 });
        status.add_css_class("status");
        end.append(&status);
        *self.status.borrow_mut() = Some(status);
        if !vertical {
            end.append(&rule(true));
            let date = text("", &["kicker", "bar-date"]);
            end.append(&date);
            *self.date.borrow_mut() = Some(date);
        } else {
            *self.date.borrow_mut() = None;
        }
        let clock = text("", &["mono", "bar-clock"]);
        if vertical {
            clock.set_justify(gtk::Justification::Center);
        }
        end.append(&clock);
        *self.clock.borrow_mut() = Some(clock);
        align(end.upcast_ref());
        let eb = gtk::Button::builder().child(&end).build();
        eb.add_css_class("btn");
        eb.add_css_class("bar-end");
        eb.connect_clicked(|_| shell().command("shade toggle"));
        center.set_end_widget(Some(&eb));

        self.root.append(&center);
        self.update_clock();
        self.update_status();
        self.update_hypr();
        self.apply_placement();
    }

    fn apply_placement(&self) {
        let p = self.placement.get();
        let margin = shell().config.borrow().bar.margin;
        let size = layer::natural_size(&self.window);
        let cover_refresh = || {
            gtk::glib::idle_add_local_once(|| {
                if let Some(c) = shell().cover.get() {
                    c.update();
                }
            });
        };
        cover_refresh();
        layer::apply_placement(&self.window, &p, size, margin, true);
    }

    pub fn update_clock(&self) {
        let now = chrono::Local::now();
        let seconds = shell().config.borrow().bar.show_seconds;
        if let Some(c) = self.clock.borrow().as_ref() {
            let t = if self.vertical() {
                now.format("%H\n%M").to_string()
            } else if seconds {
                now.format("%H:%M:%S").to_string()
            } else {
                now.format("%H:%M").to_string()
            };
            if c.text() != t {
                c.set_text(&t);
            }
        }
        if let Some(d) = self.date.borrow().as_ref() {
            let t = format!(
                "{} {} {}",
                locale::weekday_short(&now),
                now.format("%d"),
                locale::month(&now)
                    .chars()
                    .take(3)
                    .collect::<String>()
                    .to_uppercase()
            );
            if d.text() != t {
                d.set_text(&t);
            }
        }
    }

    /// Stato in forma tipografica: niente icone colorate, solo glifi sottili e cifre.
    pub fn update_status(&self) {
        let Some(status) = self.status.borrow().clone() else {
            return;
        };
        clear(&status);
        let s = shell();
        let sys = s.sys.borrow();
        let vertical = self.vertical();
        let item = |g: Glyph, value: Option<String>| {
            let b = gtk::Box::new(gtk::Orientation::Horizontal, 5);
            b.add_css_class("status-item");
            b.append(&glyph::glyph(g, 15));
            if let Some(v) = value.filter(|_| !vertical) {
                b.append(&text(&v, &["mono", "status-value"]));
            }
            b
        };
        if let Some(w) = &sys.wifi {
            if w.enabled {
                let wifi = item(
                    Glyph::Wifi,
                    w.ssid.clone().map(|s| s.chars().take(14).collect()),
                );
                if w.ssid.is_none() {
                    wifi.add_css_class("dim");
                }
                status.append(&wifi);
            }
        }
        if let Some(true) = sys.bluetooth {
            status.append(&item(Glyph::Bluetooth, None));
        }
        if let Some(v) = &sys.volume {
            let g = if v.muted {
                Glyph::VolumeMute
            } else {
                Glyph::Volume
            };
            status.append(&item(g, Some(format!("{:.0}", v.level * 100.0))));
        }
        if let Some(b) = &sys.battery {
            let it = item(Glyph::Battery, Some(format!("{}%", b.percent)));
            if b.percent <= 15 && !b.charging {
                it.add_css_class("alert");
            }
            if b.charging {
                it.add_css_class("charging");
            }
            status.append(&it);
        }
        let unread = s.notifier.get().map(|n| n.count()).unwrap_or(0);
        if unread > 0 {
            let it = item(Glyph::Bell, Some(unread.to_string()));
            it.add_css_class("accent");
            status.append(&it);
        }
    }

    pub fn update_hypr(&self) {
        let s = shell();
        let snap = s.hypr.borrow();
        if let Some(t) = self.title.borrow().as_ref() {
            let title = snap
                .active
                .as_ref()
                .map(|a| a.title.clone())
                .unwrap_or_default();
            if t.text() != title {
                t.set_text(&title);
            }
        }
        let Some(ws_box) = self.workspaces.borrow().clone() else {
            return;
        };
        let active = snap.active_workspace();
        let list: Vec<(i64, u32)> = snap
            .normal_workspaces()
            .iter()
            .map(|w| (w.id, w.windows))
            .collect();
        let signature = format!("{active}|{list:?}");
        if *self.ws_signature.borrow() == signature {
            return;
        }
        *self.ws_signature.borrow_mut() = signature;
        drop(snap);
        clear(&ws_box);
        for (i, (id, windows)) in list.iter().enumerate() {
            let l = text(&locale::folio_number(i + 1), &["mono", "ws"]);
            if *id == active {
                l.add_css_class("active");
            }
            if *windows == 0 {
                l.add_css_class("empty");
            }
            let b = gtk::Button::builder().child(&l).build();
            b.add_css_class("btn");
            let id = *id;
            b.connect_clicked(move |_| {
                hypr::dispatch(&Dispatch::Workspace(id.to_string()));
            });
            ws_box.append(&b);
        }
        // Spazi dinamici come in GNOME: ce n'è sempre uno nuovo a portata di dito.
        let plus = glyph::button(Glyph::Plus, 14, None);
        plus.add_css_class("ws-new");
        plus.connect_clicked(|_| {
            let next = shell().hypr.borrow().next_free_workspace();
            hypr::dispatch(&Dispatch::Workspace(next.to_string()));
        });
        ws_box.append(&plus);
    }
}

impl Placeable for Bar {
    fn label(&self) -> String {
        "Testata".into()
    }

    fn placement(&self) -> Placement {
        self.placement.get()
    }

    fn set_placement(&self, p: Placement) {
        self.placement.set(p);
        {
            let s = shell();
            s.state.borrow_mut().bar = Some(p);
            s.save_state();
        }
        // L'orientamento può cambiare (bordo laterale ↔ orizzontale): si reimpagina.
        self.rebuild();
    }

    fn size(&self) -> (i32, i32) {
        layer::natural_size(&self.window)
    }

    fn margin(&self) -> i32 {
        shell().config.borrow().bar.margin
    }

    fn reset(&self) {
        self.set_placement(DEFAULT);
    }

    fn can_stretch(&self) -> bool {
        true
    }
}

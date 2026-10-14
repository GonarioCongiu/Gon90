//! La Dock: app fissate e in esecuzione, alla macOS/GNOME. Ogni icona è anche
//! un micro-widget (il calendario mostra il giorno, il meteo la temperatura,
//! il terminale quante finestre ha aperte…).
//!
//! Modalità `persistent` (taskbar dei tablet Android: riserva spazio) oppure
//! `transient` (compare con lo swipe corto dal bordo inferiore e si nasconde).

use super::{clear, rule, text, Placeable};
use crate::apps::App;
use crate::glyph::{self, Glyph};
use crate::layer::{self, Keyboard, Level};
use crate::shell::shell;
use folio_core::config::{DockMode, TileSize, TileSpec, TileStyle};
use folio_core::hypr::{self, Dispatch};
use folio_core::live::{self, LiveCtx};
use folio_core::state::{Edge, Placement};
use gtk::glib;
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

pub struct Dock {
    window: gtk::Window,
    revealer: gtk::Revealer,
    root: gtk::Box,
    placement: Cell<Placement>,
    hide_timer: RefCell<Option<glib::SourceId>>,
    signature: RefCell<String>,
}

const DEFAULT: Placement = Placement::docked(Edge::Bottom, false);

impl Dock {
    pub fn new() -> Rc<Self> {
        let window = layer::surface("folio-dock", Level::Top, Keyboard::None);
        let revealer = gtk::Revealer::new();
        revealer.set_transition_duration(260);
        let root = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        root.add_css_class("dock");
        revealer.set_child(Some(&root));
        window.set_child(Some(&revealer));
        let placement = shell().state.borrow().dock.unwrap_or(DEFAULT);
        let dock = Rc::new(Self {
            window,
            revealer,
            root,
            placement: Cell::new(placement),
            hide_timer: RefCell::new(None),
            signature: RefCell::new(String::new()),
        });

        // Tenere premuto un pannello apre l'Impaginazione.
        layer::on_long_press(&dock.root, |_, _| shell().command("layout show"));
        // Qualunque interazione rimanda l'autohide.
        let motion = gtk::EventControllerMotion::new();
        let weak = Rc::downgrade(&dock);
        motion.connect_motion(move |_, _, _| {
            if let Some(d) = weak.upgrade() {
                d.autohide_later();
            }
        });
        dock.root.add_controller(motion);

        dock.rebuild();
        layer::on_monitor(&dock.window);
        if dock.mode() == DockMode::Persistent {
            dock.revealer.set_reveal_child(true);
            dock.window.present();
        }
        dock
    }

    fn mode(&self) -> DockMode {
        shell().config.borrow().dock.mode
    }

    fn vertical(&self) -> bool {
        self.placement.get().is_vertical()
    }

    fn pinned_ids(&self) -> Vec<String> {
        let s = shell();
        let st = s.state.borrow();
        let cfg = s.config.borrow();
        st.dock_pinned
            .clone()
            .unwrap_or_else(|| cfg.dock.pinned.clone())
    }

    /// App fissate (risolte tra i candidati) + app aperte non fissate.
    fn entries(&self) -> Vec<(App, bool)> {
        let s = shell();
        let catalog = s.apps.borrow();
        let mut out: Vec<(App, bool)> = Vec::new();
        for spec in self.pinned_ids() {
            if let Some(app) = catalog.resolve(&spec) {
                if !out.iter().any(|(a, _)| a.id == app.id) {
                    out.push((app.clone(), true));
                }
            }
        }
        if s.config.borrow().dock.show_running {
            let snap = s.hypr.borrow();
            let mut clients: Vec<_> = snap.clients.iter().filter(|c| c.mapped).collect();
            clients.sort_by_key(|c| c.pid);
            for c in clients {
                if let Some(app) = catalog.for_class(c.app_class()) {
                    if !out.iter().any(|(a, _)| a.id == app.id) {
                        out.push((app.clone(), false));
                    }
                }
            }
        }
        out
    }

    pub fn update_running(&self) {
        // Ricostruisce solo se cambia qualcosa di visibile.
        let s = shell();
        let active = s.active_client().map(|c| c.address).unwrap_or_default();
        let sig: String = self
            .entries()
            .iter()
            .map(|(a, p)| format!("{}:{}:{}", a.id, p, s.windows_of(a).len()))
            .collect::<Vec<_>>()
            .join(",");
        let sig = format!("{sig}|{active}|{}", self.vertical());
        if *self.signature.borrow() == sig {
            return;
        }
        *self.signature.borrow_mut() = sig;
        self.populate();
    }

    pub fn rebuild(&self) {
        self.signature.borrow_mut().clear();
        let vertical = self.vertical();
        let orientation = if vertical {
            gtk::Orientation::Vertical
        } else {
            gtk::Orientation::Horizontal
        };
        self.root.set_orientation(orientation);
        self.root
            .set_css_classes(&["dock", if vertical { "vertical" } else { "horizontal" }]);
        self.revealer
            .set_transition_type(match self.placement.get().edge {
                Edge::Top => gtk::RevealerTransitionType::SlideDown,
                Edge::Left => gtk::RevealerTransitionType::SlideRight,
                Edge::Right => gtk::RevealerTransitionType::SlideLeft,
                Edge::Bottom => gtk::RevealerTransitionType::SlideUp,
                Edge::Float => gtk::RevealerTransitionType::Crossfade,
            });
        self.populate();
        self.update_running();
        self.apply_placement();
    }

    fn apply_placement(&self) {
        let p = self.placement.get();
        let s = shell();
        let cfg = s.config.borrow().dock.clone();
        let size = layer::natural_size(&self.window);
        // Il margine visivo è già nel CSS della dock (serve all'ombra): qui nessun margine extra.
        let _ = cfg.margin;
        let cover_refresh = || {
            gtk::glib::idle_add_local_once(|| {
                if let Some(c) = shell().cover.get() {
                    c.update();
                }
            });
        };
        cover_refresh();
        layer::apply_placement(&self.window, &p, size, 0, cfg.mode == DockMode::Persistent);
    }

    fn populate(&self) {
        clear(&self.root);
        let s = shell();
        let icon = (s.config.borrow().dock.icon_size as f64 * s.config.borrow().theme.scale) as i32;
        let active_app = s.active_client().and_then(|c| {
            s.apps
                .borrow()
                .for_class(c.app_class())
                .map(|a| a.id.clone())
        });
        let entries = self.entries();
        let mut separator_done = false;
        for (app, pinned) in &entries {
            if !pinned && !separator_done {
                self.root.append(&rule(!self.vertical()));
                separator_done = true;
            }
            let item = dock_item(
                app,
                *pinned,
                icon,
                active_app.as_deref() == Some(app.id.as_str()),
            );
            self.root.append(&item);
        }
        if !entries.is_empty() {
            self.root.append(&rule(!self.vertical()));
        }
        let center = |w: &gtk::Button| {
            if self.vertical() {
                w.set_halign(gtk::Align::Center);
            } else {
                w.set_valign(gtk::Align::Center);
            }
        };
        let grid = glyph::button(Glyph::Grid, (icon as f64 * 0.42) as i32, None);
        center(&grid);
        grid.add_css_class("dock-action");
        grid.set_tooltip_text(Some("Indice"));
        grid.connect_clicked(|_| shell().command("home toggle"));
        self.root.append(&grid);
        let pages = glyph::button(Glyph::Pages, (icon as f64 * 0.42) as i32, None);
        center(&pages);
        pages.add_css_class("dock-action");
        pages.set_tooltip_text(Some("Sommario"));
        pages.connect_clicked(|_| shell().command("overview toggle"));
        self.root.append(&pages);
    }

    // ── Visibilità (modalità transitoria) ──

    pub fn reveal(&self) {
        self.window.present();
        self.revealer.set_reveal_child(true);
        self.autohide_later();
    }

    pub fn conceal(&self) {
        if self.mode() == DockMode::Persistent {
            return;
        }
        self.revealer.set_reveal_child(false);
        let w = self.window.downgrade();
        glib::timeout_add_local_once(Duration::from_millis(280), move || {
            if let Some(w) = w.upgrade() {
                if !shell()
                    .dock
                    .get()
                    .map(|d| d.revealer.reveals_child())
                    .unwrap_or(true)
                {
                    w.set_visible(false);
                }
            }
        });
    }

    pub fn toggle_reveal(&self) {
        if self.revealer.reveals_child() && self.window.is_visible() {
            self.conceal();
        } else {
            self.reveal();
        }
    }

    fn autohide_later(&self) {
        if self.mode() == DockMode::Persistent {
            return;
        }
        if let Some(id) = self.hide_timer.borrow_mut().take() {
            id.remove();
        }
        let secs = shell().config.borrow().dock.autohide_secs.max(1);
        let id = glib::timeout_add_seconds_local_once(secs, || {
            if let Some(d) = shell().dock.get() {
                d.hide_timer.borrow_mut().take();
                d.conceal();
            }
        });
        *self.hide_timer.borrow_mut() = Some(id);
    }

    pub fn autohide_soon(&self) {
        if self.mode() == DockMode::Transient {
            self.conceal();
        }
    }

    pub fn pin(&self, app_id: &str, pinned: bool) {
        let s = shell();
        let mut ids = self.pinned_ids();
        // Gli ID "a|b|c" vengono risolti e sostituiti dall'app effettiva.
        let catalog = s.apps.borrow();
        ids.retain(|spec| {
            catalog
                .resolve(spec)
                .map(|a| a.id != app_id)
                .unwrap_or(true)
        });
        if pinned {
            ids.push(app_id.to_string());
        }
        drop(catalog);
        s.state.borrow_mut().dock_pinned = Some(ids);
        s.save_state();
        self.rebuild();
    }

    pub fn is_pinned(&self, app_id: &str) -> bool {
        let s = shell();
        let catalog = s.apps.borrow();
        self.pinned_ids().iter().any(|spec| {
            catalog
                .resolve(spec)
                .map(|a| a.id == app_id)
                .unwrap_or(false)
        })
    }
}

fn dock_item(app: &App, pinned: bool, icon: i32, active: bool) -> gtk::Widget {
    let s = shell();
    let windows = s.windows_of(app);
    let overlay = gtk::Overlay::new();
    overlay.add_css_class("dock-item");
    if active {
        overlay.add_css_class("active");
    }
    if !pinned {
        overlay.add_css_class("unpinned");
    }
    let img = app.image(icon);
    overlay.set_child(Some(&img));
    overlay.set_tooltip_text(Some(&app.name));

    // Micro-widget: un distintivo con il dato vivo dell'app.
    let kind = live::guess(&app.id, &app.categories);
    let badge = text("", &["mono", "dock-badge"]);
    badge.set_halign(gtk::Align::End);
    badge.set_valign(gtk::Align::Start);
    badge.set_visible(false);
    overlay.add_overlay(&badge);
    let weak_badge = badge.downgrade();
    let app_c = app.clone();
    let refresh = move || -> bool {
        let Some(badge) = weak_badge.upgrade() else {
            return false;
        };
        let s = shell();
        let sys = s.sys.borrow();
        let ctx = LiveCtx {
            now: chrono::Local::now(),
            sys: &sys,
            windows: s.windows_of(&app_c).len(),
            app_name: &app_c.name,
            description: "",
            notes: None,
            seconds: false,
        };
        let content = live::compute(kind, &ctx);
        let b = content
            .badge
            .filter(|b| b.chars().count() <= 3 && kind != live::LiveKind::Plain);
        match b {
            Some(t) => {
                badge.set_text(&t);
                badge.set_visible(true);
            }
            None => badge.set_visible(false),
        }
        true
    };
    refresh();
    s.live.subscribe(kind, refresh);

    // Indicatori di esecuzione: un punto per finestra, fino a tre.
    if !windows.is_empty() {
        let dots = gtk::Box::new(gtk::Orientation::Horizontal, 3);
        dots.add_css_class("run-dots");
        dots.set_halign(gtk::Align::Center);
        dots.set_valign(gtk::Align::End);
        for _ in 0..windows.len().min(3) {
            let d = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            d.add_css_class("run-dot");
            dots.append(&d);
        }
        overlay.add_overlay(&dots);
    }

    let app_tap = app.clone();
    layer::on_tap(&overlay, move || shell().open_app(&app_tap));
    let app_menu = app.clone();
    let weak_overlay = overlay.downgrade();
    layer::on_long_press(&overlay, move |_, _| {
        if let Some(o) = weak_overlay.upgrade() {
            app_menu_popover(&o, &app_menu);
        }
    });
    overlay.upcast()
}

/// Menu contestuale (tenere premuto su un'icona della dock).
pub fn app_menu_popover(anchor: &impl IsA<gtk::Widget>, app: &App) {
    let s = shell();
    let pop = gtk::Popover::new();
    pop.set_has_arrow(false);
    pop.add_css_class("folio-menu");
    let col = gtk::Box::new(gtk::Orientation::Vertical, 0);
    col.append(&text(&app.name, &["serif", "menu-title"]));
    if !app.description.is_empty() {
        let d = text(&app.description, &["caption", "menu-deck"]);
        d.set_wrap(true);
        d.set_max_width_chars(32);
        d.set_xalign(0.0);
        col.append(&d);
    }
    col.append(&rule(false));

    let entry = |label: &str, g: Glyph, f: Box<dyn Fn()>| -> gtk::Button {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        row.append(&glyph::glyph(g, 18));
        row.append(&text(label, &["menu-label"]));
        let b = gtk::Button::builder().child(&row).build();
        b.add_css_class("btn");
        b.add_css_class("menu-item");
        let popw = pop.downgrade();
        b.connect_clicked(move |_| {
            if let Some(p) = popw.upgrade() {
                p.popdown();
            }
            f();
        });
        b
    };

    for w in s.windows_of(app).into_iter().take(6) {
        let title: String = if w.title.is_empty() {
            app.name.clone()
        } else {
            w.title.clone()
        };
        let short: String = title.chars().take(38).collect();
        let g = if w.is_minimized() {
            Glyph::Minimize
        } else {
            Glyph::Maximize
        };
        col.append(&entry(
            &short,
            g,
            Box::new(move || shell().focus_window(&w)),
        ));
    }
    let a = app.clone();
    col.append(&entry(
        "Nuova finestra",
        Glyph::Plus,
        Box::new(move || a.launch()),
    ));
    let pinned = s.dock.get().map(|d| d.is_pinned(&app.id)).unwrap_or(false);
    let id = app.id.clone();
    col.append(&entry(
        if pinned {
            "Rimuovi dalla dock"
        } else {
            "Fissa nella dock"
        },
        Glyph::Pin,
        Box::new(move || {
            if let Some(d) = shell().dock.get() {
                d.pin(&id, !pinned);
            }
        }),
    ));
    let id2 = app.id.clone();
    col.append(&entry(
        "Aggiungi all'Indice",
        Glyph::Grid,
        Box::new(move || {
            if let Some(h) = shell().home.get() {
                h.add_tile(TileSpec {
                    app: Some(id2.clone()),
                    live: None,
                    size: TileSize::Medium,
                    style: TileStyle::Paper,
                    label: None,
                });
            }
        }),
    ));
    let id3 = app.id.clone();
    col.append(&entry(
        "Metti sulla scrivania",
        Glyph::Desk,
        Box::new(move || {
            if let Some(d) = shell().desk.get() {
                d.add(TileSpec {
                    app: Some(id3.clone()),
                    live: None,
                    size: TileSize::Medium,
                    style: TileStyle::Paper,
                    label: None,
                });
            }
        }),
    ));
    if !s.windows_of(app).is_empty() {
        let a2 = app.clone();
        let close = entry(
            "Chiudi tutte le finestre",
            Glyph::Close,
            Box::new(move || {
                for w in shell().windows_of(&a2) {
                    hypr::dispatch(&Dispatch::Close(w.address.clone()));
                }
            }),
        );
        close.add_css_class("danger");
        col.append(&close);
    }
    pop.set_child(Some(&col));
    pop.set_parent(anchor.as_ref());
    pop.connect_closed(|p| {
        let p = p.clone();
        glib::idle_add_local_once(move || p.unparent());
    });
    pop.popup();
}

impl Placeable for Dock {
    fn label(&self) -> String {
        "Dock".into()
    }

    fn placement(&self) -> Placement {
        self.placement.get()
    }

    fn set_placement(&self, p: Placement) {
        self.placement.set(p);
        let s = shell();
        s.state.borrow_mut().dock = Some(p);
        s.save_state();
        self.rebuild();
    }

    fn size(&self) -> (i32, i32) {
        layer::natural_size(&self.window)
    }

    fn margin(&self) -> i32 {
        0
    }

    fn reset(&self) {
        self.set_placement(DEFAULT);
    }
}

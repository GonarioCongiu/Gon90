//! Comandi della finestra: il popup che la barra del titolo rivela.
//!
//! Ogni finestra ha una barra del titolo sottile (hyprbars, configurata da
//! Folio) con un solo pulsante "⋯"; toccandolo — o toccando il titolo nella
//! Testata, o con il tap a tre dita — si apre questa scheda con i comandi
//! grandi e comodi da toccare. In modalità `native` (senza plugin) una
//! maniglia alla iPadOS sopra la finestra attiva fa lo stesso.

use super::{rule, text};
use crate::apps::icon_for_class;
use crate::glyph::{self, Glyph};
use crate::layer::{self, Keyboard, Level};
use crate::shell::{shell, Overlay};
use folio_core::config::ChromeMode;
use folio_core::hypr::{self, Client, Dispatch};
use folio_core::locale;
use gtk::glib;
use gtk::prelude::*;
use gtk4_layer_shell::{Edge, LayerShell};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

const CARD_W: i32 = 460;

pub struct Chrome {
    window: gtk::Window,
    fixed: gtk::Fixed,
    revealer: gtk::Revealer,
    card: gtk::Box,
    open: Cell<bool>,
    target: RefCell<Option<Client>>,
    handle: RefCell<Option<gtk::Window>>,
    /// Ultimo controllo "hyprbars è caricato?" (si ricontrolla ogni 20 s).
    bars_checked: Cell<Option<(bool, Instant)>>,
}

impl Chrome {
    pub fn new() -> Rc<Self> {
        let window = layer::surface("folio-chrome", Level::Overlay, Keyboard::OnDemand);
        layer::fill_screen(&window);
        let fixed = gtk::Fixed::new();
        fixed.add_css_class("chrome-scrim");
        let revealer = gtk::Revealer::new();
        revealer.set_transition_type(gtk::RevealerTransitionType::SlideDown);
        revealer.set_transition_duration(200);
        let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
        card.add_css_class("chrome-card");
        card.set_size_request(CARD_W, -1);
        revealer.set_child(Some(&card));
        fixed.put(&revealer, 0.0, 0.0);
        window.set_child(Some(&fixed));

        let chrome = Rc::new(Self {
            window,
            fixed,
            revealer,
            card,
            open: Cell::new(false),
            target: RefCell::new(None),
            handle: RefCell::new(None),
            bars_checked: Cell::new(None),
        });

        // Tocco fuori dalla scheda → chiude.
        let click = gtk::GestureClick::new();
        let w = Rc::downgrade(&chrome);
        click.connect_released(move |_, _, x, y| {
            let Some(c) = w.upgrade() else { return };
            let inside = c
                .fixed
                .pick(x, y, gtk::PickFlags::DEFAULT)
                .map(|p| p.is_ancestor(&c.card) || p == *c.card.upcast_ref::<gtk::Widget>())
                .unwrap_or(false);
            if !inside {
                c.close();
            }
        });
        chrome.fixed.add_controller(click);
        let w = Rc::downgrade(&chrome);
        layer::close_on_escape(&chrome.window, move || {
            if let Some(c) = w.upgrade() {
                c.close();
            }
        });
        chrome
    }

    /// Serve la maniglia nativa? Sì in modalità `native`, e anche in modalità
    /// `hyprbars` se il plugin non risulta caricato (nessuna finestra resta senza comandi).
    fn wants_handle(&self) -> bool {
        match shell().config.borrow().chrome.mode {
            ChromeMode::Native => true,
            ChromeMode::Off => false,
            ChromeMode::Hyprbars => {
                if !hypr::available() {
                    return false;
                }
                let fresh = self
                    .bars_checked
                    .get()
                    .filter(|(_, t)| t.elapsed() < Duration::from_secs(20));
                let loaded = match fresh {
                    Some((loaded, _)) => loaded,
                    None => {
                        let loaded = hypr::request("plugin list")
                            .map(|s| s.contains("hyprbars"))
                            .unwrap_or(false);
                        self.bars_checked.set(Some((loaded, Instant::now())));
                        if !loaded {
                            log::info!("hyprbars non caricato: uso la maniglia nativa per i comandi finestra");
                        }
                        loaded
                    }
                };
                !loaded
            }
        }
    }

    /// Riposiziona la maniglia nativa sulla finestra attiva.
    pub fn follow_active(&self) {
        let active = shell().active_client();
        if let Some(target) = self.target.borrow().as_ref() {
            let gone = shell().hypr.borrow().by_address(&target.address).is_none();
            if gone && self.open.get() {
                glib::idle_add_local_once(|| {
                    if let Some(c) = shell().chrome.get() {
                        c.close();
                    }
                });
            }
        }
        if !self.wants_handle() {
            if let Some(h) = self.handle.borrow().as_ref() {
                h.set_visible(false);
            }
            return;
        }
        if self.handle.borrow().is_none() {
            *self.handle.borrow_mut() = Some(build_handle());
        }
        let handle = self
            .handle
            .borrow()
            .clone()
            .expect("maniglia appena creata");
        // Sotto un foglio a schermo intero la maniglia non deve affiorare.
        let s = shell();
        let sheet_open = s.home.get().map(|o| o.is_open()).unwrap_or(false)
            || s.overview.get().map(|o| o.is_open()).unwrap_or(false)
            || s.shade.get().map(|o| o.is_open()).unwrap_or(false)
            || s.layout.get().map(|o| o.is_open()).unwrap_or(false);
        let active = active.filter(|_| !sheet_open);
        match active {
            Some(c) if c.fullscreen == 0 && !c.is_minimized() => {
                let (ox, oy) = layer::monitor_origin();
                let x = c.at[0] - ox + c.size[0] / 2 - 42;
                let y = (c.at[1] - oy - 15).max(0);
                handle.set_margin(Edge::Left, x.max(0));
                handle.set_margin(Edge::Top, y);
                handle.set_visible(true);
            }
            _ => handle.set_visible(false),
        }
    }

    fn fill(&self, c: &Client) {
        while let Some(child) = self.card.first_child() {
            self.card.remove(&child);
        }
        let s = shell();
        let snap = s.hypr.borrow().clone();
        let app_name = s
            .apps
            .borrow()
            .for_class(c.app_class())
            .map(|a| a.name.clone())
            .unwrap_or_else(|| c.app_class().to_string());
        let pages: Vec<i64> = snap.normal_workspaces().iter().map(|w| w.id).collect();
        let ws_pos = pages
            .iter()
            .position(|&id| id == c.workspace.id)
            .map(|p| p + 1)
            .unwrap_or(0);

        // Intestazione: occhiello con app e spazio, titolo in corsivo.
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        head.add_css_class("chrome-head");
        head.append(&icon_for_class(&s.apps.borrow(), c.app_class(), 28));
        let titles = gtk::Box::new(gtk::Orientation::Vertical, 2);
        titles.set_hexpand(true);
        let kicker = if ws_pos > 0 {
            format!(
                "{} · SPAZIO {}",
                app_name.to_uppercase(),
                locale::folio_number(ws_pos)
            )
        } else {
            app_name.to_uppercase()
        };
        let k = text(&kicker, &["kicker"]);
        k.set_xalign(0.0);
        titles.append(&k);
        let t = text(
            if c.title.is_empty() {
                &app_name
            } else {
                &c.title
            },
            &["serif", "chrome-title"],
        );
        t.set_xalign(0.0);
        t.set_ellipsize(gtk::pango::EllipsizeMode::End);
        t.set_max_width_chars(30);
        titles.append(&t);
        head.append(&titles);
        self.card.append(&head);
        self.card.append(&rule(false));

        // Comandi.
        let grid = gtk::Grid::new();
        grid.add_css_class("chrome-grid");
        grid.set_column_homogeneous(true);
        grid.set_row_spacing(6);
        grid.set_column_spacing(6);
        let addr = c.address.clone();
        let floating = c.floating;
        let maximized = c.fullscreen == 1;
        type Act = Box<dyn Fn(&str)>;
        let items: Vec<(Glyph, &str, Act, bool)> = vec![
            (
                Glyph::Close,
                "Chiudi",
                Box::new(|a: &str| {
                    hypr::dispatch(&Dispatch::Close(a.into()));
                }),
                true,
            ),
            (
                Glyph::Minimize,
                "Riduci",
                Box::new(|a: &str| {
                    if let Some(c) = shell().hypr.borrow().by_address(a).cloned() {
                        shell().minimize(&c);
                    }
                }),
                true,
            ),
            (
                Glyph::Maximize,
                if maximized {
                    "Ripristina"
                } else {
                    "Ingrandisci"
                },
                Box::new(|a: &str| {
                    hypr::dispatch(&Dispatch::Maximize(a.into()));
                }),
                true,
            ),
            (
                Glyph::Fullscreen,
                "Schermo intero",
                Box::new(|a: &str| {
                    hypr::dispatch(&Dispatch::Fullscreen(a.into()));
                }),
                true,
            ),
            (
                Glyph::Float,
                if floating { "Affianca" } else { "Fluttuante" },
                Box::new(|a: &str| {
                    hypr::dispatch(&Dispatch::ToggleFloat(a.into()));
                }),
                true,
            ),
            (
                if floating { Glyph::Center } else { Glyph::Pin },
                if floating { "Centra" } else { "Fissa" },
                Box::new(move |a: &str| {
                    if floating {
                        hypr::dispatch(&Dispatch::Center(a.into()));
                    } else {
                        // Fissare richiede una finestra fluttuante: la rendiamo tale e la fissiamo.
                        hypr::dispatch(&Dispatch::ToggleFloat(a.into()));
                        hypr::dispatch(&Dispatch::Pin(a.into()));
                    }
                }),
                true,
            ),
            (
                Glyph::SplitLeft,
                "A sinistra",
                Box::new(move |a: &str| {
                    if floating {
                        hypr::dispatch(&Dispatch::ToggleFloat(a.into()));
                    }
                    hypr::dispatch(&Dispatch::FocusWindow(a.into()));
                    hypr::dispatch(&Dispatch::MoveDirection("left".into()));
                }),
                true,
            ),
            (
                Glyph::SplitRight,
                "A destra",
                Box::new(move |a: &str| {
                    if floating {
                        hypr::dispatch(&Dispatch::ToggleFloat(a.into()));
                    }
                    hypr::dispatch(&Dispatch::FocusWindow(a.into()));
                    hypr::dispatch(&Dispatch::MoveDirection("right".into()));
                }),
                true,
            ),
        ];
        for (i, (g, label, act, enabled)) in items.into_iter().enumerate() {
            let b = glyph::button(g, 22, Some(label));
            b.add_css_class("chrome-btn");
            if g == Glyph::Close {
                b.add_css_class("danger");
            }
            b.set_sensitive(enabled);
            let a = addr.clone();
            b.connect_clicked(move |_| {
                act(&a);
                shell().refresh_hypr();
                if let Some(c) = shell().chrome.get() {
                    c.close();
                }
            });
            grid.attach(&b, (i % 4) as i32, (i / 4) as i32, 1, 1);
        }
        self.card.append(&grid);
        self.card.append(&rule(false));

        // Sposta in un altro spazio (spazi dinamici: c'è sempre "+").
        let move_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        move_row.add_css_class("chrome-move");
        move_row.append(&text("SPOSTA IN", &["kicker"]));
        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        move_row.append(&spacer);
        let mut targets: Vec<(String, String)> = pages
            .iter()
            .enumerate()
            .map(|(i, id)| (locale::folio_number(i + 1), id.to_string()))
            .collect();
        targets.push(("+".into(), snap.next_free_workspace().to_string()));
        for (label, ws) in targets {
            let b = gtk::Button::with_label(&label);
            b.add_css_class("btn");
            b.add_css_class("chip");
            b.add_css_class("mono");
            if ws == c.workspace.id.to_string() {
                b.add_css_class("current");
            }
            let a = addr.clone();
            b.connect_clicked(move |_| {
                hypr::dispatch(&Dispatch::MoveToWorkspace {
                    address: a.clone(),
                    workspace: ws.clone(),
                    follow: true,
                });
                if let Some(c) = shell().chrome.get() {
                    c.close();
                }
            });
            move_row.append(&b);
        }
        self.card.append(&move_row);
    }

    fn position(&self, c: &Client) {
        let (mw, mh) = layer::monitor_size();
        let (ox, oy) = layer::monitor_origin();
        let (wx, wy, ww) = (c.at[0] - ox, c.at[1] - oy, c.size[0]);
        let x = if self
            .handle
            .borrow()
            .as_ref()
            .map(|h| h.is_visible())
            .unwrap_or(false)
        {
            // Maniglia nativa al centro: la scheda scende da lì.
            wx + ww / 2 - CARD_W / 2
        } else {
            // Il pulsante "⋯" di hyprbars è a destra.
            wx + ww - CARD_W - 8
        };
        let (_, nat) = self.card.preferred_size();
        let h = nat.height().max(200);
        let x = x.clamp(12, (mw - CARD_W - 12).max(12));
        let y = (wy + 6).clamp(12, (mh - h - 12).max(12));
        self.fixed.move_(&self.revealer, x as f64, y as f64);
    }
}

fn build_handle() -> gtk::Window {
    let w = layer::surface("folio-handle", Level::Top, Keyboard::None);
    w.set_anchor(Edge::Top, true);
    w.set_anchor(Edge::Left, true);
    // -1: i margini si contano dal bordo dello schermo, non dall'area lasciata
    // libera dalla Testata (le coordinate di Hyprland sono assolute).
    w.set_exclusive_zone(-1);
    let hit = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    hit.add_css_class("chrome-handle-hit");
    hit.set_size_request(84, 20);
    let pill = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    pill.add_css_class("chrome-handle");
    pill.set_size_request(56, 5);
    pill.set_halign(gtk::Align::Center);
    pill.set_valign(gtk::Align::Center);
    pill.set_hexpand(true);
    hit.append(&pill);
    w.set_child(Some(&hit));
    layer::on_tap(&hit, || shell().command("chrome toggle"));
    // Trascinare la maniglia verso il basso riduce la finestra (come iPadOS).
    layer::on_swipe(&hit, |_, vy| {
        if vy > 500.0 {
            shell().command("minimize");
        }
    });
    w.present();
    w.set_visible(false);
    w
}

impl Overlay for Chrome {
    fn is_open(&self) -> bool {
        self.open.get()
    }

    fn open(&self) {
        let Some(c) = shell().active_client() else {
            return;
        };
        if self.open.replace(true) {
            return;
        }
        self.fill(&c);
        *self.target.borrow_mut() = Some(c.clone());
        layer::on_monitor(&self.window);
        self.window.present();
        self.position(&c);
        self.revealer.set_reveal_child(true);
    }

    fn close(&self) {
        if !self.open.replace(false) {
            return;
        }
        self.revealer.set_reveal_child(false);
        self.target.borrow_mut().take();
        let w = self.window.downgrade();
        glib::timeout_add_local_once(Duration::from_millis(210), move || {
            let closed = shell().chrome.get().map(|c| !c.open.get()).unwrap_or(true);
            if let (Some(w), true) = (w.upgrade(), closed) {
                w.set_visible(false);
            }
        });
    }
}

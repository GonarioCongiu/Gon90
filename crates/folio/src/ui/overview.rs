//! Il Sommario: spazi di lavoro alla GNOME Shell + "Recenti" di Android.
//!
//! * In alto le miniature degli spazi (sempre uno vuoto in più: spazi dinamici).
//! * Al centro le finestre dello spazio scelto, come schede con anteprima.
//! * Swipe orizzontale: spazio precedente/successivo.
//! * Tocco su una scheda → la apre. Swipe in alto su una scheda → la chiude.
//! * Tieni premuto una scheda, poi tocca uno spazio → ve la sposta.
//! * Tocco su un'area vuota → vai a quello spazio.

use super::{clear, rule, text};
use crate::apps::icon_for_class;
use crate::glyph::{self, Glyph};
use crate::layer::{self, Keyboard, Level};
use crate::shell::{shell, Overlay};
use folio_core::hypr::{self, Client, Dispatch, Monitor, Snapshot};
use folio_core::{locale, sys};
use gtk::prelude::*;
use gtk::{gdk, glib};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

const MINIMIZED_PAGE: i64 = -1000;

pub struct Overview {
    window: gtk::Window,
    revealer: gtk::Revealer,
    strip: gtk::Box,
    stack: gtk::Stack,
    heading: gtk::Label,
    count: gtk::Label,
    hint: gtk::Label,
    open: Cell<bool>,
    current: Cell<i64>,
    picked: RefCell<Option<String>>,
    shots: RefCell<HashMap<String, gdk::Texture>>,
    signature: RefCell<String>,
}

/// Anteprima grezza (RGB) di una finestra, prodotta nel thread di cattura.
struct Shot {
    address: String,
    width: i32,
    height: i32,
    rgb: Vec<u8>,
}

impl Overview {
    pub fn new() -> Rc<Self> {
        let window = layer::surface("folio-overview", Level::Top, Keyboard::Exclusive);
        layer::fill_free_area(&window);
        let revealer = gtk::Revealer::new();
        revealer.set_transition_type(gtk::RevealerTransitionType::Crossfade);
        revealer.set_transition_duration(220);
        let sheet = gtk::Box::new(gtk::Orientation::Vertical, 0);
        sheet.add_css_class("overview");
        sheet.add_css_class("sheet");
        revealer.set_child(Some(&sheet));
        window.set_child(Some(&revealer));

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 20);
        header.add_css_class("sheet-header");
        header.append(&text("Sommario", &["serif", "sheet-title"]));
        let issue = text("SPAZI DI LAVORO E FINESTRE", &["kicker", "sheet-issue"]);
        issue.set_valign(gtk::Align::End);
        issue.set_margin_bottom(10);
        header.append(&issue);
        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        header.append(&spacer);
        let to_home = gtk::Button::with_label("Indice");
        to_home.add_css_class("btn");
        to_home.add_css_class("text-btn");
        to_home.set_valign(gtk::Align::Center);
        to_home.connect_clicked(|_| {
            let s = shell();
            s.close_overlays();
            s.command("home show");
        });
        header.append(&to_home);
        let close = glyph::button(Glyph::Close, 22, None);
        close.add_css_class("close-btn");
        close.set_valign(gtk::Align::Center);
        close.connect_clicked(|_| shell().command("overview hide"));
        header.append(&close);
        sheet.append(&header);

        let strip = gtk::Box::new(gtk::Orientation::Horizontal, 18);
        strip.add_css_class("ws-strip-large");
        strip.set_halign(gtk::Align::Center);
        sheet.append(&strip);
        sheet.append(&rule(false));

        let page_head = gtk::Box::new(gtk::Orientation::Horizontal, 14);
        page_head.add_css_class("page-head");
        let heading = text("", &["serif", "page-title"]);
        page_head.append(&heading);
        let count = text("", &["kicker", "page-count"]);
        count.set_valign(gtk::Align::Center);
        page_head.append(&count);
        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        page_head.append(&spacer);
        let go = gtk::Button::with_label("Vai a questo spazio  →");
        go.add_css_class("btn");
        go.add_css_class("text-btn");
        page_head.append(&go);
        sheet.append(&page_head);

        let stack = gtk::Stack::new();
        stack.set_transition_type(gtk::StackTransitionType::SlideLeftRight);
        stack.set_transition_duration(260);
        stack.set_vexpand(true);
        stack.add_css_class("ov-stack");
        sheet.append(&stack);

        let hint = text(
            "Tocca per aprire · scorri una scheda verso l'alto per chiuderla · tienila premuta e scegli uno spazio per spostarla",
            &["caption", "ov-hint"],
        );
        sheet.append(&hint);

        let ov = Rc::new(Self {
            window,
            revealer,
            strip,
            stack,
            heading,
            count,
            hint,
            open: Cell::new(false),
            current: Cell::new(1),
            picked: RefCell::new(None),
            shots: RefCell::new(HashMap::new()),
            signature: RefCell::new(String::new()),
        });

        let w = Rc::downgrade(&ov);
        go.connect_clicked(move |_| {
            if let Some(o) = w.upgrade() {
                o.go_to(o.current.get());
            }
        });
        let w = Rc::downgrade(&ov);
        layer::on_swipe(&ov.stack, move |vx, vy| {
            let Some(o) = w.upgrade() else { return };
            if vx.abs() > 600.0 && vx.abs() > vy.abs() * 1.3 {
                o.step(if vx < 0.0 { 1 } else { -1 });
            }
        });
        let w = Rc::downgrade(&ov);
        layer::close_on_escape(&ov.window, move || {
            if let Some(o) = w.upgrade() {
                o.close();
            }
        });
        ov
    }

    /// Ordine delle pagine: spazi normali + uno nuovo + (se servono) le finestre ridotte.
    fn pages(snap: &Snapshot) -> Vec<i64> {
        let mut v: Vec<i64> = snap.normal_workspaces().iter().map(|w| w.id).collect();
        v.push(snap.next_free_workspace());
        if !snap.minimized().is_empty() {
            v.push(MINIMIZED_PAGE);
        }
        v
    }

    fn step(&self, delta: i32) {
        let pages = Self::pages(&shell().hypr.borrow());
        let Some(pos) = pages.iter().position(|&p| p == self.current.get()) else {
            return;
        };
        let next = (pos as i32 + delta).clamp(0, pages.len() as i32 - 1) as usize;
        self.show_page(pages[next]);
    }

    fn go_to(&self, ws: i64) {
        if ws != MINIMIZED_PAGE {
            hypr::dispatch(&Dispatch::Workspace(ws.to_string()));
        }
        self.close();
    }

    fn show_page(&self, ws: i64) {
        self.current.set(ws);
        self.stack.set_visible_child_name(&ws.to_string());
        self.update_heading();
        self.build_strip();
    }

    fn update_heading(&self) {
        let s = shell();
        let snap = s.hypr.borrow();
        let ws = self.current.get();
        let (title, n) = if ws == MINIMIZED_PAGE {
            ("Finestre ridotte".to_string(), snap.minimized().len())
        } else {
            let pages = Self::pages(&snap);
            let pos = pages.iter().position(|&p| p == ws).unwrap_or(0) + 1;
            (
                format!("Spazio {}", locale::folio_number(pos)),
                snap.windows_on(ws).len(),
            )
        };
        self.heading.set_text(&title);
        self.count.set_text(&if n == 0 {
            "PAGINA BIANCA".into()
        } else {
            locale::plural(n, "finestra", "finestre").to_uppercase()
        });
    }

    /// Ricostruisce tutto (chiamato all'apertura e quando Hyprland cambia).
    fn build(&self) {
        let s = shell();
        let snap = s.hypr.borrow().clone();
        let sig = format!(
            "{:?}|{:?}",
            snap.clients
                .iter()
                .map(|c| (&c.address, c.workspace.id, &c.title))
                .collect::<Vec<_>>(),
            Self::pages(&snap)
        );
        *self.signature.borrow_mut() = sig;
        while let Some(child) = self.stack.first_child() {
            self.stack.remove(&child);
        }
        let pages = Self::pages(&snap);
        if !pages.contains(&self.current.get()) {
            self.current.set(snap.active_workspace());
        }
        for ws in &pages {
            let page = self.build_page(&snap, *ws);
            self.stack.add_named(&page, Some(&ws.to_string()));
        }
        self.stack
            .set_visible_child_name(&self.current.get().to_string());
        self.update_heading();
        self.build_strip();
    }

    fn build_strip(&self) {
        clear(&self.strip);
        let s = shell();
        let snap = s.hypr.borrow().clone();
        let mon = snap.focused_monitor().cloned().unwrap_or_else(|| Monitor {
            width: 1920,
            height: 1080,
            scale: 1.0,
            ..Default::default()
        });
        let (mw, mh) = mon.logical_size();
        let th = 96;
        let tw = (th as f64 * mw as f64 / mh.max(1) as f64) as i32;
        let picked = self.picked.borrow().clone();
        let active = snap.active_workspace();
        let me = s.overview.get().map(Rc::downgrade);

        for (i, ws) in Self::pages(&snap).into_iter().enumerate() {
            let thumb = gtk::Box::new(gtk::Orientation::Vertical, 8);
            thumb.add_css_class("ws-cell");
            let frame = gtk::Fixed::new();
            frame.set_size_request(tw, th);
            frame.add_css_class("ws-thumb");
            frame.set_overflow(gtk::Overflow::Hidden);
            if ws == self.current.get() {
                frame.add_css_class("current");
            }
            if ws == active {
                frame.add_css_class("active");
            }
            let is_new = ws != MINIMIZED_PAGE
                && snap.windows_on(ws).is_empty()
                && !snap.workspaces.iter().any(|w| w.id == ws);
            if ws == MINIMIZED_PAGE {
                let g = glyph::glyph(Glyph::Minimize, 22);
                frame.put(&g, (tw / 2 - 11) as f64, (th / 2 - 11) as f64);
            } else if is_new {
                frame.add_css_class("new");
                let g = glyph::glyph(Glyph::Plus, 22);
                frame.put(&g, (tw / 2 - 11) as f64, (th / 2 - 11) as f64);
            } else {
                let catalog = s.apps.borrow();
                let ox = mon.x;
                let oy = mon.y;
                for c in snap.windows_on(ws).iter().rev() {
                    // Rettangoli in scala, sempre contenuti nella cornice (striscia di altezza uniforme).
                    let x =
                        (((c.at[0] - ox) as f64 * tw as f64 / mw as f64) as i32).clamp(0, tw - 10);
                    let y =
                        (((c.at[1] - oy) as f64 * th as f64 / mh as f64) as i32).clamp(0, th - 10);
                    let w =
                        ((c.size[0] as f64 * tw as f64 / mw as f64) as i32).clamp(8, tw - x - 2);
                    let h =
                        ((c.size[1] as f64 * th as f64 / mh as f64) as i32).clamp(8, th - y - 2);
                    let rect = gtk::CenterBox::new();
                    rect.add_css_class("win-rect");
                    rect.set_size_request(w - 2, h - 2);
                    rect.set_overflow(gtk::Overflow::Hidden);
                    if w > 22 && h > 22 {
                        rect.set_center_widget(Some(&icon_for_class(&catalog, c.app_class(), 14)));
                    }
                    frame.put(&rect, x.max(0) as f64, y.max(0) as f64);
                }
            }
            thumb.append(&frame);
            let label = if ws == MINIMIZED_PAGE {
                "RIDOTTE".to_string()
            } else if is_new {
                "NUOVO".to_string()
            } else {
                locale::folio_number(i + 1)
            };
            let l = text(&label, &["kicker", "ws-label"]);
            if picked.is_some() && ws != MINIMIZED_PAGE {
                l.set_text("SPOSTA QUI");
                thumb.add_css_class("drop-target");
            }
            thumb.append(&l);
            let me = me.clone();
            layer::on_tap(&thumb, move || {
                let Some(o) = me.as_ref().and_then(|w| w.upgrade()) else {
                    return;
                };
                o.thumb_tapped(ws);
            });
            self.strip.append(&thumb);
        }
    }

    fn thumb_tapped(&self, ws: i64) {
        let picked = self.picked.borrow_mut().take();
        if let Some(address) = picked {
            if ws != MINIMIZED_PAGE {
                hypr::dispatch(&Dispatch::MoveToWorkspace {
                    address,
                    workspace: ws.to_string(),
                    follow: false,
                });
            } else if let Some(c) = shell().hypr.borrow().by_address(&address).cloned() {
                shell().minimize(&c);
            }
            self.hint.remove_css_class("accent");
            shell().refresh_hypr();
            return;
        }
        if self.current.get() == ws {
            // Secondo tocco sullo spazio già mostrato: ci si va.
            self.go_to(ws);
        } else {
            self.show_page(ws);
        }
    }

    fn build_page(&self, snap: &Snapshot, ws: i64) -> gtk::Widget {
        let page = gtk::Box::new(gtk::Orientation::Vertical, 0);
        page.add_css_class("ov-page");
        let wins: Vec<Client> = if ws == MINIMIZED_PAGE {
            snap.minimized().into_iter().cloned().collect()
        } else {
            snap.windows_on(ws).into_iter().cloned().collect()
        };
        let me = shell().overview.get().map(Rc::downgrade);

        if wins.is_empty() {
            let empty = gtk::Box::new(gtk::Orientation::Vertical, 10);
            empty.set_valign(gtk::Align::Center);
            empty.set_vexpand(true);
            empty.append(&text("Pagina bianca", &["serif", "empty-title"]));
            empty.append(&text("Tocca per iniziare a scrivere qui", &["caption"]));
            page.append(&empty);
        } else {
            let (mw, mh) = layer::monitor_size();
            let inset = layer::reserved_insets();
            let n = wins.len();
            let cols = match n {
                1 => 1,
                2 => 2,
                3 | 4 => 2,
                5 | 6 => 3,
                _ => 4,
            };
            let rows = n.div_ceil(cols);
            let gap = 28;
            // Intestazione, miniature, titolo di pagina e didascalia occupano ~500 px.
            let avail_w = (mw - inset[2] - inset[3] - 160) as f64;
            let avail_h = (mh - inset[0] - inset[1] - 500).max(220) as f64;
            let card_w = ((avail_w - gap as f64 * (cols - 1) as f64) / cols as f64).min(760.0);
            // Altezza utile del corpo della scheda (44 px sono la sua intestazione).
            let card_h = ((avail_h - gap as f64 * (rows - 1) as f64) / rows as f64 - 44.0)
                .min(card_w * 0.66);
            let card_w = card_w.min(card_h / 0.6);

            let grid = gtk::Grid::new();
            grid.set_row_spacing(gap as u32);
            grid.set_column_spacing(gap as u32);
            grid.set_halign(gtk::Align::Center);
            grid.set_valign(gtk::Align::Center);
            grid.set_vexpand(true);
            for (i, c) in wins.iter().enumerate() {
                let card = self.card(c, card_w as i32, card_h as i32, ws == MINIMIZED_PAGE);
                grid.attach(&card, (i % cols) as i32, (i / cols) as i32, 1, 1);
            }
            page.append(&grid);
        }

        // Tocco su un'area vuota della pagina → vai allo spazio.
        let click = gtk::GestureClick::new();
        let pw = page.downgrade();
        click.connect_released(move |_, n, x, y| {
            let (Some(o), Some(page)) = (me.as_ref().and_then(|w| w.upgrade()), pw.upgrade())
            else {
                return;
            };
            if n != 1 {
                return;
            }
            let on_card = page
                .pick(x, y, gtk::PickFlags::DEFAULT)
                .map(|w| {
                    let mut cur = Some(w);
                    while let Some(widget) = cur {
                        if widget.has_css_class("ov-card") {
                            return true;
                        }
                        cur = widget.parent();
                    }
                    false
                })
                .unwrap_or(false);
            if !on_card {
                o.go_to(ws);
            }
        });
        page.add_controller(click);
        page.upcast()
    }

    fn card(&self, c: &Client, w: i32, h: i32, minimized: bool) -> gtk::Widget {
        let s = shell();
        let holder = gtk::Fixed::new();
        holder.set_size_request(w, h + 44);

        let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
        card.add_css_class("ov-card");
        card.set_size_request(w, h + 44);
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        head.add_css_class("ov-card-head");
        head.append(&icon_for_class(&s.apps.borrow(), c.app_class(), 20));
        let title = text(
            if c.title.is_empty() {
                c.app_class()
            } else {
                &c.title
            },
            &["ov-card-title"],
        );
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title.set_hexpand(true);
        title.set_xalign(0.0);
        head.append(&title);
        let close = glyph::button(Glyph::Close, 16, None);
        close.add_css_class("ov-card-close");
        let addr = c.address.clone();
        close.connect_clicked(move |_| {
            hypr::dispatch(&Dispatch::Close(addr.clone()));
        });
        head.append(&close);
        card.append(&head);

        let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
        body.add_css_class("ov-card-body");
        body.set_size_request(w, h);
        body.set_vexpand(true);
        match self.shots.borrow().get(&c.address) {
            Some(tex) => {
                let pic = gtk::Picture::for_paintable(tex);
                pic.set_content_fit(gtk::ContentFit::Contain);
                pic.set_vexpand(true);
                body.append(&pic);
            }
            None => {
                // Nessuna anteprima (altro spazio o grim assente): grande icona su carta.
                let icon = icon_for_class(&s.apps.borrow(), c.app_class(), (h / 3).clamp(32, 128));
                icon.set_vexpand(true);
                icon.set_valign(gtk::Align::Center);
                body.append(&icon);
                body.add_css_class("placeholder");
            }
        }
        card.append(&body);
        holder.put(&card, 0.0, 0.0);

        // Tocco: apri (o ripristina).
        let client = c.clone();
        layer::on_tap(&card, move || {
            let s = shell();
            if minimized {
                s.restore(&client);
            } else {
                s.focus_window(&client);
            }
            s.close_overlays();
        });
        // Tieni premuto: "raccogli" la finestra per spostarla in un altro spazio.
        let addr = c.address.clone();
        let wcard = card.downgrade();
        layer::on_long_press(&card, move |_, _| {
            let Some(o) = shell().overview.get().cloned() else {
                return;
            };
            *o.picked.borrow_mut() = Some(addr.clone());
            if let Some(c) = wcard.upgrade() {
                c.add_css_class("picked");
            }
            o.hint
                .set_text("Ora tocca lo spazio di destinazione in alto");
            o.hint.add_css_class("accent");
            o.build_strip();
        });
        // Swipe verso l'alto: chiude la finestra, come nei Recenti di Android.
        let drag = gtk::GestureDrag::new();
        let wcard = card.downgrade();
        let wholder = holder.downgrade();
        drag.connect_drag_update(move |g, dx, dy| {
            let (Some(card), Some(holder)) = (wcard.upgrade(), wholder.upgrade()) else {
                return;
            };
            if dy < -12.0 && dy.abs() > dx.abs() {
                g.set_state(gtk::EventSequenceState::Claimed);
                holder.move_(&card, 0.0, dy);
                card.set_opacity((1.0 + dy / 320.0).clamp(0.15, 1.0));
            }
        });
        let wcard = card.downgrade();
        let wholder = holder.downgrade();
        let addr = c.address.clone();
        drag.connect_drag_end(move |_, _, dy| {
            let (Some(card), Some(holder)) = (wcard.upgrade(), wholder.upgrade()) else {
                return;
            };
            if dy < -150.0 {
                holder.move_(&card, 0.0, -(h as f64) * 1.2);
                card.set_opacity(0.0);
                hypr::dispatch(&Dispatch::Close(addr.clone()));
            } else {
                holder.move_(&card, 0.0, 0.0);
                card.set_opacity(1.0);
            }
        });
        card.add_controller(drag);
        holder.upcast()
    }

    pub fn on_hypr(&self) {
        if !self.open.get() {
            return;
        }
        let snap = shell().hypr.borrow().clone();
        let sig = format!(
            "{:?}|{:?}",
            snap.clients
                .iter()
                .map(|c| (&c.address, c.workspace.id, &c.title))
                .collect::<Vec<_>>(),
            Self::pages(&snap)
        );
        if *self.signature.borrow() != sig {
            self.build();
        }
    }

    fn present(&self) {
        let snap = shell().hypr.borrow().clone();
        self.current.set(snap.active_workspace());
        self.picked.borrow_mut().take();
        self.hint.remove_css_class("accent");
        self.hint.set_text(
            "Tocca per aprire · scorri una scheda verso l'alto per chiuderla · tienila premuta e scegli uno spazio per spostarla",
        );
        self.build();
        layer::on_monitor(&self.window);
        self.window.present();
        self.revealer.set_reveal_child(true);
    }
}

/// Cattura lo schermo con grim (PPM: niente decodifica PNG) e ritaglia le finestre visibili.
fn capture(monitor: Option<Monitor>, windows: Vec<Client>) -> Vec<Shot> {
    let Some(mon) = monitor else {
        return Vec::new();
    };
    if !sys::in_path("grim") || windows.is_empty() {
        return Vec::new();
    }
    let out = std::process::Command::new("grim")
        .args(["-t", "ppm", "-o", &mon.name, "-"])
        .stderr(std::process::Stdio::null())
        .output();
    let Ok(out) = out else { return Vec::new() };
    if !out.status.success() {
        return Vec::new();
    }
    let Some((iw, ih, data)) = parse_ppm(&out.stdout) else {
        return Vec::new();
    };
    let (lw, _) = mon.logical_size();
    let scale = iw as f64 / lw.max(1) as f64;
    windows
        .into_iter()
        .filter_map(|c| {
            let x = (((c.at[0] - mon.x) as f64) * scale).round().max(0.0) as usize;
            let y = (((c.at[1] - mon.y) as f64) * scale).round().max(0.0) as usize;
            let w = ((c.size[0] as f64) * scale).round() as usize;
            let h = ((c.size[1] as f64) * scale).round() as usize;
            let w = w.min(iw.saturating_sub(x));
            let h = h.min(ih.saturating_sub(y));
            if w < 8 || h < 8 {
                return None;
            }
            // Riduzione intera: le schede non superano ~900 px di larghezza.
            let f = w.div_ceil(900).max(1);
            let (ow, oh) = (w / f, h / f);
            // Media su blocchi f×f: testo e filetti restano leggibili anche ridotti.
            let mut rgb = Vec::with_capacity(ow * oh * 3);
            let n = (f * f) as u32;
            for row in 0..oh {
                for col in 0..ow {
                    let mut acc = [0u32; 3];
                    for dy in 0..f {
                        let base = ((y + row * f + dy) * iw + x + col * f) * 3;
                        for px in data[base..base + f * 3].chunks_exact(3) {
                            acc[0] += px[0] as u32;
                            acc[1] += px[1] as u32;
                            acc[2] += px[2] as u32;
                        }
                    }
                    rgb.extend_from_slice(&[
                        (acc[0] / n) as u8,
                        (acc[1] / n) as u8,
                        (acc[2] / n) as u8,
                    ]);
                }
            }
            Some(Shot {
                address: c.address,
                width: ow as i32,
                height: oh as i32,
                rgb,
            })
        })
        .collect()
}

fn parse_ppm(bytes: &[u8]) -> Option<(usize, usize, &[u8])> {
    // Intestazione: "P6" w h maxval, separati da spazi; poi un solo spazio e i dati.
    let mut fields = Vec::new();
    let mut i = 0;
    while fields.len() < 4 && i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i < bytes.len() && bytes[i] == b'#' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        let start = i;
        while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        fields.push(std::str::from_utf8(&bytes[start..i]).ok()?.to_string());
    }
    if fields.len() < 4 || fields[0] != "P6" || fields[3] != "255" {
        return None;
    }
    let (w, h): (usize, usize) = (fields[1].parse().ok()?, fields[2].parse().ok()?);
    let data = bytes.get(i + 1..)?;
    (data.len() >= w * h * 3).then_some((w, h, data))
}

impl Overlay for Overview {
    fn is_open(&self) -> bool {
        self.open.get()
    }

    fn open(&self) {
        if self.open.replace(true) {
            return;
        }
        // Le anteprime si catturano PRIMA di mostrare il Sommario (altrimenti
        // grim fotograferebbe il Sommario stesso).
        let snap = shell().hypr.borrow().clone();
        let mon = snap.focused_monitor().cloned();
        let visible: Vec<Client> = snap
            .windows_on(snap.active_workspace())
            .into_iter()
            .cloned()
            .collect();
        glib::spawn_future_local(async move {
            let shots = gtk::gio::spawn_blocking(move || capture(mon, visible))
                .await
                .unwrap_or_default();
            let Some(o) = shell().overview.get().cloned() else {
                return;
            };
            if !o.open.get() {
                return;
            }
            let mut map = HashMap::new();
            for shot in shots {
                let stride = shot.width as usize * 3;
                let bytes = glib::Bytes::from_owned(shot.rgb);
                let tex = gdk::MemoryTexture::new(
                    shot.width,
                    shot.height,
                    gdk::MemoryFormat::R8g8b8,
                    &bytes,
                    stride,
                );
                map.insert(shot.address, tex.upcast::<gdk::Texture>());
            }
            *o.shots.borrow_mut() = map;
            o.present();
        });
    }

    fn close(&self) {
        if !self.open.replace(false) {
            return;
        }
        self.revealer.set_reveal_child(false);
        self.picked.borrow_mut().take();
        let w = self.window.downgrade();
        glib::timeout_add_local_once(Duration::from_millis(240), move || {
            let closed = shell()
                .overview
                .get()
                .map(|o| !o.open.get())
                .unwrap_or(true);
            if let (Some(w), true) = (w.upgrade(), closed) {
                w.set_visible(false);
                if let Some(o) = shell().overview.get() {
                    o.shots.borrow_mut().clear();
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::parse_ppm;

    #[test]
    fn ppm_header() {
        let mut img = b"P6\n2 1\n255\n".to_vec();
        img.extend_from_slice(&[1, 2, 3, 4, 5, 6]);
        let (w, h, data) = parse_ppm(&img).unwrap();
        assert_eq!((w, h), (2, 1));
        assert_eq!(data, &[1, 2, 3, 4, 5, 6]);
    }
}

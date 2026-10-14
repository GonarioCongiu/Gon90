//! L'Indice: la schermata Home.
//!
//! N° 01 «In evidenza» — widget grandi impaginati su una gabbia a colonne.
//! Tenere premuto entra in modalità modifica (le tile oscillano come su iOS):
//! tocca una tile e poi un'altra per spostarla, oppure trascinala; gli angoli
//! permettono di rimuoverla, cambiarne il formato e l'inchiostro, o
//! appoggiarla sulla scrivania.
//!
//! N° 02 «Tutte le applicazioni» — indice A–Z in cui ogni icona è un widget
//! piccolo e vivo; tenere premuto apre il menu dell'app.

use super::{clear, rule, text};
use crate::glyph::{self, Glyph};
use crate::layer::{self, Keyboard, Level};
use crate::shell::{shell, Overlay};
use crate::tile::{tile_size, Tile};
use crate::ui::dock::app_menu_popover;
use folio_core::config::{TileSize, TileSpec, TileStyle};
use folio_core::{grid, locale};
use gtk::prelude::*;
use gtk::{gdk, glib, graphene};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

pub struct Home {
    window: gtk::Window,
    revealer: gtk::Revealer,
    scroller: gtk::ScrolledWindow,
    content: gtk::Box,
    featured_section: gtk::Box,
    featured: gtk::Fixed,
    index_box: gtk::Box,
    letters: gtk::Box,
    search: gtk::SearchEntry,
    edit_btn: gtk::Button,
    issue: gtk::Label,
    hint: gtk::Label,
    editing: Cell<bool>,
    selected: Cell<Option<usize>>,
    open: Cell<bool>,
    dirty: Cell<bool>,
    anchors: RefCell<Vec<(char, gtk::Widget)>>,
}

const PAD: i32 = 56;

impl Home {
    pub fn new() -> Rc<Self> {
        let window = layer::surface("folio-home", Level::Top, Keyboard::Exclusive);
        layer::fill_free_area(&window);

        let revealer = gtk::Revealer::new();
        // Dissolvenza in GTK; lo scorrimento dal basso (cassetto di Android) lo fa
        // Hyprland sul livello `folio-home` (vedi rules.lua): più fluido, sulla GPU.
        revealer.set_transition_type(gtk::RevealerTransitionType::Crossfade);
        revealer.set_transition_duration(200);
        let sheet = gtk::Box::new(gtk::Orientation::Vertical, 0);
        sheet.add_css_class("home");
        sheet.add_css_class("sheet");
        revealer.set_child(Some(&sheet));
        window.set_child(Some(&revealer));

        // ── Intestazione ──
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 20);
        header.add_css_class("sheet-header");
        let title = text("Indice", &["serif", "sheet-title"]);
        header.append(&title);
        let issue = text("", &["kicker", "sheet-issue"]);
        issue.set_valign(gtk::Align::End);
        issue.set_margin_bottom(10);
        header.append(&issue);
        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        header.append(&spacer);
        let search = gtk::SearchEntry::new();
        search.set_placeholder_text(Some("Cerca un'applicazione"));
        search.add_css_class("search");
        search.set_valign(gtk::Align::Center);
        search.set_width_chars(26);
        header.append(&search);
        let edit_btn = gtk::Button::with_label("Modifica");
        edit_btn.add_css_class("btn");
        edit_btn.add_css_class("text-btn");
        edit_btn.set_valign(gtk::Align::Center);
        header.append(&edit_btn);
        let close = glyph::button(Glyph::Close, 22, None);
        close.add_css_class("close-btn");
        close.set_valign(gtk::Align::Center);
        close.connect_clicked(|_| shell().command("home hide"));
        header.append(&close);
        sheet.append(&header);
        sheet.append(&rule(false));

        // ── Contenuto scorrevole ──
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.add_css_class("sheet-content");
        let scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .child(&content)
            .build();
        scroller.set_kinetic_scrolling(true);
        sheet.append(&scroller);

        let featured_section = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let hint = text("Tieni premuto per impaginare", &["caption", "section-hint"]);
        featured_section.append(&section_head("01", "In evidenza", Some(&hint)));
        let featured = gtk::Fixed::new();
        featured.set_halign(gtk::Align::Start);
        featured.add_css_class("featured");
        featured_section.append(&featured);
        content.append(&featured_section);

        let letters = gtk::Box::new(gtk::Orientation::Horizontal, 2);
        letters.add_css_class("letters");
        content.append(&section_head("02", "Tutte le applicazioni", Some(&letters)));
        let index_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        index_box.add_css_class("index");
        content.append(&index_box);

        let home = Rc::new(Self {
            window,
            revealer,
            scroller,
            content,
            featured_section,
            featured,
            index_box,
            letters,
            search,
            edit_btn,
            issue,
            hint,
            editing: Cell::new(false),
            selected: Cell::new(None),
            open: Cell::new(false),
            dirty: Cell::new(true),
            anchors: RefCell::new(Vec::new()),
        });

        let weak = Rc::downgrade(&home);
        home.edit_btn.connect_clicked(move |_| {
            if let Some(h) = weak.upgrade() {
                h.set_editing(!h.editing.get());
            }
        });
        let weak = Rc::downgrade(&home);
        home.search.connect_search_changed(move |e| {
            if let Some(h) = weak.upgrade() {
                let q = e.text().to_string();
                h.featured_section.set_visible(q.is_empty());
                h.rebuild_index(&q);
            }
        });
        home.search.connect_activate(|e| {
            let q = e.text().to_string();
            let s = shell();
            let first = s.apps.borrow().apps.iter().find(|a| a.matches(&q)).cloned();
            if let Some(app) = first {
                s.open_app(&app);
            }
        });
        shell().osk.attach(&home.search);

        // Swipe verso il basso (dall'inizio della pagina) chiude, come una tendina.
        let weak = Rc::downgrade(&home);
        layer::on_swipe(&home.scroller, move |vx, vy| {
            if let Some(h) = weak.upgrade() {
                if vy > 900.0
                    && vy.abs() > vx.abs() * 1.5
                    && h.scroller.vadjustment().value() <= 1.0
                {
                    h.close();
                }
            }
        });
        let weak = Rc::downgrade(&home);
        layer::close_on_escape(&home.window, move || {
            if let Some(h) = weak.upgrade() {
                if h.editing.get() {
                    h.set_editing(false);
                } else {
                    h.close();
                }
            }
        });
        home
    }

    // ── Dati ──

    fn specs(&self) -> Vec<TileSpec> {
        let s = shell();
        let st = s.state.borrow();
        st.tiles
            .clone()
            .unwrap_or_else(|| s.config.borrow().home.tiles.clone())
    }

    fn set_specs(&self, specs: Vec<TileSpec>) {
        let s = shell();
        s.state.borrow_mut().tiles = Some(specs);
        s.save_state();
        self.rebuild_featured();
    }

    pub fn add_tile(&self, spec: TileSpec) {
        let mut specs = self.specs();
        specs.push(spec);
        self.set_specs(specs);
    }

    pub fn invalidate(&self) {
        self.dirty.set(true);
        if self.open.get() {
            self.rebuild_all();
        }
    }

    fn rebuild_all(&self) {
        self.dirty.set(false);
        self.rebuild_featured();
        self.rebuild_index(&self.search.text());
    }

    fn grid_metrics(&self) -> (i32, i32, u32) {
        let s = shell();
        let cfg = s.config.borrow();
        let scale = cfg.theme.scale;
        let cell = (cfg.home.cell as f64 * scale) as i32;
        let gap = (cfg.home.gap as f64 * scale) as i32;
        // Almeno 4 colonne: è la larghezza di una tile "larga".
        let cols = if cfg.home.columns > 0 {
            cfg.home.columns.max(4)
        } else {
            grid::columns_for(layer::monitor_size().0 - 2 * PAD - 20, cell, gap)
        };
        (cell, gap, cols)
    }

    // ── N° 01: In evidenza ──

    fn rebuild_featured(&self) {
        while let Some(child) = self.featured.first_child() {
            self.featured.remove(&child);
        }
        let (cell, gap, cols) = self.grid_metrics();
        let specs = self.specs();
        let sizes: Vec<TileSize> = specs.iter().map(|s| s.size).collect();
        let layout = grid::pack(&sizes, cols);
        let rows = grid::rows(&layout) as i32;
        self.featured.set_size_request(
            cols as i32 * (cell + gap) - gap,
            (rows * (cell + gap) - gap).max(cell),
        );
        let editing = self.editing.get();
        if editing {
            self.featured.add_css_class("editing");
        } else {
            self.featured.remove_css_class("editing");
        }
        self.hint.set_text(if editing {
            "Tocca una tile e poi la nuova posizione · trascina · usa gli angoli"
        } else {
            "Tieni premuto per impaginare"
        });

        for (i, (spec, (col, row, w, h))) in specs.iter().zip(layout).enumerate() {
            let _ = (w, h);
            let eff = spec.clone();
            let tile = Tile::new(&eff, i + 1, cell, gap);
            let (tw, th) = tile_size(&eff, cell, gap);
            tile.root.set_size_request(tw, th);
            if self.selected.get() == Some(i) {
                tile.root.add_css_class("selected");
            }
            self.wire_featured(&tile, i);
            self.featured.put(
                &tile.root,
                (col as i32 * (cell + gap)) as f64,
                (row as i32 * (cell + gap)) as f64,
            );
        }
    }

    fn wire_featured(&self, tile: &Rc<Tile>, index: usize) {
        // Le tile vengono costruite solo all'apertura, quando `shell().home` esiste già.
        let Some(me) = shell().home.get().cloned() else {
            return;
        };
        let editing = self.editing.get();
        let t = tile.clone();
        let w = Rc::downgrade(&me);
        layer::on_tap(&tile.root, move || {
            let Some(h) = w.upgrade() else { return };
            if h.editing.get() {
                h.tap_in_edit(index);
            } else {
                t.activate();
            }
        });
        let w = Rc::downgrade(&me);
        layer::on_long_press(&tile.root, move |_, _| {
            if let Some(h) = w.upgrade() {
                if !h.editing.get() {
                    h.set_editing(true);
                }
            }
        });
        if !editing {
            return;
        }

        // Angoli della modalità modifica.
        let corner = |g: Glyph, halign: gtk::Align, valign: gtk::Align, tip: &str| {
            let b = glyph::button(g, 14, None);
            b.add_css_class("tile-chip");
            b.set_halign(halign);
            b.set_valign(valign);
            b.set_tooltip_text(Some(tip));
            tile.root.add_overlay(&b);
            b
        };
        let w = Rc::downgrade(&me);
        corner(
            Glyph::Close,
            gtk::Align::Start,
            gtk::Align::Start,
            "Rimuovi",
        )
        .connect_clicked(move |_| {
            if let Some(h) = w.upgrade() {
                let mut specs = h.specs();
                if index < specs.len() {
                    specs.remove(index);
                }
                h.selected.set(None);
                h.set_specs(specs);
            }
        });
        let w = Rc::downgrade(&me);
        corner(Glyph::Resize, gtk::Align::End, gtk::Align::End, "Formato").connect_clicked(
            move |_| {
                if let Some(h) = w.upgrade() {
                    let mut specs = h.specs();
                    if let Some(s) = specs.get_mut(index) {
                        s.size = s.size.next();
                    }
                    h.set_specs(specs);
                }
            },
        );
        let w = Rc::downgrade(&me);
        corner(
            Glyph::Palette,
            gtk::Align::End,
            gtk::Align::Start,
            "Inchiostro",
        )
        .connect_clicked(move |_| {
            if let Some(h) = w.upgrade() {
                let mut specs = h.specs();
                if let Some(s) = specs.get_mut(index) {
                    s.style = s.style.next();
                }
                h.set_specs(specs);
            }
        });
        let spec = tile.spec.clone();
        corner(
            Glyph::Desk,
            gtk::Align::Start,
            gtk::Align::End,
            "Sulla scrivania",
        )
        .connect_clicked(move |_| {
            if let Some(d) = shell().desk.get() {
                d.add(spec.clone());
            }
        });

        // Trascinamento (drag & drop) per riordinare.
        let src = gtk::DragSource::new();
        src.set_actions(gdk::DragAction::MOVE);
        src.connect_prepare(move |_, _, _| {
            Some(gdk::ContentProvider::for_value(&(index as u32).to_value()))
        });
        let root = tile.root.clone();
        src.connect_drag_begin(move |s, _| {
            let paintable = gtk::WidgetPaintable::new(Some(&root));
            s.set_icon(Some(&paintable), 24, 24);
        });
        tile.root.add_controller(src);
        let drop = gtk::DropTarget::new(glib::Type::U32, gdk::DragAction::MOVE);
        let w = Rc::downgrade(&me);
        drop.connect_drop(move |_, value, _, _| {
            let (Some(h), Ok(from)) = (w.upgrade(), value.get::<u32>()) else {
                return false;
            };
            h.move_tile(from as usize, index);
            true
        });
        tile.root.add_controller(drop);
    }

    fn tap_in_edit(&self, index: usize) {
        match self.selected.get() {
            None => {
                self.selected.set(Some(index));
                self.rebuild_featured();
            }
            Some(from) if from == index => {
                self.selected.set(None);
                self.rebuild_featured();
            }
            Some(from) => {
                self.selected.set(None);
                self.move_tile(from, index);
            }
        }
    }

    fn move_tile(&self, from: usize, to: usize) {
        let mut specs = self.specs();
        if from >= specs.len() || to >= specs.len() || from == to {
            return;
        }
        let item = specs.remove(from);
        specs.insert(to, item);
        self.set_specs(specs);
    }

    /// Entra in modalità modifica (anche da tastiera: `folio msg home edit`).
    pub fn edit(&self) {
        self.set_editing(true);
    }

    fn set_editing(&self, on: bool) {
        self.editing.set(on);
        self.selected.set(None);
        self.edit_btn
            .set_label(if on { "Fine" } else { "Modifica" });
        if on {
            self.edit_btn.add_css_class("accent");
        } else {
            self.edit_btn.remove_css_class("accent");
        }
        self.rebuild_featured();
    }

    // ── N° 02: Indice A–Z ──

    fn rebuild_index(&self, query: &str) {
        clear(&self.index_box);
        clear(&self.letters);
        self.anchors.borrow_mut().clear();
        let s = shell();
        let apps: Vec<_> = s
            .apps
            .borrow()
            .apps
            .iter()
            .filter(|a| query.is_empty() || a.matches(query))
            .cloned()
            .collect();
        let (cell, gap, cols) = self.grid_metrics();

        let mut groups: Vec<(char, Vec<crate::apps::App>)> = Vec::new();
        for app in apps {
            let l = app.letter();
            match groups.last_mut() {
                Some((c, v)) if *c == l => v.push(app),
                _ => groups.push((l, vec![app])),
            }
        }
        if groups.is_empty() {
            self.index_box
                .append(&text("Nessun risultato", &["serif", "empty-note"]));
            return;
        }
        let flow = gtk::FlowBox::new();
        flow.set_selection_mode(gtk::SelectionMode::None);
        flow.set_homogeneous(true);
        flow.set_max_children_per_line(cols);
        flow.set_min_children_per_line(cols.min(4));
        flow.set_column_spacing(gap as u32);
        flow.set_row_spacing(gap as u32);
        flow.set_halign(gtk::Align::Start);
        flow.set_valign(gtk::Align::Start);
        flow.add_css_class("index-flow");
        let mut number = 0usize;
        for (letter, list) in groups {
            // La lettera occupa una cella, come il capolettera di un indice analitico.
            let cell_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
            cell_box.add_css_class("letter-cell");
            cell_box.set_size_request(cell, cell);
            cell_box.set_halign(gtk::Align::Start);
            cell_box.set_valign(gtk::Align::Start);
            let l = text(&letter.to_string(), &["serif", "letter"]);
            l.set_xalign(0.0);
            l.set_yalign(0.0);
            l.set_vexpand(true);
            cell_box.append(&l);
            let count = text(
                &locale::folio_number(list.len()),
                &["kicker", "letter-count"],
            );
            count.set_xalign(0.0);
            cell_box.append(&count);
            flow.insert(&cell_box, -1);
            self.anchors
                .borrow_mut()
                .push((letter, cell_box.clone().upcast()));
            for app in list {
                number += 1;
                let spec = TileSpec {
                    app: Some(app.id.clone()),
                    live: None,
                    size: TileSize::Small,
                    style: TileStyle::Paper,
                    label: None,
                };
                let tile = Tile::new(&spec, number, cell, gap);
                let t = tile.clone();
                layer::on_tap(&tile.root, move || t.activate());
                let anchor = tile.root.downgrade();
                layer::on_long_press(&tile.root, move |_, _| {
                    if let Some(a) = anchor.upgrade() {
                        app_menu_popover(&a, &app);
                    }
                });
                flow.insert(&tile.root, -1);
            }

            let b = gtk::Button::with_label(&letter.to_string());
            b.add_css_class("btn");
            b.add_css_class("letter-btn");
            let w = shell().home.get().map(Rc::downgrade);
            b.connect_clicked(move |_| {
                if let Some(h) = w.as_ref().and_then(|w| w.upgrade()) {
                    h.scroll_to(letter);
                }
            });
            self.letters.append(&b);
        }
        self.index_box.append(&flow);
    }

    fn scroll_to(&self, letter: char) {
        let anchors = self.anchors.borrow();
        let Some((_, widget)) = anchors.iter().find(|(c, _)| *c == letter) else {
            return;
        };
        if let Some(p) = widget.compute_point(&self.content, &graphene::Point::new(0.0, 0.0)) {
            self.scroller.vadjustment().set_value(p.y() as f64 - 16.0);
        }
    }
}

fn section_head(number: &str, title: &str, extra: Option<&impl IsA<gtk::Widget>>) -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    b.add_css_class("section-head");
    let n = text(&format!("N° {number}"), &["kicker", "section-number"]);
    n.set_valign(gtk::Align::Baseline);
    b.append(&n);
    let t = text(title, &["serif", "section-title"]);
    t.set_valign(gtk::Align::Baseline);
    b.append(&t);
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    b.append(&spacer);
    if let Some(e) = extra {
        e.as_ref().set_valign(gtk::Align::Center);
        b.append(e.as_ref());
    }
    b
}

impl Overlay for Home {
    fn is_open(&self) -> bool {
        self.open.get()
    }

    fn open(&self) {
        if self.open.replace(true) {
            return;
        }
        self.search.set_text("");
        self.featured_section.set_visible(true);
        // Le tile sono sempre ricostruite: così leggono metriche e app aggiornate.
        self.rebuild_featured();
        if self.dirty.get() {
            self.dirty.set(false);
            self.rebuild_index("");
        }
        let now = chrono::Local::now();
        self.issue.set_text(
            &format!("{} — {}", locale::issue(&now), locale::long_date(&now)).to_uppercase(),
        );
        self.scroller.vadjustment().set_value(0.0);
        layer::on_monitor(&self.window);
        self.window.present();
        self.revealer.set_reveal_child(true);
    }

    fn close(&self) {
        if !self.open.replace(false) {
            return;
        }
        if self.editing.get() {
            self.set_editing(false);
        }
        self.revealer.set_reveal_child(false);
        shell().osk.hide();
        let w = self.window.downgrade();
        glib::timeout_add_local_once(Duration::from_millis(220), move || {
            let still_closed = shell().home.get().map(|h| !h.open.get()).unwrap_or(true);
            if let (Some(w), true) = (w.upgrade(), still_closed) {
                w.set_visible(false);
            }
        });
    }
}

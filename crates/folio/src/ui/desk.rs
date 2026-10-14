//! La Scrivania: widget appoggiati direttamente sul desktop (livello `bottom`),
//! sopra la Copertina e sotto le finestre. Si spostano in modalità Impaginazione.

use super::{text, Placeable};
use crate::glyph::{self, Glyph};
use crate::layer::{self, Keyboard, Level};
use crate::shell::shell;
use crate::tile::Tile;
use folio_core::config::TileSpec;
use folio_core::state::{DeskWidget, Edge, Placement};
use gtk::glib;
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub struct DeskItem {
    pub id: u32,
    window: gtk::Window,
    holder: gtk::Box,
    spec: RefCell<TileSpec>,
    placement: Cell<Placement>,
}

type ItemAction = Box<dyn Fn(&Rc<DeskItem>)>;

pub struct Desk {
    items: RefCell<Vec<Rc<DeskItem>>>,
}

impl Desk {
    pub fn new() -> Rc<Self> {
        let desk = Rc::new(Self {
            items: RefCell::new(Vec::new()),
        });
        let saved = shell().state.borrow().desk.clone();
        for w in saved {
            desk.spawn(w);
        }
        desk
    }

    pub fn items(&self) -> Vec<Rc<DeskItem>> {
        self.items.borrow().clone()
    }

    fn spawn(&self, w: DeskWidget) {
        let window = layer::surface("folio-desk", Level::Bottom, Keyboard::None);
        let holder = gtk::Box::new(gtk::Orientation::Vertical, 0);
        holder.add_css_class("desk-item");
        window.set_child(Some(&holder));
        let item = Rc::new(DeskItem {
            id: w.id,
            window,
            holder,
            spec: RefCell::new(w.tile.clone()),
            placement: Cell::new(w.placement),
        });
        item.render();
        layer::on_monitor(&item.window);
        item.window.present();
        self.items.borrow_mut().push(item);
    }

    pub fn add(&self, tile: TileSpec) {
        let s = shell();
        let id = s.state.borrow().next_desk_id();
        let k = self.items.borrow().len() as f64;
        let placement = Placement {
            edge: Edge::Float,
            x: 0.06 + 0.03 * k,
            y: 0.16 + 0.04 * k,
            stretch: false,
            vertical: false,
        };
        let w = DeskWidget {
            id,
            tile,
            placement,
        };
        s.state.borrow_mut().desk.push(w.clone());
        s.save_state();
        self.spawn(w);
        s.close_overlays();
    }

    pub fn remove(&self, id: u32) {
        let s = shell();
        s.state.borrow_mut().desk.retain(|d| d.id != id);
        s.save_state();
        let mut items = self.items.borrow_mut();
        if let Some(pos) = items.iter().position(|i| i.id == id) {
            let item = items.remove(pos);
            item.window.close();
        }
    }
}

impl DeskItem {
    fn save(&self) {
        let s = shell();
        if let Some(d) = s
            .state
            .borrow_mut()
            .desk
            .iter_mut()
            .find(|d| d.id == self.id)
        {
            d.tile = self.spec.borrow().clone();
            d.placement = self.placement.get();
        }
        s.save_state();
    }

    fn render(self: &Rc<Self>) {
        while let Some(c) = self.holder.first_child() {
            self.holder.remove(&c);
        }
        let s = shell();
        let (cell, gap) = {
            let cfg = s.config.borrow();
            (
                (cfg.home.cell as f64 * cfg.theme.scale) as i32,
                (cfg.home.gap as f64 * cfg.theme.scale) as i32,
            )
        };
        let tile = Tile::new(&self.spec.borrow(), self.id as usize, cell, gap);
        tile.root.add_css_class("desk-tile");
        let t = tile.clone();
        layer::on_tap(&tile.root, move || t.activate());
        let me = Rc::downgrade(self);
        let anchor = tile.root.downgrade();
        layer::on_long_press(&tile.root, move |_, _| {
            if let (Some(me), Some(a)) = (me.upgrade(), anchor.upgrade()) {
                me.menu(&a);
            }
        });
        self.holder.append(&tile.root);
        let size = layer::natural_size(&self.holder);
        layer::apply_placement(&self.window, &self.placement.get(), size, 0, false);
    }

    fn menu(self: &Rc<Self>, anchor: &gtk::Overlay) {
        let pop = gtk::Popover::new();
        pop.set_has_arrow(false);
        pop.add_css_class("folio-menu");
        let col = gtk::Box::new(gtk::Orientation::Vertical, 0);
        col.append(&text("Sulla scrivania", &["serif", "menu-title"]));
        let me = Rc::downgrade(self);
        let add = |label: &str, g: Glyph, f: ItemAction| {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            row.append(&glyph::glyph(g, 18));
            row.append(&text(label, &["menu-label"]));
            let b = gtk::Button::builder().child(&row).build();
            b.add_css_class("btn");
            b.add_css_class("menu-item");
            let me = me.clone();
            let popw = pop.downgrade();
            b.connect_clicked(move |_| {
                if let Some(p) = popw.upgrade() {
                    p.popdown();
                }
                if let Some(m) = me.upgrade() {
                    f(&m);
                }
            });
            col.append(&b);
        };
        add(
            "Formato successivo",
            Glyph::Resize,
            Box::new(|m| {
                let next = m.spec.borrow().size.next();
                m.spec.borrow_mut().size = next;
                m.save();
                let m2 = m.clone();
                glib::idle_add_local_once(move || m2.render());
            }),
        );
        add(
            "Cambia inchiostro",
            Glyph::Palette,
            Box::new(|m| {
                let next = m.spec.borrow().style.next();
                m.spec.borrow_mut().style = next;
                m.save();
                let m2 = m.clone();
                glib::idle_add_local_once(move || m2.render());
            }),
        );
        add(
            "Impagina",
            Glyph::Layout,
            Box::new(|_| shell().command("layout show")),
        );
        add(
            "Togli dalla scrivania",
            Glyph::Close,
            Box::new(|m| {
                let id = m.id;
                glib::idle_add_local_once(move || {
                    if let Some(d) = shell().desk.get() {
                        d.remove(id);
                    }
                });
            }),
        );
        pop.set_child(Some(&col));
        pop.set_parent(anchor);
        pop.connect_closed(|p| {
            let p = p.clone();
            glib::idle_add_local_once(move || p.unparent());
        });
        pop.popup();
    }
}

impl Placeable for DeskItem {
    fn label(&self) -> String {
        let spec = self.spec.borrow();
        let name = spec
            .label
            .clone()
            .or_else(|| {
                spec.app
                    .as_deref()
                    .and_then(|a| shell().apps.borrow().resolve(a).map(|x| x.name.clone()))
            })
            .or_else(|| spec.live.map(|l| l.label().to_string()))
            .unwrap_or_else(|| "Widget".into());
        format!("Widget · {name}")
    }

    fn placement(&self) -> Placement {
        self.placement.get()
    }

    fn set_placement(&self, p: Placement) {
        // I widget restano liberi: niente aggancio con riserva di spazio.
        self.placement.set(p);
        self.save();
        let size = layer::natural_size(&self.holder);
        layer::apply_placement(&self.window, &p, size, 0, false);
    }

    fn size(&self) -> (i32, i32) {
        layer::natural_size(&self.holder)
    }

    fn margin(&self) -> i32 {
        0
    }

    fn reset(&self) {
        self.set_placement(Placement {
            edge: Edge::Float,
            x: 0.06,
            y: 0.16,
            stretch: false,
            vertical: false,
        });
    }
}

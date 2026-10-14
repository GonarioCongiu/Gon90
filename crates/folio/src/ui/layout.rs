//! Modalità Impaginazione: un menabò a schermo intero con la gabbia a 12
//! colonne. Ogni elemento (Testata, Dock, widget della Scrivania) diventa un
//! blocco tratteggiato da trascinare col dito ovunque: vicino a un bordo si
//! aggancia (e si orienta: verticale sui lati), altrove resta libero.

use super::{text, Placeable};
use crate::glyph::{self, Glyph};
use crate::layer::{self, Keyboard, Level};
use crate::shell::{shell, Overlay};
use folio_core::state::{Edge, Placement};
use gtk::glib;
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

const SNAP_EDGE: i32 = 44;
const SNAP_GRID: i32 = 12;
const MARGIN: f64 = 64.0;
const GUTTER: f64 = 24.0;

struct Block {
    widget: gtk::Box,
    size_label: gtk::Label,
    target: Rc<dyn Placeable>,
    rect: Cell<(i32, i32, i32, i32)>,
    start: Cell<(i32, i32)>,
}

pub struct LayoutEditor {
    window: gtk::Window,
    fixed: gtk::Fixed,
    guides: gtk::DrawingArea,
    guide: Rc<Cell<Option<(bool, f64)>>>,
    open: Cell<bool>,
    blocks: RefCell<Vec<Rc<Block>>>,
}

/// Bordi delle colonne della gabbia (x in px).
fn column_edges(width: i32) -> Vec<f64> {
    let col_w = (width as f64 - 2.0 * MARGIN - GUTTER * 11.0) / 12.0;
    (0..12)
        .flat_map(|i| {
            let x = MARGIN + i as f64 * (col_w + GUTTER);
            [x, x + col_w]
        })
        .collect()
}

impl LayoutEditor {
    pub fn new() -> Rc<Self> {
        let window = layer::surface("folio-layout", Level::Overlay, Keyboard::Exclusive);
        layer::fill_screen(&window);
        let overlay = gtk::Overlay::new();
        overlay.add_css_class("layout-scrim");
        let guide: Rc<Cell<Option<(bool, f64)>>> = Rc::new(Cell::new(None));

        let guides = gtk::DrawingArea::new();
        guides.add_css_class("layout-grid");
        guides.set_can_target(false);
        let g = guide.clone();
        guides.set_draw_func(move |a, cr, w, h| {
            let c = a.color();
            let (r, gg, b, al) = (
                c.red() as f64,
                c.green() as f64,
                c.blue() as f64,
                c.alpha() as f64,
            );
            // Colonne campite, come su un menabò.
            let col_w = (w as f64 - 2.0 * MARGIN - GUTTER * 11.0) / 12.0;
            cr.set_source_rgba(r, gg, b, al * 0.06);
            for i in 0..12 {
                cr.rectangle(MARGIN + i as f64 * (col_w + GUTTER), 0.0, col_w, h as f64);
            }
            let _ = cr.fill();
            // Righe di base ogni 24 px.
            cr.set_source_rgba(r, gg, b, al * 0.07);
            cr.set_line_width(1.0);
            let mut y = 24.0;
            while y < h as f64 {
                cr.move_to(0.0, y + 0.5);
                cr.line_to(w as f64, y + 0.5);
                y += 24.0;
            }
            let _ = cr.stroke();
            // Margini di aggancio.
            cr.set_source_rgba(r, gg, b, al * 0.35);
            cr.set_dash(&[4.0, 6.0], 0.0);
            let m = SNAP_EDGE as f64;
            cr.rectangle(m, m, w as f64 - 2.0 * m, h as f64 - 2.0 * m);
            let _ = cr.stroke();
            cr.set_dash(&[], 0.0);
            // Guida attiva (in colore d'accento tramite CSS: qui usiamo il colore pieno).
            if let Some((vertical, pos)) = g.get() {
                cr.set_source_rgba(r, gg, b, al);
                cr.set_line_width(1.5);
                if vertical {
                    cr.move_to(pos, 0.0);
                    cr.line_to(pos, h as f64);
                } else {
                    cr.move_to(0.0, pos);
                    cr.line_to(w as f64, pos);
                }
                let _ = cr.stroke();
            }
        });
        overlay.set_child(Some(&guides));

        let fixed = gtk::Fixed::new();
        overlay.add_overlay(&fixed);

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        header.add_css_class("layout-head");
        header.set_valign(gtk::Align::Center);
        header.set_halign(gtk::Align::Center);
        let titles = gtk::Box::new(gtk::Orientation::Vertical, 4);
        titles.append(&text("Impaginazione", &["serif", "layout-title"]));
        let sub = text(
            "Trascina gli elementi. Vicino a un bordo si agganciano; sui lati diventano verticali.",
            &["caption"],
        );
        sub.set_wrap(true);
        sub.set_max_width_chars(42);
        titles.append(&sub);
        header.append(&titles);
        let done = gtk::Button::with_label("Fine");
        done.add_css_class("btn");
        done.add_css_class("text-btn");
        done.add_css_class("accent");
        done.set_valign(gtk::Align::Center);
        done.connect_clicked(|_| shell().command("layout hide"));
        header.append(&done);
        overlay.add_overlay(&header);
        window.set_child(Some(&overlay));

        let ed = Rc::new(Self {
            window,
            fixed,
            guides,
            guide,
            open: Cell::new(false),
            blocks: RefCell::new(Vec::new()),
        });
        let w = Rc::downgrade(&ed);
        layer::close_on_escape(&ed.window, move || {
            if let Some(e) = w.upgrade() {
                e.close();
            }
        });
        ed
    }

    fn targets(&self) -> Vec<Rc<dyn Placeable>> {
        let s = shell();
        let mut v: Vec<Rc<dyn Placeable>> = Vec::new();
        if let Some(b) = s.bar.get() {
            v.push(b.clone());
        }
        if let Some(d) = s.dock.get() {
            v.push(d.clone());
        }
        if let Some(desk) = s.desk.get() {
            for item in desk.items() {
                v.push(item);
            }
        }
        v
    }

    fn build(&self) {
        while let Some(c) = self.fixed.first_child() {
            self.fixed.remove(&c);
        }
        self.blocks.borrow_mut().clear();
        for target in self.targets() {
            let block = self.block(target);
            self.place(&block);
            self.blocks.borrow_mut().push(block);
        }
    }

    fn place(&self, block: &Rc<Block>) {
        let t = &block.target;
        let size = t.size();
        let rect = layer::placement_rect(&t.placement(), size, t.margin());
        block.rect.set(rect);
        block
            .widget
            .set_size_request(rect.2.max(64), rect.3.max(40));
        block
            .size_label
            .set_text(&format!("{} × {}", rect.2, rect.3));
        if block.widget.parent().is_some() {
            self.fixed
                .move_(&block.widget, rect.0 as f64, rect.1 as f64);
        } else {
            self.fixed.put(&block.widget, rect.0 as f64, rect.1 as f64);
        }
    }

    fn block(&self, target: Rc<dyn Placeable>) -> Rc<Block> {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 4);
        widget.add_css_class("layout-block");
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.set_halign(gtk::Align::Center);
        row.set_valign(gtk::Align::Center);
        row.set_vexpand(true);
        row.append(&text(
            &target.label().to_uppercase(),
            &["kicker", "block-label"],
        ));
        let size_label = text("", &["mono", "block-size"]);
        row.append(&size_label);

        let reset = glyph::button(Glyph::Restart, 14, None);
        reset.add_css_class("block-chip");
        reset.set_tooltip_text(Some("Posizione originale"));
        row.append(&reset);
        let rotate = glyph::button(Glyph::Rotate, 14, None);
        rotate.add_css_class("block-chip");
        rotate.set_tooltip_text(Some("Orientamento (se libero)"));
        row.append(&rotate);
        let stretch = target.can_stretch().then(|| {
            let b = glyph::button(Glyph::Resize, 14, None);
            b.add_css_class("block-chip");
            b.set_tooltip_text(Some("Al vivo / compatta"));
            row.append(&b);
            b
        });
        widget.append(&row);

        let block = Rc::new(Block {
            widget,
            size_label,
            target,
            rect: Cell::new((0, 0, 0, 0)),
            start: Cell::new((0, 0)),
        });

        let b = Rc::downgrade(&block);
        reset.connect_clicked(move |_| {
            if let Some(b) = b.upgrade() {
                b.target.reset();
                refresh_soon(&b);
            }
        });
        let b = Rc::downgrade(&block);
        rotate.connect_clicked(move |_| {
            if let Some(b) = b.upgrade() {
                let mut p = b.target.placement();
                if p.edge == Edge::Float {
                    p.vertical = !p.vertical;
                    b.target.set_placement(p);
                    refresh_soon(&b);
                }
            }
        });
        if let Some(stretch) = stretch {
            let b = Rc::downgrade(&block);
            stretch.connect_clicked(move |_| {
                if let Some(b) = b.upgrade() {
                    let mut p = b.target.placement();
                    if p.edge != Edge::Float {
                        p.stretch = !p.stretch;
                        b.target.set_placement(p);
                        refresh_soon(&b);
                    }
                }
            });
        }

        // Trascinamento: il menabò è fermo e a tutto schermo, quindi le
        // coordinate sono stabili anche mentre il pannello vero si sposta.
        let drag = gtk::GestureDrag::new();
        let b = Rc::downgrade(&block);
        drag.connect_drag_begin(move |_, _, _| {
            if let Some(b) = b.upgrade() {
                let r = b.rect.get();
                b.start.set((r.0, r.1));
                b.widget.add_css_class("dragging");
            }
        });
        let b = Rc::downgrade(&block);
        drag.connect_drag_update(move |g, dx, dy| {
            let Some(b) = b.upgrade() else { return };
            if dx.abs() + dy.abs() > 6.0 {
                g.set_state(gtk::EventSequenceState::Claimed);
            }
            let Some(ed) = shell().layout.get().cloned() else {
                return;
            };
            let (mw, mh) = layer::monitor_size();
            let (sx, sy) = b.start.get();
            let (_, _, w, h) = b.rect.get();
            let mut x = (sx as f64 + dx).clamp(0.0, (mw - w).max(0) as f64);
            let y = (sy as f64 + dy).clamp(0.0, (mh - h).max(0) as f64);
            // Aggancio alla gabbia: bordo sinistro dell'elemento su un filo di colonna.
            ed.guide.set(None);
            for edge in column_edges(mw) {
                if (x - edge).abs() < SNAP_GRID as f64 {
                    x = edge;
                    ed.guide.set(Some((true, edge)));
                    break;
                }
            }
            if (x + w as f64 / 2.0 - mw as f64 / 2.0).abs() < SNAP_GRID as f64 {
                x = (mw - w) as f64 / 2.0;
                ed.guide.set(Some((true, mw as f64 / 2.0)));
            }
            b.rect.set((x as i32, y as i32, w, h));
            ed.fixed.move_(&b.widget, x, y);
            ed.guides.queue_draw();
        });
        let b = Rc::downgrade(&block);
        drag.connect_drag_end(move |_, dx, dy| {
            let Some(b) = b.upgrade() else { return };
            b.widget.remove_css_class("dragging");
            if let Some(ed) = shell().layout.get() {
                ed.guide.set(None);
                ed.guides.queue_draw();
            }
            if dx.abs() + dy.abs() < 6.0 {
                return;
            }
            let old = b.target.placement();
            let mon = layer::monitor_size();
            let mut p = Placement::from_rect(
                b.rect.get(),
                mon,
                SNAP_EDGE,
                old.is_vertical(),
                old.stretch && b.target.can_stretch(),
            );
            if p.edge == Edge::Float {
                p.stretch = false;
            }
            b.target.set_placement(p);
            refresh_soon(&b);
        });
        block.widget.add_controller(drag);
        block
    }
}

/// Dopo uno spostamento il pannello vero si reimpagina (può cambiare
/// orientamento e dimensione): il blocco si riallinea un istante dopo.
fn refresh_soon(block: &Rc<Block>) {
    let b = block.clone();
    glib::timeout_add_local_once(Duration::from_millis(140), move || {
        if let Some(ed) = shell().layout.get() {
            ed.place(&b);
        }
    });
}

impl Overlay for LayoutEditor {
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
        // In modalità transitoria la dock deve essere visibile per poterla spostare.
        if let Some(d) = shell().dock.get() {
            d.reveal();
        }
        layer::on_monitor(&self.window);
        self.window.present();
        self.build();
    }

    fn close(&self) {
        if !self.open.replace(false) {
            return;
        }
        self.window.set_visible(false);
        self.blocks.borrow_mut().clear();
        if let Some(d) = shell().dock.get() {
            d.autohide_soon();
        }
    }
}

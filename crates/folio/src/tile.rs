//! L'icona-widget: ogni app è un piccolo articolo vivo, in quattro formati
//! (piccolo 1×1, medio 2×2, largo 4×2, grande 4×4) e tre inchiostri
//! (carta, nero, colore d'accento) — tra le live tile di Windows 8 e i widget di iOS.

use crate::apps::App;
use crate::glyph::{self, Glyph};
use crate::shell::shell;
use folio_core::config::{TileSize, TileSpec};
use folio_core::live::{self, LiveContent, LiveCtx, LiveKind};
use folio_core::{locale, paths, sys};
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub struct Tile {
    pub root: gtk::Overlay,
    pub spec: TileSpec,
    pub app: Option<App>,
    pub kind: LiveKind,
}

struct Parts {
    kicker: gtk::Label,
    headline: gtk::Label,
    deck: gtk::Label,
    badge: gtk::Label,
    progress: gtk::DrawingArea,
    progress_value: Rc<RefCell<Option<f64>>>,
    spark: gtk::DrawingArea,
    series: Rc<RefCell<Vec<f64>>>,
    play: Option<gtk::Button>,
    playing: Cell<Option<bool>>,
}

pub fn tile_size(spec: &TileSpec, cell: i32, gap: i32) -> (i32, i32) {
    let (w, h) = spec.size.span();
    (
        w as i32 * cell + (w as i32 - 1) * gap,
        h as i32 * cell + (h as i32 - 1) * gap,
    )
}

fn label(css: &[&str], xalign: f32) -> gtk::Label {
    let l = gtk::Label::new(None);
    for c in css {
        l.add_css_class(c);
    }
    l.set_xalign(xalign);
    l.set_ellipsize(gtk::pango::EllipsizeMode::End);
    // Larghezza naturale minima: è la tile a decidere la misura, non il testo.
    l.set_max_width_chars(1);
    l.set_hexpand(true);
    l
}

/// Filetto di avanzamento a tutta larghezza (2 px).
fn hairline(value: Rc<RefCell<Option<f64>>>) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.set_content_height(2);
    area.set_hexpand(true);
    area.add_css_class("t-progress");
    area.set_draw_func(move |a, cr, w, h| {
        let Some(v) = *value.borrow() else { return };
        let c = a.color();
        cr.set_source_rgba(
            c.red() as f64,
            c.green() as f64,
            c.blue() as f64,
            c.alpha() as f64 * 0.18,
        );
        cr.rectangle(0.0, 0.0, w as f64, h as f64);
        let _ = cr.fill();
        cr.set_source_rgba(
            c.red() as f64,
            c.green() as f64,
            c.blue() as f64,
            c.alpha() as f64,
        );
        cr.rectangle(0.0, 0.0, w as f64 * v.clamp(0.0, 1.0), h as f64);
        let _ = cr.fill();
    });
    area
}

/// Sparkline editoriale: linea sottile, area tenue, punto finale.
fn sparkline(series: Rc<RefCell<Vec<f64>>>) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.set_content_height(56);
    area.set_hexpand(true);
    area.add_css_class("t-spark");
    area.set_draw_func(move |a, cr, w, h| {
        let data = series.borrow();
        if data.len() < 2 {
            return;
        }
        let c = a.color();
        let (w, h) = (w as f64, h as f64 - 4.0);
        let step = w / (data.len() - 1) as f64;
        let y = |v: f64| 2.0 + h - v.clamp(0.0, 1.0) * h;
        cr.move_to(0.0, y(data[0]));
        for (i, v) in data.iter().enumerate().skip(1) {
            cr.line_to(i as f64 * step, y(*v));
        }
        cr.set_source_rgba(
            c.red() as f64,
            c.green() as f64,
            c.blue() as f64,
            c.alpha() as f64,
        );
        cr.set_line_width(1.4);
        let _ = cr.stroke_preserve();
        cr.line_to(w, h + 2.0);
        cr.line_to(0.0, h + 2.0);
        cr.close_path();
        cr.set_source_rgba(
            c.red() as f64,
            c.green() as f64,
            c.blue() as f64,
            c.alpha() as f64 * 0.08,
        );
        let _ = cr.fill();
        let last = *data.last().unwrap_or(&0.0);
        cr.arc(w - 1.5, y(last), 2.5, 0.0, std::f64::consts::TAU);
        cr.set_source_rgba(
            c.red() as f64,
            c.green() as f64,
            c.blue() as f64,
            c.alpha() as f64,
        );
        let _ = cr.fill();
    });
    area
}

fn media_controls() -> (gtk::Box, gtk::Button) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    row.add_css_class("t-media");
    let make = |g: Glyph, action: &'static str| {
        let b = glyph::button(g, 18, None);
        b.connect_clicked(move |_| sys::media(action));
        b
    };
    let play = make(Glyph::Play, "play-pause");
    row.append(&make(Glyph::Prev, "previous"));
    row.append(&play);
    row.append(&make(Glyph::Next, "next"));
    (row, play)
}

impl Tile {
    /// Costruisce la tile. `number` è il numero di pagina stampato nell'angolo.
    pub fn new(spec: &TileSpec, number: usize, cell: i32, gap: i32) -> Rc<Tile> {
        let s = shell();
        let app = spec
            .app
            .as_deref()
            .and_then(|c| s.apps.borrow().resolve(c).cloned());
        let kind = spec.live.unwrap_or_else(|| match &app {
            Some(a) => live::guess(&a.id, &a.categories),
            None => LiveKind::Plain,
        });
        let (w, h) = tile_size(spec, cell, gap);

        let root = gtk::Overlay::new();
        root.set_size_request(w, h);
        root.set_halign(gtk::Align::Start);
        root.set_valign(gtk::Align::Start);
        root.add_css_class("tile");
        root.add_css_class(spec.size.css());
        root.add_css_class(spec.style.css());
        root.add_css_class(&format!("live-{:?}", kind).to_lowercase());
        root.set_overflow(gtk::Overflow::Hidden);

        let name = spec
            .label
            .clone()
            .or_else(|| app.as_ref().map(|a| a.name.clone()))
            .unwrap_or_else(|| kind.label().to_string());

        let parts = Parts {
            kicker: label(&["kicker", "t-kicker"], 0.0),
            headline: label(&["serif", "t-headline"], 0.0),
            deck: label(&["t-deck"], 0.0),
            badge: label(&["mono", "t-badge"], 1.0),
            progress_value: Rc::new(RefCell::new(None)),
            progress: gtk::DrawingArea::new(),
            series: Rc::new(RefCell::new(Vec::new())),
            spark: gtk::DrawingArea::new(),
            play: None,
            playing: Cell::new(None),
        };
        let mut parts = parts;
        parts.progress = hairline(parts.progress_value.clone());
        parts.spark = sparkline(parts.series.clone());

        let name_label = label(&["t-name"], 0.0);
        name_label.set_text(&name);
        let num = label(&["t-num"], 1.0);
        num.set_text(&locale::folio_number(number));

        let icon = |px: i32| -> gtk::Widget {
            match &app {
                Some(a) => a.image(px).upcast(),
                None => {
                    let g = match kind {
                        LiveKind::Music => Glyph::Play,
                        LiveKind::Weather => Glyph::Sun,
                        LiveKind::Battery => Glyph::Battery,
                        LiveKind::Network => Glyph::Wifi,
                        _ => Glyph::Grid,
                    };
                    glyph::glyph(g, px).upcast()
                }
            }
        };

        let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
        body.add_css_class("t-body");
        match spec.size {
            TileSize::Small => {
                // Icona al centro, nome sotto, distintivo vivo in alto a destra.
                let top = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                parts.badge.set_hexpand(true);
                top.append(&parts.badge);
                body.append(&top);
                let ic = icon((cell as f64 * 0.40) as i32);
                ic.set_vexpand(true);
                ic.set_valign(gtk::Align::Center);
                body.append(&ic);
                name_label.set_xalign(0.5);
                body.append(&name_label);
            }
            TileSize::Medium => {
                let top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                parts.kicker.set_hexpand(true);
                top.append(&parts.kicker);
                top.append(&icon(22));
                body.append(&top);
                parts.headline.set_vexpand(true);
                parts.headline.set_valign(gtk::Align::End);
                body.append(&parts.headline);
                body.append(&parts.deck);
                body.append(&parts.progress);
                let foot = gtk::Box::new(gtk::Orientation::Horizontal, 6);
                name_label.set_hexpand(true);
                foot.append(&name_label);
                foot.append(&num);
                foot.add_css_class("t-foot");
                body.append(&foot);
            }
            TileSize::Wide | TileSize::Large => {
                let top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                parts.kicker.set_hexpand(true);
                top.append(&parts.kicker);
                top.append(&icon(if spec.size == TileSize::Large { 32 } else { 26 }));
                body.append(&top);
                let middle = gtk::Box::new(gtk::Orientation::Vertical, 2);
                middle.set_vexpand(true);
                middle.set_valign(gtk::Align::End);
                if spec.size == TileSize::Large {
                    parts.headline.set_wrap(true);
                    parts.headline.set_lines(2);
                    parts.deck.set_wrap(true);
                    parts.deck.set_lines(3);
                }
                middle.append(&parts.headline);
                middle.append(&parts.deck);
                if kind == LiveKind::System && spec.size == TileSize::Wide {
                    // Nel formato largo il grafico sta accanto al titolo, come un'infografica.
                    let row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
                    row.set_vexpand(true);
                    middle.set_hexpand(true);
                    row.append(&middle);
                    parts.spark.set_size_request(w / 2 - 24, -1);
                    parts.spark.set_valign(gtk::Align::End);
                    row.append(&parts.spark);
                    body.append(&row);
                } else {
                    body.append(&middle);
                    if kind == LiveKind::System {
                        body.append(&parts.spark);
                    }
                }
                if kind == LiveKind::Music {
                    let (row, play) = media_controls();
                    body.append(&row);
                    parts.play = Some(play);
                }
                body.append(&parts.progress);
                let foot = gtk::Box::new(gtk::Orientation::Horizontal, 6);
                name_label.set_hexpand(true);
                foot.append(&name_label);
                foot.append(&num);
                foot.add_css_class("t-foot");
                body.append(&foot);
            }
        }
        root.set_child(Some(&body));

        let tile = Rc::new(Tile {
            root,
            spec: spec.clone(),
            app,
            kind,
        });
        let parts = Rc::new(parts);

        // Riempimento + abbonamento agli aggiornamenti. Le tile nascoste non
        // vengono ricalcolate finché non tornano visibili.
        let weak_root = tile.root.downgrade();
        let app_for_live = tile.app.clone();
        let filled = Rc::new(Cell::new(false));
        let update: Rc<dyn Fn() -> bool> = Rc::new(move || {
            let Some(root) = weak_root.upgrade() else {
                return false;
            };
            if filled.get() && !root.is_mapped() {
                return true;
            }
            filled.set(true);
            apply(&parts, &compute(kind, app_for_live.as_ref(), &name));
            true
        });
        update();
        let u = update.clone();
        shell().live.subscribe(kind, move || u());
        tile.root.connect_map(move |_| {
            update();
        });
        tile
    }

    /// Azione principale: apre l'app (o mette in pausa la musica se non c'è app).
    pub fn activate(&self) {
        match &self.app {
            Some(app) => shell().open_app(app),
            None if self.kind == LiveKind::Music => sys::media("play-pause"),
            None => {}
        }
    }
}

fn compute(kind: LiveKind, app: Option<&App>, name: &str) -> LiveContent {
    let s = shell();
    let windows = app.map(|a| s.windows_of(a).len()).unwrap_or(0);
    let notes = if kind == LiveKind::Notes {
        let file = s.config.borrow().home.notes_file.clone();
        std::fs::read_to_string(paths::expand(&file)).ok()
    } else {
        None
    };
    let sys = s.sys.borrow();
    let ctx = LiveCtx {
        now: chrono::Local::now(),
        sys: &sys,
        windows,
        app_name: name,
        description: app.map(|a| a.description.as_str()).unwrap_or(""),
        notes: notes.as_deref(),
        seconds: false,
    };
    live::compute(kind, &ctx)
}

fn set_text(l: &gtk::Label, text: &str) {
    if l.text() != text {
        l.set_text(text);
    }
    l.set_visible(!text.is_empty());
}

fn apply(p: &Parts, c: &LiveContent) {
    set_text(&p.kicker, &c.kicker);
    set_text(&p.headline, &c.headline);
    set_text(&p.deck, &c.deck);
    set_text(&p.badge, c.badge.as_deref().unwrap_or(""));
    let changed = *p.progress_value.borrow() != c.progress;
    if changed {
        *p.progress_value.borrow_mut() = c.progress;
        p.progress.queue_draw();
    }
    p.progress.set_visible(c.progress.is_some());
    if *p.series.borrow() != c.series {
        *p.series.borrow_mut() = c.series.clone();
        p.spark.queue_draw();
    }
    if let Some(play) = &p.play {
        let playing = c.kicker == "IN RIPRODUZIONE";
        if p.playing.replace(Some(playing)) != Some(playing) {
            let g = if playing { Glyph::Pause } else { Glyph::Play };
            play.set_child(Some(&glyph::glyph(g, 18)));
        }
    }
}

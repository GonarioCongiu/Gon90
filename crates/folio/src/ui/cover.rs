//! La Copertina: lo sfondo del desktop impaginato come la copertina di una
//! rivista — testata, numero, data in corpo enorme, sommario a piè di pagina.
//! Vive sul livello `background` e si aggiorna ogni minuto.

use super::{rule, text};
use crate::layer::{self, Keyboard, Level};
use crate::shell::shell;
use folio_core::{locale, paths};
use gtk::pango;
use gtk::prelude::*;
use std::rc::Rc;

pub struct Cover {
    window: gtk::Window,
    issue: gtk::Label,
    edition: gtk::Label,
    day: gtk::Label,
    month: gtk::Label,
    weekday: gtk::Label,
    footer_left: gtk::Label,
    footer_right: gtk::Label,
    grid: gtk::DrawingArea,
    story_kicker: gtk::Label,
    story: gtk::Label,
    story_deck: gtk::Label,
    page: gtk::Box,
}

/// Citazioni per i giorni senza "notizie" (nessuna finestra aperta).
const QUOTES: [(&str, &str); 6] = [
    (
        "Complicare è facile, semplificare è difficile.",
        "Bruno Munari",
    ),
    ("Meno, ma meglio.", "Dieter Rams"),
    (
        "Se sai progettare una cosa, sai progettare tutto.",
        "Massimo Vignelli",
    ),
    ("Dal cucchiaio alla città.", "Ernesto Nathan Rogers"),
    ("La forma segue la funzione.", "Louis Sullivan"),
    ("Il buon design è il minor design possibile.", "Dieter Rams"),
];

fn size_px(label: &gtk::Label, px: f64) {
    let attrs = pango::AttrList::new();
    attrs.insert(pango::AttrSize::new_size_absolute(
        (px * pango::SCALE as f64) as i32,
    ));
    attrs.insert(pango::AttrInt::new_letter_spacing(
        (-px * 0.02 * pango::SCALE as f64) as i32,
    ));
    label.set_attributes(Some(&attrs));
}

impl Cover {
    pub fn new() -> Rc<Self> {
        let window = layer::surface("folio-cover", Level::Background, Keyboard::None);
        layer::fill_screen(&window);
        let cfg = shell().config.borrow().cover.clone();

        let overlay = gtk::Overlay::new();
        overlay.add_css_class("cover");

        let base = gtk::Box::new(gtk::Orientation::Vertical, 0);
        base.add_css_class("cover-paper");
        if let Some(img) = cfg.image.as_deref().filter(|s| !s.is_empty()) {
            let pic = gtk::Picture::for_filename(paths::expand(img));
            pic.set_content_fit(gtk::ContentFit::Cover);
            pic.set_vexpand(true);
            pic.add_css_class("cover-image");
            base.append(&pic);
        }
        overlay.set_child(Some(&base));

        // Gabbia a 12 colonne appena accennata, come su un menabò.
        let grid = gtk::DrawingArea::new();
        grid.set_can_target(false);
        grid.add_css_class("cover-grid");
        let show_grid = cfg.show_grid;
        grid.set_draw_func(move |a, cr, w, h| {
            if !show_grid {
                return;
            }
            let c = a.color();
            cr.set_source_rgba(
                c.red() as f64,
                c.green() as f64,
                c.blue() as f64,
                c.alpha() as f64,
            );
            cr.set_line_width(1.0);
            let margin = 64.0;
            let gutter = 24.0;
            let cols = 12.0;
            let col_w = (w as f64 - 2.0 * margin - gutter * (cols - 1.0)) / cols;
            for i in 0..12 {
                let x = margin + i as f64 * (col_w + gutter);
                for xx in [x, x + col_w] {
                    cr.move_to(xx.round() + 0.5, 0.0);
                    cr.line_to(xx.round() + 0.5, h as f64);
                }
            }
            let _ = cr.stroke();
        });
        overlay.add_overlay(&grid);

        let page = gtk::Box::new(gtk::Orientation::Vertical, 0);
        page.add_css_class("cover-page");
        let top = gtk::Box::new(gtk::Orientation::Horizontal, 24);
        let masthead = text(&cfg.masthead, &["serif", "cover-masthead"]);
        masthead.set_xalign(0.0);
        masthead.set_hexpand(true);
        top.append(&masthead);
        let meta = gtk::Box::new(gtk::Orientation::Vertical, 4);
        meta.set_valign(gtk::Align::End);
        let issue = text("", &["kicker", "cover-issue"]);
        issue.set_xalign(1.0);
        let edition = text("", &["kicker", "cover-edition"]);
        edition.set_xalign(1.0);
        meta.append(&issue);
        meta.append(&edition);
        top.append(&meta);
        page.append(&top);
        page.append(&rule(false));
        let tagline = text(&cfg.tagline.to_uppercase(), &["kicker", "cover-tagline"]);
        tagline.set_xalign(0.0);
        page.append(&tagline);

        // Articolo di copertina: la finestra in primo piano o una citazione.
        let story_row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        story_row.set_vexpand(true);
        let left_space = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        left_space.set_hexpand(true);
        story_row.append(&left_space);
        let story_box = gtk::Box::new(gtk::Orientation::Vertical, 10);
        story_box.add_css_class("cover-story");
        story_box.set_valign(gtk::Align::Center);
        let story_kicker = text("", &["kicker", "cover-story-kicker"]);
        story_kicker.set_xalign(0.0);
        let story = text("", &["serif", "cover-story-title"]);
        story.set_xalign(0.0);
        story.set_wrap(true);
        story.set_lines(3);
        story.set_ellipsize(pango::EllipsizeMode::End);
        story.set_max_width_chars(26);
        let story_deck = text("", &["cover-story-deck"]);
        story_deck.set_xalign(0.0);
        story_box.append(&story_kicker);
        story_box.append(&story);
        story_box.append(&story_deck);
        story_row.append(&story_box);
        page.append(&story_row);

        let date_row = gtk::Box::new(gtk::Orientation::Horizontal, 28);
        let day = text("", &["serif", "cover-day"]);
        day.set_valign(gtk::Align::Baseline);
        date_row.append(&day);
        let words = gtk::Box::new(gtk::Orientation::Vertical, 0);
        words.set_valign(gtk::Align::End);
        let month = text("", &["serif", "cover-month"]);
        month.set_xalign(0.0);
        let weekday = text("", &["serif", "cover-weekday"]);
        weekday.set_xalign(0.0);
        words.append(&month);
        words.append(&weekday);
        date_row.append(&words);
        page.append(&date_row);
        page.append(&rule(false));
        let foot = gtk::Box::new(gtk::Orientation::Horizontal, 24);
        foot.add_css_class("cover-foot");
        let footer_left = text("", &["kicker"]);
        footer_left.set_xalign(0.0);
        footer_left.set_hexpand(true);
        let footer_right = text("", &["kicker"]);
        footer_right.set_xalign(1.0);
        foot.append(&footer_left);
        foot.append(&footer_right);
        page.append(&foot);
        overlay.add_overlay(&page);
        window.set_child(Some(&overlay));

        // Corpi tipografici proporzionati allo schermo.
        let (_, mh) = layer::monitor_size();
        let unit = mh as f64 / 1080.0;
        size_px(&masthead, 132.0 * unit);
        let cover = Rc::new(Self {
            window,
            issue,
            edition,
            day,
            month,
            weekday,
            footer_left,
            footer_right,
            grid,
            story_kicker,
            story,
            story_deck,
            page,
        });
        size_px(&cover.story, 58.0 * unit);
        size_px(&cover.day, 300.0 * unit);
        size_px(&cover.month, 92.0 * unit);
        size_px(&cover.weekday, 44.0 * unit);
        cover.update();
        layer::on_monitor(&cover.window);
        cover.window.present();
        cover
    }

    /// La carta copre tutto lo schermo; il testo resta nella "pagina" lasciata
    /// libera da Testata e Dock, ovunque siano state impaginate.
    fn update_insets(&self) {
        let inset = layer::reserved_insets();
        self.page.set_margin_top(inset[0]);
        self.page.set_margin_bottom(inset[1]);
        self.page.set_margin_start(inset[2]);
        self.page.set_margin_end(inset[3]);
    }

    pub fn update(&self) {
        self.update_insets();
        let now = chrono::Local::now();
        let s = shell();
        self.issue.set_text(&locale::issue(&now).to_uppercase());
        self.edition.set_text(&locale::edition(&now).to_uppercase());
        self.day.set_text(&now.format("%-d").to_string());
        self.month.set_text(locale::month(&now));
        self.weekday.set_text(locale::weekday(&now));
        let snap = s.hypr.borrow();
        let pages: Vec<i64> = snap.normal_workspaces().iter().map(|w| w.id).collect();
        let active = snap.active_workspace();
        let pos = pages
            .iter()
            .position(|&p| p == active)
            .map(|p| p + 1)
            .unwrap_or(1);
        let n = snap.windows_on(active).len();
        self.footer_left.set_text(
            &format!(
                "Spazio {} di {} — {}",
                locale::folio_number(pos),
                locale::folio_number(pages.len().max(1)),
                if n == 0 {
                    "pagina bianca".to_string()
                } else {
                    locale::plural(n, "finestra", "finestre")
                }
            )
            .to_uppercase(),
        );
        let sys = s.sys.borrow();
        let weather = sys
            .weather
            .as_ref()
            .map(|w| format!("{} {} · {}", w.place, w.temp, w.condition))
            .unwrap_or_default();
        self.footer_right.set_text(&weather.to_uppercase());
        drop(sys);
        match snap.active.as_ref().filter(|a| !a.title.is_empty()) {
            Some(a) => {
                let app = s
                    .apps
                    .borrow()
                    .for_class(a.app_class())
                    .map(|x| x.name.clone())
                    .unwrap_or_else(|| a.app_class().to_string());
                self.story_kicker.set_text("IN PRIMO PIANO");
                self.story.set_text(&a.title);
                self.story_deck.set_text(&format!("— {app}"));
            }
            None => {
                let (quote, author) =
                    QUOTES[chrono::Datelike::ordinal(&now) as usize % QUOTES.len()];
                self.story_kicker.set_text("LA CITAZIONE DEL GIORNO");
                self.story.set_text(&format!("«{quote}»"));
                self.story_deck.set_text(&format!("— {author}"));
            }
        }
        self.grid.queue_draw();
    }
}

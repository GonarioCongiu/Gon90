//! Tipografia e palette. Il foglio di stile statico (`style.css`) usa solo
//! colori simbolici (`@paper`, `@ink`, `@accent`…): qui li definiamo per
//! l'edizione chiara o scura e aggiungiamo le famiglie di caratteri scelte.
//! `~/.config/folio/style.css` viene caricato per ultimo e può ridefinire tutto.

use folio_core::config::{Config, IconStyle};
use folio_core::paths;
use gtk::gdk;
use std::cell::Cell;

const STYLE: &str = include_str!("style.css");

pub struct Theme {
    base: gtk::CssProvider,
    user: gtk::CssProvider,
    dark: Cell<Option<bool>>,
    installed: Cell<bool>,
}

#[derive(Clone, Copy)]
struct Rgb(f64, f64, f64);

impl Rgb {
    fn parse(hex: &str) -> Option<Self> {
        let h = hex.trim().trim_start_matches('#');
        if h.len() != 6 {
            return None;
        }
        let v = |i: usize| {
            u8::from_str_radix(&h[i..i + 2], 16)
                .ok()
                .map(|b| b as f64 / 255.0)
        };
        Some(Rgb(v(0)?, v(2)?, v(4)?))
    }

    fn mix(self, other: Rgb, t: f64) -> Rgb {
        Rgb(
            self.0 + (other.0 - self.0) * t,
            self.1 + (other.1 - self.1) * t,
            self.2 + (other.2 - self.2) * t,
        )
    }

    fn hex(self) -> String {
        let c = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        format!("#{:02x}{:02x}{:02x}", c(self.0), c(self.1), c(self.2))
    }

    fn luminance(self) -> f64 {
        0.2126 * self.0 + 0.7152 * self.1 + 0.0722 * self.2
    }
}

fn quote_family(f: &str) -> String {
    format!("\"{}\"", f.replace('"', ""))
}

pub fn palette_css(cfg: &Config, dark: bool) -> String {
    let t = &cfg.theme;
    let fallback_paper = if dark {
        Rgb(0.07, 0.07, 0.067)
    } else {
        Rgb(0.953, 0.941, 0.910)
    };
    let fallback_ink = if dark {
        Rgb(0.925, 0.910, 0.871)
    } else {
        Rgb(0.078, 0.078, 0.075)
    };
    let (paper_hex, ink_hex) = if dark {
        (&t.paper_dark, &t.ink_dark)
    } else {
        (&t.paper, &t.ink)
    };
    let paper = Rgb::parse(paper_hex).unwrap_or(fallback_paper);
    let ink = Rgb::parse(ink_hex).unwrap_or(fallback_ink);
    let accent = Rgb::parse(&t.accent).unwrap_or(Rgb(0.83, 0.27, 0.16));
    let on_accent = if accent.luminance() > 0.55 {
        Rgb(0.08, 0.08, 0.08)
    } else {
        Rgb(1.0, 0.98, 0.96)
    };

    let paper_2 = paper.mix(ink, if dark { 0.06 } else { 0.045 });
    let paper_3 = paper.mix(ink, if dark { 0.12 } else { 0.09 });
    let ink_2 = ink.mix(paper, 0.42);
    let ink_3 = ink.mix(paper, 0.62);

    let display = format!(
        "{}, \"Instrument Serif\", \"IBM Plex Serif\", \"Noto Serif\", serif",
        quote_family(&t.font_display)
    );
    let text = format!(
        "{}, \"Inter\", \"Noto Sans\", sans-serif",
        quote_family(&t.font_text)
    );
    let mono = format!(
        "{}, \"JetBrains Mono\", \"IBM Plex Mono\", monospace",
        quote_family(&t.font_mono)
    );
    let radius = t.radius;
    let icon_filter = match t.icons {
        IconStyle::Mono => "grayscale(1) contrast(1.08)",
        IconStyle::Color => "none",
    };

    format!(
        "/* generato da Folio — edizione {edition} */
@define-color paper {paper};
@define-color paper_2 {paper_2};
@define-color paper_3 {paper_3};
@define-color ink {ink};
@define-color ink_2 {ink_2};
@define-color ink_3 {ink_3};
@define-color accent {accent};
@define-color on_accent {on_accent};
@define-color rule alpha({ink}, 0.14);
@define-color rule_strong alpha({ink}, 0.32);
@define-color shadow alpha(#000000, {shadow});
@define-color scrim {paper};

.folio {{ font-family: {text}; }}
.serif {{ font-family: {display}; font-weight: 400; }}
.mono, .kicker, .t-num {{ font-family: {mono}; }}
.tile, .dock, .chrome-card, .shade-panel, .toast-card, .layout-block, .ws-thumb, .ov-card, .qs-tile {{ border-radius: {radius}px; }}
.app-icon {{ filter: {icon_filter}; }}
.app-icon.live, .dock-item.active .app-icon, .dock-item:active .app-icon {{ filter: none; }}
",
        edition = if dark { "serale" } else { "diurna" },
        paper = paper.hex(),
        paper_2 = paper_2.hex(),
        paper_3 = paper_3.hex(),
        ink = ink.hex(),
        ink_2 = ink_2.hex(),
        ink_3 = ink_3.hex(),
        accent = accent.hex(),
        on_accent = on_accent.hex(),
        shadow = if dark { "0.55" } else { "0.16" },
    )
}

impl Theme {
    pub fn new() -> Self {
        Self {
            base: gtk::CssProvider::new(),
            user: gtk::CssProvider::new(),
            dark: Cell::new(None),
            installed: Cell::new(false),
        }
    }

    pub fn is_dark(&self) -> bool {
        self.dark.get().unwrap_or(false)
    }

    pub fn invalidate(&self) {
        self.dark.set(None);
    }

    /// Applica palette e stile; restituisce `true` se qualcosa è cambiato.
    pub fn apply(&self, cfg: &Config, dark: bool) -> bool {
        if self.dark.get() == Some(dark) {
            return false;
        }
        self.dark.set(Some(dark));
        let Some(display) = gdk::Display::default() else {
            return false;
        };
        if !self.installed.replace(true) {
            gtk::style_context_add_provider_for_display(
                &display,
                &self.base,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
            gtk::style_context_add_provider_for_display(
                &display,
                &self.user,
                gtk::STYLE_PROVIDER_PRIORITY_USER,
            );
        }
        if let Some(settings) = gtk::Settings::default() {
            settings.set_gtk_application_prefer_dark_theme(dark);
        }
        self.base
            .load_from_string(&format!("{}\n{}", palette_css(cfg, dark), STYLE));
        let user_css =
            std::fs::read_to_string(paths::config_dir().join("style.css")).unwrap_or_default();
        self.user.load_from_string(&user_css);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixes_colors() {
        let a = Rgb::parse("#000000").unwrap();
        let b = Rgb::parse("#ffffff").unwrap();
        assert_eq!(a.mix(b, 0.5).hex(), "#808080");
        assert!(palette_css(&Config::default(), true).contains("@define-color paper #121211;"));
    }
}

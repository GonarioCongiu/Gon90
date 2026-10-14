//! Catalogo delle applicazioni installate (file .desktop) e abbinamento
//! con le finestre di Hyprland.

use gtk::prelude::*;
use gtk::{gdk, gio};
use std::collections::HashMap;

#[derive(Clone)]
pub struct App {
    pub id: String,
    pub name: String,
    pub description: String,
    pub categories: String,
    pub icon: Option<gio::Icon>,
    pub info: gio_unix::DesktopAppInfo,
}

impl App {
    pub fn launch(&self) {
        let ctx = gdk::Display::default().map(|d| d.app_launch_context());
        if let Err(e) = self.info.launch(&[], ctx.as_ref()) {
            log::warn!("impossibile avviare {}: {e}", self.id);
        }
    }

    /// Immagine dell'icona; `.app-icon` permette lo stile monocromatico.
    pub fn image(&self, px: i32) -> gtk::Image {
        let img = match &self.icon {
            Some(icon) => gtk::Image::from_gicon(icon),
            None => gtk::Image::from_icon_name("application-x-executable"),
        };
        img.set_pixel_size(px);
        img.add_css_class("app-icon");
        img
    }

    /// Lettera dell'indice A–Z.
    pub fn letter(&self) -> char {
        self.name
            .chars()
            .find(|c| c.is_alphanumeric())
            .map(|c| {
                if c.is_ascii_digit() {
                    '#'
                } else {
                    c.to_uppercase().next().unwrap_or(c)
                }
            })
            .unwrap_or('#')
    }

    pub fn matches(&self, query: &str) -> bool {
        let q = query.to_lowercase();
        self.name.to_lowercase().contains(&q)
            || self.description.to_lowercase().contains(&q)
            || self.id.to_lowercase().contains(&q)
            || self
                .info
                .keywords()
                .iter()
                .any(|k| k.to_lowercase().contains(&q))
    }
}

#[derive(Default)]
pub struct Catalog {
    pub apps: Vec<App>,
    by_id: HashMap<String, usize>,
    by_key: HashMap<String, usize>,
}

fn norm(s: &str) -> String {
    s.trim().trim_end_matches(".desktop").to_lowercase()
}

impl Catalog {
    pub fn load() -> Self {
        let mut apps: Vec<App> = gio::AppInfo::all()
            .into_iter()
            .filter(|a| a.should_show())
            .filter_map(|a| a.downcast::<gio_unix::DesktopAppInfo>().ok())
            .filter_map(|info| {
                let id = info.id()?.to_string();
                Some(App {
                    name: info.name().to_string(),
                    description: info
                        .description()
                        .map(|d| d.to_string())
                        .unwrap_or_default(),
                    categories: info.categories().map(|c| c.to_string()).unwrap_or_default(),
                    icon: info.icon(),
                    id,
                    info,
                })
            })
            .collect();
        apps.sort_by_key(|a| a.name.to_lowercase());
        apps.dedup_by(|a, b| a.id == b.id);

        let mut by_id = HashMap::new();
        let mut by_key = HashMap::new();
        for (i, app) in apps.iter().enumerate() {
            by_id.insert(app.id.clone(), i);
            let mut keys = vec![norm(&app.id)];
            if let Some(last) = norm(&app.id).rsplit('.').next() {
                keys.push(last.to_string());
            }
            if let Some(wm) = app.info.startup_wm_class() {
                keys.push(norm(&wm));
            }
            if let Some(exe) = app.info.executable().file_name() {
                keys.push(norm(&exe.to_string_lossy()));
            }
            keys.push(norm(&app.name));
            for k in keys {
                by_key.entry(k).or_insert(i);
            }
        }
        Self {
            apps,
            by_id,
            by_key,
        }
    }

    pub fn get(&self, id: &str) -> Option<&App> {
        self.by_id
            .get(id)
            .or_else(|| self.by_id.get(&format!("{id}.desktop")))
            .map(|&i| &self.apps[i])
    }

    /// Risolve "a.desktop|b.desktop|c.desktop": vince il primo installato.
    pub fn resolve(&self, candidates: &str) -> Option<&App> {
        candidates
            .split('|')
            .map(str::trim)
            .filter(|c| !c.is_empty())
            .find_map(|c| self.get(c))
    }

    /// App a cui appartiene una finestra, a partire dalla sua classe Wayland/X11.
    pub fn for_class(&self, class: &str) -> Option<&App> {
        if class.is_empty() {
            return None;
        }
        let k = norm(class);
        self.by_key
            .get(&k)
            .or_else(|| k.rsplit('.').next().and_then(|last| self.by_key.get(last)))
            .map(|&i| &self.apps[i])
    }
}

/// Icona per una finestra anche se non appartiene a nessuna app nota.
pub fn icon_for_class(catalog: &Catalog, class: &str, px: i32) -> gtk::Image {
    match catalog.for_class(class) {
        Some(app) => app.image(px),
        None => {
            let theme = gdk::Display::default().map(|d| gtk::IconTheme::for_display(&d));
            let lower = class.to_lowercase();
            let name = if theme.map(|t| t.has_icon(&lower)).unwrap_or(false) {
                lower
            } else {
                "application-x-executable".to_string()
            };
            let img = gtk::Image::from_icon_name(&name);
            img.set_pixel_size(px);
            img.add_css_class("app-icon");
            img
        }
    }
}

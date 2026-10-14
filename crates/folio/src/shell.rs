//! L'orchestratore: possiede configurazione, stato, dati di sistema e di
//! Hyprland, e coordina tutte le superfici. Vive per tutta la sessione e
//! si raggiunge da qualunque callback con `shell()`.

use crate::apps::{App, Catalog};
use crate::livehub::LiveHub;
use crate::osk::Osk;
use crate::theme::Theme;
use crate::ui::{
    bar::Bar, chrome::Chrome, cover::Cover, desk::Desk, dock::Dock, home::Home,
    layout::LayoutEditor, notify::Notifier, overview::Overview, shade::Shade,
};
use folio_core::config::{Config, ThemeMode};
use folio_core::gesture::{self, Outcome};
use folio_core::hypr::{self, Client, Dispatch, Snapshot, MINIMIZED};
use folio_core::ipc;
use folio_core::state::State;
use folio_core::sys::{self, SysSnapshot};
use gtk::prelude::*;
use gtk::{gio, glib};
use std::cell::{Cell, OnceCell, RefCell};
use std::rc::Rc;
use std::time::Duration;

thread_local! {
    static SHELL: OnceCell<Rc<Shell>> = const { OnceCell::new() };
}

/// Accesso globale (solo dal thread GTK).
pub fn shell() -> Rc<Shell> {
    SHELL.with(|s| s.get().expect("Folio non ancora avviato").clone())
}

pub fn try_shell() -> Option<Rc<Shell>> {
    SHELL.with(|s| s.get().cloned())
}

pub struct Shell {
    pub app: gtk::Application,
    pub config: RefCell<Config>,
    pub state: RefCell<State>,
    pub theme: Theme,
    pub apps: RefCell<Catalog>,
    pub hypr: RefCell<Snapshot>,
    pub sys: RefCell<SysSnapshot>,
    pub live: LiveHub,
    pub osk: Osk,
    refresh_pending: Cell<bool>,
    last_minute: Cell<u32>,

    pub bar: OnceCell<Rc<Bar>>,
    pub dock: OnceCell<Rc<Dock>>,
    pub home: OnceCell<Rc<Home>>,
    pub overview: OnceCell<Rc<Overview>>,
    pub chrome: OnceCell<Rc<Chrome>>,
    pub shade: OnceCell<Rc<Shade>>,
    pub notifier: OnceCell<Rc<Notifier>>,
    pub cover: OnceCell<Rc<Cover>>,
    pub desk: OnceCell<Rc<Desk>>,
    pub layout: OnceCell<Rc<LayoutEditor>>,
}

pub fn start(app: &gtk::Application) {
    if try_shell().is_some() {
        return;
    }
    // La shell non ha finestre "normali": la teniamo viva esplicitamente.
    std::mem::forget(app.hold());

    if !gtk4_layer_shell::is_supported() {
        log::error!("Il compositor non supporta wlr-layer-shell: Folio richiede Hyprland (o un compositor wlroots).");
    }

    let config = Config::load();
    let state = State::load();
    let me = Rc::new(Shell {
        app: app.clone(),
        osk: Osk::new(&config.osk),
        config: RefCell::new(config),
        state: RefCell::new(state),
        theme: Theme::new(),
        apps: RefCell::new(Catalog::load()),
        hypr: RefCell::new(Snapshot::default()),
        sys: RefCell::new(SysSnapshot::default()),
        live: LiveHub::default(),
        refresh_pending: Cell::new(false),
        last_minute: Cell::new(u32::MAX),
        bar: OnceCell::new(),
        dock: OnceCell::new(),
        home: OnceCell::new(),
        overview: OnceCell::new(),
        chrome: OnceCell::new(),
        shade: OnceCell::new(),
        notifier: OnceCell::new(),
        cover: OnceCell::new(),
        desk: OnceCell::new(),
        layout: OnceCell::new(),
    });
    SHELL.with(|s| {
        let _ = s.set(me.clone());
    });

    me.apply_theme();

    // ── Socket di comando ──
    let (tx, rx) = async_channel::unbounded::<String>();
    match ipc::serve(move |line| {
        if line.is_empty() {
            return "errore: comando vuoto".into();
        }
        match tx.send_blocking(line) {
            Ok(()) => "ok".into(),
            Err(_) => "errore: shell in chiusura".into(),
        }
    }) {
        Ok(()) => {}
        Err(ipc::ServeError::AlreadyRunning) => {
            log::warn!("Folio è già in esecuzione: esco.");
            app.quit();
            return;
        }
        Err(e) => log::error!("socket di comando non disponibile: {e}"),
    }
    glib::spawn_future_local(async move {
        while let Ok(line) = rx.recv().await {
            shell().command(&line);
        }
    });

    // ── Eventi di Hyprland ──
    let (htx, hrx) = async_channel::unbounded::<hypr::Event>();
    if hypr::listen(move |ev| {
        if ev.is_relevant() {
            let _ = htx.send_blocking(ev);
        }
    })
    .is_none()
    {
        log::warn!("Hyprland non rilevato: spazi di lavoro e finestre non saranno disponibili.");
    }
    glib::spawn_future_local(async move {
        while let Ok(ev) = hrx.recv().await {
            shell().on_hypr_event(&ev);
        }
    });

    // ── Dati di sistema ──
    let (stx, srx) = async_channel::unbounded::<SysSnapshot>();
    {
        let cfg = me.config.borrow();
        let weather = cfg
            .weather
            .enabled
            .then(|| (cfg.weather.location.clone(), cfg.weather.refresh_min));
        sys::spawn_monitor(
            sys::MonitorOptions {
                weather,
                interval: Duration::from_secs(2),
            },
            move |snap| {
                let _ = stx.send_blocking(snap);
            },
        );
    }
    glib::spawn_future_local(async move {
        while let Ok(snap) = srx.recv().await {
            let s = shell();
            *s.sys.borrow_mut() = snap;
            s.live.refresh(false);
            if let Some(shade) = s.shade.get() {
                shade.sync();
            }
            if let Some(bar) = s.bar.get() {
                bar.update_status();
            }
        }
    });

    // ── App installate: si aggiorna quando cambia un .desktop ──
    let monitor = gio::AppInfoMonitor::get();
    monitor.connect_changed(|_| {
        let s = shell();
        *s.apps.borrow_mut() = Catalog::load();
        if let Some(home) = s.home.get() {
            home.invalidate();
        }
        if let Some(dock) = s.dock.get() {
            dock.rebuild();
        }
    });
    // Il monitor deve restare vivo per tutta la sessione.
    std::mem::forget(monitor);

    me.build_surfaces();
    me.refresh_hypr();

    glib::timeout_add_seconds_local(1, || {
        shell().tick();
        glib::ControlFlow::Continue
    });
}

impl Shell {
    fn build_surfaces(self: &Rc<Self>) {
        let cfg = self.config.borrow().clone();
        if cfg.cover.enabled {
            let _ = self.cover.set(Cover::new());
        }
        let _ = self.desk.set(Desk::new());
        if cfg.bar.enabled {
            let _ = self.bar.set(Bar::new());
        }
        if cfg.dock.enabled {
            let _ = self.dock.set(Dock::new());
        }
        let _ = self.home.set(Home::new());
        let _ = self.overview.set(Overview::new());
        let _ = self.chrome.set(Chrome::new());
        let _ = self.shade.set(Shade::new());
        let _ = self.layout.set(LayoutEditor::new());
        if cfg.notifications.enabled {
            let _ = self.notifier.set(Notifier::new());
        }
    }

    // ───────────────────────────── Tema ─────────────────────────────

    pub fn is_dark(&self) -> bool {
        if let Some(d) = self.state.borrow().dark {
            return d;
        }
        let cfg = self.config.borrow();
        match cfg.theme.mode {
            ThemeMode::Light => false,
            ThemeMode::Dark => true,
            ThemeMode::Auto => {
                let h = chrono::Timelike::hour(&chrono::Local::now());
                let (from, until) = (cfg.theme.dark_from, cfg.theme.dark_until);
                if from > until {
                    h >= from || h < until
                } else {
                    h >= from && h < until
                }
            }
        }
    }

    pub fn apply_theme(&self) {
        let dark = self.is_dark();
        let changed = self.theme.apply(&self.config.borrow(), dark);
        if changed {
            // Allinea anche le app GTK/libadwaita alla stessa "edizione".
            let scheme = if dark { "prefer-dark" } else { "prefer-light" };
            let gtk3 = if dark { "adw-gtk3-dark" } else { "adw-gtk3" };
            // GTK3 non conosce color-scheme: se si usa adw-gtk3 si cambia variante.
            sys::spawn(&format!(
                "gsettings set org.gnome.desktop.interface color-scheme {scheme} 2>/dev/null; \
                 case \"$(gsettings get org.gnome.desktop.interface gtk-theme 2>/dev/null)\" in \
                 *adw-gtk3*) gsettings set org.gnome.desktop.interface gtk-theme {gtk3} ;; esac"
            ));
            if let Some(cover) = self.cover.get() {
                cover.update();
            }
            self.sync_compositor_colors(dark);
        }
    }

    /// Porta la stessa palette nel compositor: bordi, sfondo e barre del titolo.
    fn sync_compositor_colors(&self, dark: bool) {
        if !hypr::available() {
            return;
        }
        let t = self.config.borrow().theme.clone();
        let hex = |s: &str| s.trim().trim_start_matches('#').to_string();
        let (paper, ink) = if dark {
            (hex(&t.paper_dark), hex(&t.ink_dark))
        } else {
            (hex(&t.paper), hex(&t.ink))
        };
        let lua = format!(
            "hl.config({{ general = {{ col = {{ active_border = \"rgba({ink}d9)\", inactive_border = \"rgba({ink}1f)\" }} }}, \
             misc = {{ background_color = \"rgb({paper})\" }} }}) \
             if hl.plugin and hl.plugin.hyprbars then hl.config({{ plugin = {{ hyprbars = {{ bar_color = \"rgb({paper})\", [\"col.text\"] = \"rgb({ink})\" }} }} }}) end"
        );
        gtk::gio::spawn_blocking(move || {
            if !hypr::eval(&lua) {
                log::debug!("colori del compositor non aggiornati (Hyprland < 0.55?)");
            }
        });
    }

    pub fn set_theme(&self, arg: &str) {
        {
            let mut st = self.state.borrow_mut();
            st.dark = match arg {
                "light" => Some(false),
                "dark" => Some(true),
                "auto" => None,
                _ => Some(!self.is_dark_unlocked(&st)),
            };
            st.save();
        }
        self.apply_theme();
        if let Some(shade) = self.shade.get() {
            shade.sync();
        }
    }

    fn is_dark_unlocked(&self, st: &State) -> bool {
        st.dark.unwrap_or_else(|| self.theme.is_dark())
    }

    // ───────────────────────────── Tempo ─────────────────────────────

    fn tick(&self) {
        self.live.refresh(true);
        if let Some(bar) = self.bar.get() {
            bar.update_clock();
        }
        let minute = chrono::Timelike::minute(&chrono::Local::now());
        if minute != self.last_minute.get() {
            self.last_minute.set(minute);
            self.apply_theme();
            if let Some(cover) = self.cover.get() {
                cover.update();
            }
        }
    }

    // ───────────────────────────── Hyprland ─────────────────────────────

    fn on_hypr_event(&self, ev: &hypr::Event) {
        if ev.name == "configreloaded" {
            log::info!("configurazione di Hyprland ricaricata");
        }
        if self.refresh_pending.replace(true) {
            return;
        }
        // Raggruppa le raffiche di eventi (apertura finestra = 4-5 eventi).
        glib::timeout_add_local_once(Duration::from_millis(45), || shell().refresh_hypr());
    }

    pub fn refresh_hypr(&self) {
        glib::spawn_future_local(async {
            let snap = gio::spawn_blocking(Snapshot::fetch)
                .await
                .unwrap_or_default();
            let s = shell();
            s.refresh_pending.set(false);
            *s.hypr.borrow_mut() = snap;
            s.after_hypr();
        });
    }

    fn after_hypr(&self) {
        if let Some(bar) = self.bar.get() {
            bar.update_hypr();
        }
        if let Some(dock) = self.dock.get() {
            dock.update_running();
        }
        if let Some(cover) = self.cover.get() {
            cover.update();
        }
        if let Some(chrome) = self.chrome.get() {
            chrome.follow_active();
        }
        if let Some(ov) = self.overview.get() {
            ov.on_hypr();
        }
        self.live.refresh(false);
    }

    /// Finestre (aperte o ridotte) appartenenti a un'app installata.
    pub fn windows_of(&self, app: &App) -> Vec<Client> {
        let catalog = self.apps.borrow();
        let mut v: Vec<Client> = self
            .hypr
            .borrow()
            .clients
            .iter()
            .filter(|c| {
                c.mapped
                    && catalog
                        .for_class(c.app_class())
                        .map(|a| a.id == app.id)
                        .unwrap_or(false)
            })
            .cloned()
            .collect();
        v.sort_by_key(|c| c.focus_history_id);
        v
    }

    pub fn focus_window(&self, c: &Client) {
        if c.is_minimized() {
            self.restore(c);
        } else {
            hypr::dispatch(&Dispatch::FocusWindow(c.address.clone()));
        }
    }

    pub fn minimize(&self, c: &Client) {
        hypr::dispatch(&Dispatch::MoveToWorkspace {
            address: c.address.clone(),
            workspace: format!("special:{MINIMIZED}"),
            follow: false,
        });
    }

    pub fn restore(&self, c: &Client) {
        let ws = self.hypr.borrow().active_workspace().to_string();
        hypr::dispatch(&Dispatch::MoveToWorkspace {
            address: c.address.clone(),
            workspace: ws,
            follow: true,
        });
        hypr::dispatch(&Dispatch::FocusWindow(c.address.clone()));
    }

    /// Apre un'app come su un tablet: se è già aperta la porta in primo piano.
    pub fn open_app(&self, app: &App) {
        let wins = self.windows_of(app);
        let active = self
            .hypr
            .borrow()
            .active
            .as_ref()
            .map(|a| a.address.clone());
        match wins.first() {
            // Tocco sull'app già attiva: passa alla finestra successiva della stessa app.
            Some(first) if Some(first.address.clone()) == active && wins.len() > 1 => {
                self.focus_window(&wins[1]);
            }
            Some(first) => self.focus_window(first),
            None => app.launch(),
        }
        self.close_overlays();
        if let Some(dock) = self.dock.get() {
            dock.autohide_soon();
        }
    }

    pub fn active_client(&self) -> Option<Client> {
        self.hypr.borrow().active.clone()
    }

    // ───────────────────────────── Overlay ─────────────────────────────

    pub fn close_overlays(&self) {
        glib::idle_add_local_once(|| shell().sync_handle());
        if let Some(c) = self.chrome.get() {
            c.close();
        }
        if let Some(c) = self.shade.get() {
            c.close();
        }
        if let Some(c) = self.layout.get() {
            c.close();
        }
        if let Some(c) = self.overview.get() {
            c.close();
        }
        if let Some(c) = self.home.get() {
            c.close();
        }
        self.osk.hide();
    }

    /// Chiude il più "alto" degli overlay aperti. Restituisce true se ne ha chiuso uno.
    fn close_topmost(&self) -> bool {
        if let Some(c) = self.chrome.get().filter(|c| c.is_open()) {
            c.close();
        } else if let Some(c) = self.shade.get().filter(|c| c.is_open()) {
            c.close();
        } else if let Some(c) = self.layout.get().filter(|c| c.is_open()) {
            c.close();
        } else if let Some(c) = self.overview.get().filter(|c| c.is_open()) {
            c.close();
        } else if let Some(c) = self.home.get().filter(|c| c.is_open()) {
            c.close();
        } else {
            return false;
        }
        true
    }

    /// Gesto Indietro di Android: prima chiude gli overlay, poi parla alla finestra.
    pub fn back(&self) {
        if self.close_topmost() {
            return;
        }
        if let Some(c) = self.active_client() {
            // Dialoghi e finestre fluttuanti: Esc. Finestre principali: Alt+← (browser, file manager…).
            let (mods, key) = if c.floating {
                ("", "Escape")
            } else {
                ("ALT", "Left")
            };
            hypr::dispatch(&Dispatch::SendShortcut {
                address: c.address.clone(),
                mods: mods.into(),
                key: key.into(),
            });
        }
    }

    // ───────────────────────────── Comandi ─────────────────────────────

    pub fn command(&self, line: &str) {
        let mut parts = line.split_whitespace();
        let cmd = parts.next().unwrap_or_default();
        let arg = parts.next().unwrap_or("toggle");
        log::debug!("comando: {line}");
        match cmd {
            "home" | "indice" if arg == "edit" => {
                if let Some(h) = self.home.get() {
                    self.close_overlays();
                    h.open();
                    h.edit();
                }
            }
            "home" | "indice" => toggle(
                self.home.get().map(|h| h.as_ref() as &dyn Overlay),
                arg,
                self,
            ),
            "overview" | "recents" | "sommario" => toggle(
                self.overview.get().map(|h| h.as_ref() as &dyn Overlay),
                arg,
                self,
            ),
            "shade" | "notiziario" => toggle(
                self.shade.get().map(|h| h.as_ref() as &dyn Overlay),
                arg,
                self,
            ),
            "chrome" => toggle(
                self.chrome.get().map(|h| h.as_ref() as &dyn Overlay),
                arg,
                self,
            ),
            "layout" | "impagina" => toggle(
                self.layout.get().map(|h| h.as_ref() as &dyn Overlay),
                arg,
                self,
            ),
            "dock" => {
                if let Some(d) = self.dock.get() {
                    match arg {
                        "show" => d.reveal(),
                        "hide" => d.conceal(),
                        _ => d.toggle_reveal(),
                    }
                }
            }
            "back" => self.back(),
            "close" => self.close_overlays(),
            "minimize" => {
                if let Some(c) = self.active_client() {
                    self.minimize(&c);
                }
            }
            "restore" => {
                let last = self.hypr.borrow().minimized().first().map(|c| (*c).clone());
                if let Some(c) = last {
                    self.restore(&c);
                }
            }
            "gesture" => self.gesture(line),
            "theme" => self.set_theme(arg),
            "osk" => match arg {
                "show" => self.osk.show(),
                "hide" => self.osk.hide(),
                _ => self.osk.toggle(),
            },
            "reload" => self.reload(),
            "rotate" => crate::ui::shade::rotate(),
            "ping" => {}
            "quit" => self.app.quit(),
            other => log::warn!("comando sconosciuto: {other}"),
        }
        self.sync_handle();
    }

    /// La maniglia nativa segue la finestra attiva e si nasconde sotto i fogli.
    pub fn sync_handle(&self) {
        if let Some(c) = self.chrome.get() {
            c.follow_active();
        }
    }

    fn gesture(&self, line: &str) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.get(1) != Some(&"edge-up") {
            log::warn!("gesto sconosciuto: {line}");
            return;
        }
        let fraction: f64 = parts.get(2).and_then(|v| v.parse().ok()).unwrap_or(0.0);
        let held: u32 = parts.get(3).and_then(|v| v.parse().ok()).unwrap_or(0);
        let home_open = self.home.get().map(|h| h.is_open()).unwrap_or(false);
        let outcome = gesture::edge_up(&self.config.borrow().gestures, fraction, held, home_open);
        log::debug!("swipe dal basso {fraction:.2} fermo {held}ms → {outcome:?}");
        match outcome {
            Outcome::Dock => {
                if let Some(d) = self.dock.get() {
                    d.reveal();
                }
            }
            Outcome::Home => {
                self.close_overlays();
                if let Some(h) = self.home.get() {
                    h.open();
                }
            }
            Outcome::Recents => {
                self.close_overlays();
                if let Some(o) = self.overview.get() {
                    o.open();
                }
            }
            Outcome::Nothing => {}
        }
    }

    fn reload(&self) {
        *self.config.borrow_mut() = Config::load();
        *self.apps.borrow_mut() = Catalog::load();
        self.apply_theme_forced();
        if let Some(b) = self.bar.get() {
            b.rebuild();
        }
        if let Some(d) = self.dock.get() {
            d.rebuild();
        }
        if let Some(h) = self.home.get() {
            h.invalidate();
        }
        if let Some(c) = self.cover.get() {
            c.update();
        }
        log::info!("configurazione ricaricata");
    }

    fn apply_theme_forced(&self) {
        self.theme.invalidate();
        self.apply_theme();
    }

    pub fn save_state(&self) {
        self.state.borrow().save();
    }
}

/// Interfaccia comune degli overlay a schermo intero.
pub trait Overlay {
    fn is_open(&self) -> bool;
    fn open(&self);
    fn close(&self);
}

fn toggle(overlay: Option<&dyn Overlay>, arg: &str, shell: &Shell) {
    let Some(o) = overlay else { return };
    match arg {
        "show" | "open" => {
            if !o.is_open() {
                shell.close_overlays();
                o.open();
            }
        }
        "hide" | "close" => o.close(),
        _ => {
            if o.is_open() {
                o.close();
            } else {
                shell.close_overlays();
                o.open();
            }
        }
    }
}

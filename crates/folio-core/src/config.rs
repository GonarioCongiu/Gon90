//! `~/.config/folio/config.toml`
//!
//! Ogni campo ha un valore predefinito: un file vuoto (o assente) produce
//! la configurazione "di redazione" pensata per un tablet in orizzontale.

use crate::live::LiveKind;
use crate::paths;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub theme: Theme,
    pub bar: BarConfig,
    pub dock: DockConfig,
    pub home: HomeConfig,
    pub cover: CoverConfig,
    pub chrome: ChromeConfig,
    pub gestures: GestureConfig,
    pub notifications: NotifConfig,
    pub osk: OskConfig,
    pub weather: WeatherConfig,
    pub commands: Commands,
}

impl Config {
    pub fn path() -> std::path::PathBuf {
        paths::config_dir().join("config.toml")
    }

    /// Carica la configurazione; in caso di errore di sintassi registra il
    /// problema e ricade sui valori predefiniti invece di lasciare lo schermo vuoto.
    pub fn load() -> Self {
        let path = Self::path();
        match std::fs::read_to_string(&path) {
            Ok(text) => match toml::from_str::<Config>(&text) {
                Ok(cfg) => cfg,
                Err(err) => {
                    log::error!("{}: {err}", path.display());
                    Config::default()
                }
            },
            Err(_) => Config::default(),
        }
    }

    pub fn parse(text: &str) -> anyhow::Result<Self> {
        Ok(toml::from_str(text)?)
    }
}

// ───────────────────────────── Tema ─────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    Light,
    Dark,
    /// "Edizione serale": scuro tra `dark_from` e `dark_until`.
    Auto,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum IconStyle {
    /// Icone desaturate, come le fotografie in bianco e nero di una rivista.
    Mono,
    Color,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Theme {
    pub mode: ThemeMode,
    pub dark_from: u32,
    pub dark_until: u32,
    pub accent: String,
    pub paper: String,
    pub ink: String,
    pub paper_dark: String,
    pub ink_dark: String,
    pub font_display: String,
    pub font_text: String,
    pub font_mono: String,
    pub radius: u32,
    pub icons: IconStyle,
    /// Moltiplicatore globale delle dimensioni (utile su schermi molto densi).
    pub scale: f64,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            mode: ThemeMode::Auto,
            dark_from: 19,
            dark_until: 7,
            accent: "#D4442A".into(),
            paper: "#F3F0E8".into(),
            ink: "#141413".into(),
            paper_dark: "#121211".into(),
            ink_dark: "#ECE8DE".into(),
            font_display: "Instrument Serif".into(),
            font_text: "Inter".into(),
            font_mono: "JetBrains Mono".into(),
            radius: 10,
            icons: IconStyle::Mono,
            scale: 1.0,
        }
    }
}

// ───────────────────────────── Testata ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BarConfig {
    pub enabled: bool,
    pub masthead: String,
    pub show_title: bool,
    pub show_seconds: bool,
    /// Spessore della testata in px (altezza se orizzontale, larghezza se verticale).
    pub size: i32,
    /// Distanza dal bordo dello schermo quando è agganciata.
    pub margin: i32,
}

impl Default for BarConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            masthead: "Folio".into(),
            show_title: true,
            show_seconds: false,
            size: 44,
            margin: 0,
        }
    }
}

// ───────────────────────────── Dock ─────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DockMode {
    /// Sempre visibile e riserva spazio (come la taskbar persistente dei tablet Android).
    Persistent,
    /// Compare con lo swipe corto dal bordo inferiore e si nasconde da sola.
    Transient,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DockConfig {
    pub enabled: bool,
    pub mode: DockMode,
    /// ID dei file .desktop; più candidati separati da `|` (vince il primo installato).
    pub pinned: Vec<String>,
    pub icon_size: i32,
    pub margin: i32,
    pub autohide_secs: u32,
    pub show_running: bool,
}

impl Default for DockConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: DockMode::Persistent,
            pinned: vec![
                "org.gnome.Nautilus.desktop|thunar.desktop|org.kde.dolphin.desktop|pcmanfm.desktop".into(),
                "firefox.desktop|org.mozilla.firefox.desktop|chromium.desktop|brave-browser.desktop|zen.desktop".into(),
                "kitty.desktop|Alacritty.desktop|foot.desktop|org.gnome.Console.desktop|com.mitchellh.ghostty.desktop".into(),
                "org.gnome.Calendar.desktop|org.kde.merkuro.calendar.desktop".into(),
                "org.gnome.TextEditor.desktop|org.kde.kate.desktop|code.desktop|dev.zed.Zed.desktop".into(),
                "org.gnome.Settings.desktop|nwg-look.desktop|systemsettings.desktop".into(),
            ],
            icon_size: 52,
            margin: 12,
            autohide_secs: 4,
            show_running: true,
        }
    }
}

// ───────────────────────────── Indice (home) ─────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TileSize {
    Small,
    Medium,
    Wide,
    Large,
}

impl TileSize {
    /// Colonne × righe occupate nella griglia.
    pub fn span(self) -> (u32, u32) {
        match self {
            TileSize::Small => (1, 1),
            TileSize::Medium => (2, 2),
            TileSize::Wide => (4, 2),
            TileSize::Large => (4, 4),
        }
    }

    pub fn next(self) -> Self {
        match self {
            TileSize::Small => TileSize::Medium,
            TileSize::Medium => TileSize::Wide,
            TileSize::Wide => TileSize::Large,
            TileSize::Large => TileSize::Small,
        }
    }

    pub fn css(self) -> &'static str {
        match self {
            TileSize::Small => "small",
            TileSize::Medium => "medium",
            TileSize::Wide => "wide",
            TileSize::Large => "large",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TileStyle {
    Paper,
    Ink,
    Accent,
}

impl TileStyle {
    pub fn css(self) -> &'static str {
        match self {
            TileStyle::Paper => "paper",
            TileStyle::Ink => "ink",
            TileStyle::Accent => "accent",
        }
    }

    pub fn next(self) -> Self {
        match self {
            TileStyle::Paper => TileStyle::Ink,
            TileStyle::Ink => TileStyle::Accent,
            TileStyle::Accent => TileStyle::Paper,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TileSpec {
    /// ID .desktop (anche più candidati separati da `|`). Facoltativo: un widget
    /// può esistere anche solo come contenuto (es. il lettore musicale).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app: Option<String>,
    /// Fonte del contenuto vivo. Assente = scelta automatica in base all'app.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub live: Option<LiveKind>,
    #[serde(default = "default_tile_size")]
    pub size: TileSize,
    #[serde(default = "default_tile_style")]
    pub style: TileStyle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

fn default_tile_size() -> TileSize {
    TileSize::Medium
}
fn default_tile_style() -> TileStyle {
    TileStyle::Paper
}

impl TileSpec {
    pub fn app(app: &str, live: Option<LiveKind>, size: TileSize, style: TileStyle) -> Self {
        Self {
            app: Some(app.into()),
            live,
            size,
            style,
            label: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HomeConfig {
    /// Lato di una cella della griglia in px.
    pub cell: i32,
    pub gap: i32,
    /// 0 = calcolate dalla larghezza dello schermo.
    pub columns: u32,
    pub notes_file: String,
    pub tiles: Vec<TileSpec>,
}

impl Default for HomeConfig {
    fn default() -> Self {
        use LiveKind::*;
        use TileSize::*;
        use TileStyle::*;
        Self {
            cell: 92,
            gap: 12,
            columns: 0,
            notes_file: "~/Documenti/note.md".into(),
            tiles: vec![
                TileSpec::app("org.gnome.clocks.desktop|org.kde.kclock.desktop", Some(Clock), Large, Ink),
                TileSpec::app("org.gnome.Calendar.desktop|org.kde.merkuro.calendar.desktop", Some(Calendar), Medium, Paper),
                TileSpec::app("org.gnome.Weather.desktop|org.kde.kweather.desktop", Some(Weather), Medium, Paper),
                TileSpec { app: None, live: Some(Music), size: Wide, style: Accent, label: Some("Musica".into()) },
                TileSpec::app("org.gnome.SystemMonitor.desktop|btop.desktop|htop.desktop|org.kde.plasma-systemmonitor.desktop", Some(System), Wide, Paper),
                TileSpec::app("org.gnome.Nautilus.desktop|thunar.desktop|org.kde.dolphin.desktop", Some(Disk), Medium, Paper),
                TileSpec::app("kitty.desktop|Alacritty.desktop|foot.desktop|org.gnome.Console.desktop", Some(Uptime), Medium, Ink),
                TileSpec::app("firefox.desktop|chromium.desktop|brave-browser.desktop|zen.desktop", Some(Windows), Medium, Paper),
                TileSpec::app("org.gnome.TextEditor.desktop|org.kde.kate.desktop|code.desktop", Some(Notes), Wide, Paper),
                TileSpec::app("org.gnome.Settings.desktop|nwg-look.desktop", Some(Battery), Small, Paper),
                TileSpec::app("org.gnome.Loupe.desktop|org.kde.gwenview.desktop|imv.desktop", None, Small, Paper),
                TileSpec::app("org.gnome.Software.desktop|octopi.desktop|cachyos-pi.desktop", None, Small, Paper),
            ],
        }
    }
}

// ───────────────────────────── Copertina ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CoverConfig {
    pub enabled: bool,
    pub masthead: String,
    pub tagline: String,
    /// Immagine di copertina facoltativa (a tutta pagina, dietro la tipografia).
    pub image: Option<String>,
    /// Mostra la griglia di impaginazione a 12 colonne, appena accennata.
    pub show_grid: bool,
}

impl Default for CoverConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            masthead: "FOLIO".into(),
            tagline: "Periodico di ambiente desktop".into(),
            image: None,
            show_grid: true,
        }
    }
}

// ───────────────────────────── Barra del titolo ─────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ChromeMode {
    /// Barra del titolo disegnata da hyprbars; il suo pulsante "⋯" apre il popup di Folio.
    Hyprbars,
    /// Nessun plugin: una maniglia sottile (stile iPadOS) sopra la finestra attiva apre il popup.
    Native,
    Off,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ChromeConfig {
    pub mode: ChromeMode,
    /// Altezza della barra di hyprbars (serve per posizionare il popup sotto di essa).
    pub bar_height: i32,
}

impl Default for ChromeConfig {
    fn default() -> Self {
        Self {
            mode: ChromeMode::Hyprbars,
            bar_height: 34,
        }
    }
}

// ───────────────────────────── Gesti ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GestureConfig {
    /// Sotto questa frazione di altezza lo swipe dal basso mostra solo la dock.
    pub dock_threshold: f64,
    /// Pausa (ms) prima del rilascio che trasforma "Home" in "Recenti".
    pub hold_ms: u32,
    /// Oltre questa frazione lo swipe lungo apre comunque i Recenti.
    pub recents_threshold: f64,
}

impl Default for GestureConfig {
    fn default() -> Self {
        Self {
            dock_threshold: 0.08,
            hold_ms: 220,
            recents_threshold: 0.55,
        }
    }
}

// ───────────────────────────── Notifiche, tastiera, meteo ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NotifConfig {
    pub enabled: bool,
    pub timeout_ms: u32,
    pub max_toasts: usize,
}

impl Default for NotifConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            timeout_ms: 5000,
            max_toasts: 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OskConfig {
    pub enabled: bool,
    /// Nome del processo della tastiera a schermo (wvkbd reagisce a SIGUSR1/SIGUSR2).
    pub process: String,
    /// Comando di avvio se il processo non è in esecuzione.
    pub command: String,
    /// Mostra la tastiera quando un campo di ricerca di Folio riceve il fuoco.
    pub auto_show: bool,
}

impl Default for OskConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            process: "wvkbd-mobintl".into(),
            command: "wvkbd-mobintl --hidden -L 280 --fn 'Inter 18'".into(),
            auto_show: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WeatherConfig {
    pub enabled: bool,
    /// Vuoto = posizione stimata da wttr.in.
    pub location: String,
    pub refresh_min: u64,
}

impl Default for WeatherConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            location: String::new(),
            refresh_min: 30,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Commands {
    pub terminal: String,
    pub lock: String,
    pub suspend: String,
    pub reboot: String,
    pub poweroff: String,
    pub logout: String,
    pub screenshot: String,
    pub settings: String,
}

impl Default for Commands {
    fn default() -> Self {
        Self {
            terminal: "kitty".into(),
            lock: "hyprlock".into(),
            suspend: "systemctl suspend".into(),
            reboot: "systemctl reboot".into(),
            poweroff: "systemctl poweroff".into(),
            logout: "command -v hyprshutdown >/dev/null && hyprshutdown || hyprctl dispatch 'hl.dsp.exit()'".into(),
            screenshot: "mkdir -p ~/Immagini/Schermate && grim ~/Immagini/Schermate/$(date +%Y-%m-%d_%H-%M-%S).png".into(),
            settings: "".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_file_gives_defaults() {
        let cfg = Config::parse("").unwrap();
        assert_eq!(cfg.bar.size, 44);
        assert!(!cfg.home.tiles.is_empty());
    }

    #[test]
    fn partial_override() {
        let cfg = Config::parse(
            "[theme]\nmode = \"dark\"\naccent = \"#0033FF\"\n[dock]\nmode = \"transient\"\n",
        )
        .unwrap();
        assert_eq!(cfg.theme.mode, ThemeMode::Dark);
        assert_eq!(cfg.theme.accent, "#0033FF");
        assert_eq!(cfg.dock.mode, DockMode::Transient);
        assert_eq!(cfg.theme.font_text, "Inter");
    }

    #[test]
    fn shipped_dotfile_is_valid() {
        let text = include_str!("../../../dotfiles/folio/config.toml");
        let cfg = Config::parse(text).expect("dotfiles/folio/config.toml deve essere valido");
        assert_eq!(cfg.home.tiles.len(), 10);
        assert_eq!(cfg.chrome.mode, ChromeMode::Hyprbars);
    }

    #[test]
    fn tiles_roundtrip() {
        let cfg = Config::default();
        let text = toml::to_string_pretty(&cfg).unwrap();
        let back = Config::parse(&text).unwrap();
        assert_eq!(back.home.tiles, cfg.home.tiles);
    }
}

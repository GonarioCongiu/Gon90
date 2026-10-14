//! Contenuti "vivi" delle icone-widget.
//!
//! Ogni icona di Folio è un piccolo articolo: un occhiello (kicker), un titolo
//! (headline) e un sommario (deck), più eventuali dati grafici. Il calcolo è
//! puro — dati in ingresso, testo in uscita — e quindi testabile.

use crate::locale;
use crate::sys::SysSnapshot;
use chrono::{DateTime, Datelike, Local, NaiveDate, Timelike};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum LiveKind {
    Clock,
    Calendar,
    Weather,
    Music,
    System,
    Battery,
    Disk,
    Uptime,
    Windows,
    Notes,
    Network,
    Plain,
}

impl LiveKind {
    pub const ALL: [LiveKind; 12] = [
        LiveKind::Clock,
        LiveKind::Calendar,
        LiveKind::Weather,
        LiveKind::Music,
        LiveKind::System,
        LiveKind::Battery,
        LiveKind::Disk,
        LiveKind::Uptime,
        LiveKind::Windows,
        LiveKind::Notes,
        LiveKind::Network,
        LiveKind::Plain,
    ];

    pub fn label(self) -> &'static str {
        match self {
            LiveKind::Clock => "Orologio",
            LiveKind::Calendar => "Calendario",
            LiveKind::Weather => "Meteo",
            LiveKind::Music => "Musica",
            LiveKind::System => "Sistema",
            LiveKind::Battery => "Batteria",
            LiveKind::Disk => "Archivio",
            LiveKind::Uptime => "Terminale",
            LiveKind::Windows => "Finestre",
            LiveKind::Notes => "Appunti",
            LiveKind::Network => "Rete",
            LiveKind::Plain => "Applicazione",
        }
    }

    /// Ha senso aggiornarlo ogni secondo?
    pub fn ticks(self) -> bool {
        matches!(self, LiveKind::Clock)
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LiveContent {
    /// Occhiello: piccolo, maiuscolo, spaziato.
    pub kicker: String,
    /// Titolo: grande, in serif.
    pub headline: String,
    /// Sommario: una riga di testo.
    pub deck: String,
    /// Filetto di avanzamento 0..1.
    pub progress: Option<f64>,
    /// Serie per la sparkline (0..1).
    pub series: Vec<f64>,
    /// Forma compatta per la dock e le icone piccole.
    pub badge: Option<String>,
}

pub struct LiveCtx<'a> {
    pub now: DateTime<Local>,
    pub sys: &'a SysSnapshot,
    /// Finestre aperte dell'app associata.
    pub windows: usize,
    pub app_name: &'a str,
    pub description: &'a str,
    pub notes: Option<&'a str>,
    pub seconds: bool,
}

/// Sceglie automaticamente un contenuto dal nome e dalle categorie del .desktop.
pub fn guess(app_id: &str, categories: &str) -> LiveKind {
    let id = app_id.to_lowercase();
    let cat = categories.to_lowercase();
    let has = |needles: &[&str]| needles.iter().any(|n| id.contains(n));
    if has(&["clock", "kclock"]) {
        LiveKind::Clock
    } else if has(&["calendar", "merkuro"]) || cat.contains("calendar") {
        LiveKind::Calendar
    } else if has(&["weather"]) {
        LiveKind::Weather
    } else if has(&[
        "spotify",
        "music",
        "rhythmbox",
        "amberol",
        "elisa",
        "lollypop",
    ]) || cat.contains("audio")
    {
        LiveKind::Music
    } else if has(&["monitor", "btop", "htop", "resources", "missioncenter"]) {
        LiveKind::System
    } else if has(&["nautilus", "thunar", "dolphin", "pcmanfm", "nemo"])
        || cat.contains("filemanager")
    {
        LiveKind::Disk
    } else if cat.contains("terminalemulator") {
        LiveKind::Uptime
    } else if cat.contains("webbrowser") {
        LiveKind::Windows
    } else if cat.contains("texteditor") || has(&["notes", "obsidian"]) {
        LiveKind::Notes
    } else if has(&["settings", "control-center", "systemsettings"]) {
        LiveKind::Battery
    } else {
        LiveKind::Plain
    }
}

fn days_in_month(year: i32, month: u32) -> u32 {
    let (ny, nm) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    NaiveDate::from_ymd_opt(ny, nm, 1)
        .and_then(|d| d.pred_opt())
        .map(|d| d.day())
        .unwrap_or(30)
}

fn count_badge(n: usize) -> Option<String> {
    (n > 0).then(|| n.to_string())
}

pub fn compute(kind: LiveKind, ctx: &LiveCtx) -> LiveContent {
    let now = ctx.now;
    let sys = ctx.sys;
    match kind {
        LiveKind::Clock => {
            let time = if ctx.seconds {
                now.format("%H:%M:%S").to_string()
            } else {
                now.format("%H:%M").to_string()
            };
            let day_frac = (now.num_seconds_from_midnight() as f64) / 86_400.0;
            LiveContent {
                kicker: locale::edition(&now).to_uppercase(),
                headline: time.clone(),
                deck: locale::long_date(&now),
                progress: Some(day_frac),
                badge: Some(now.format("%H:%M").to_string()),
                ..Default::default()
            }
        }
        LiveKind::Calendar => {
            let dim = days_in_month(now.year(), now.month());
            LiveContent {
                kicker: locale::weekday(&now).to_uppercase(),
                headline: now.day().to_string(),
                deck: format!(
                    "{} {} · settimana {}",
                    locale::month(&now),
                    now.year(),
                    now.iso_week().week()
                ),
                progress: Some(now.day() as f64 / dim as f64),
                badge: Some(now.day().to_string()),
                ..Default::default()
            }
        }
        LiveKind::Weather => match &sys.weather {
            Some(w) => LiveContent {
                kicker: if w.place.is_empty() {
                    "METEO".into()
                } else {
                    format!("METEO · {}", w.place.to_uppercase())
                },
                headline: w.temp.clone(),
                deck: w.condition.clone(),
                badge: Some(w.temp.trim_start_matches('+').to_string()),
                ..Default::default()
            },
            None => LiveContent {
                kicker: "METEO".into(),
                headline: "—".into(),
                deck: "In attesa delle previsioni".into(),
                ..Default::default()
            },
        },
        LiveKind::Music => match &sys.music {
            Some(m) => LiveContent {
                kicker: if m.playing {
                    "IN RIPRODUZIONE".into()
                } else {
                    "IN PAUSA".into()
                },
                headline: if m.title.is_empty() {
                    m.player.clone()
                } else {
                    m.title.clone()
                },
                deck: m.artist.clone(),
                badge: Some(if m.playing {
                    "▶".into()
                } else {
                    "Ⅱ".into()
                }),
                ..Default::default()
            },
            None => LiveContent {
                kicker: "MUSICA".into(),
                headline: "Silenzio".into(),
                deck: "Nessun lettore attivo".into(),
                ..Default::default()
            },
        },
        LiveKind::System => LiveContent {
            kicker: "SISTEMA".into(),
            headline: format!("{:.0}%", sys.cpu),
            deck: format!("Processore · memoria al {:.0}%", sys.mem),
            progress: Some(sys.mem / 100.0),
            series: sys.cpu_hist.clone(),
            badge: Some(format!("{:.0}", sys.cpu)),
        },
        LiveKind::Battery => match &sys.battery {
            Some(b) => LiveContent {
                kicker: if b.charging {
                    "IN CARICA".into()
                } else if b.full {
                    "CARICA COMPLETA".into()
                } else {
                    "BATTERIA".into()
                },
                headline: format!("{}%", b.percent),
                deck: ctx.app_name.to_string(),
                progress: Some(b.percent as f64 / 100.0),
                badge: Some(format!("{}", b.percent)),
                ..Default::default()
            },
            None => LiveContent {
                kicker: "ALIMENTAZIONE".into(),
                headline: "Rete".into(),
                deck: ctx.app_name.to_string(),
                ..Default::default()
            },
        },
        LiveKind::Disk => match &sys.disk {
            Some(d) => LiveContent {
                kicker: "ARCHIVIO".into(),
                headline: format!("{:.0} GB", d.free_gb),
                deck: format!("liberi su {:.0} GB", d.total_gb),
                progress: Some(d.used),
                badge: count_badge(ctx.windows),
                ..Default::default()
            },
            None => plain(ctx),
        },
        LiveKind::Uptime => LiveContent {
            kicker: "IN SERVIZIO DA".into(),
            headline: locale::duration(sys.uptime),
            deck: format!("carico medio {:.2}", sys.load),
            badge: count_badge(ctx.windows),
            ..Default::default()
        },
        LiveKind::Windows => LiveContent {
            kicker: "IN LETTURA".into(),
            headline: if ctx.windows == 0 {
                "—".into()
            } else {
                ctx.windows.to_string()
            },
            deck: if ctx.windows == 0 {
                ctx.description.to_string()
            } else {
                locale::plural(ctx.windows, "finestra aperta", "finestre aperte")
            },
            badge: count_badge(ctx.windows),
            ..Default::default()
        },
        LiveKind::Notes => {
            let mut lines = ctx
                .notes
                .unwrap_or("")
                .lines()
                .map(|l| {
                    l.trim()
                        .trim_start_matches('#')
                        .trim_start_matches("- ")
                        .trim()
                })
                .filter(|l| !l.is_empty());
            let first = lines.next().unwrap_or("Pagina bianca").to_string();
            let second = lines.next().unwrap_or("Tocca per scrivere").to_string();
            LiveContent {
                kicker: "APPUNTI".into(),
                headline: first,
                deck: second,
                badge: count_badge(ctx.windows),
                ..Default::default()
            }
        }
        LiveKind::Network => match &sys.wifi {
            Some(w) if w.enabled => LiveContent {
                kicker: "RETE".into(),
                headline: w.ssid.clone().unwrap_or_else(|| "Non connesso".into()),
                deck: "Wi-Fi attivo".into(),
                ..Default::default()
            },
            _ => LiveContent {
                kicker: "RETE".into(),
                headline: "Offline".into(),
                deck: "Wi-Fi spento".into(),
                ..Default::default()
            },
        },
        LiveKind::Plain => plain(ctx),
    }
}

fn plain(ctx: &LiveCtx) -> LiveContent {
    LiveContent {
        kicker: if ctx.windows > 0 {
            locale::plural(ctx.windows, "finestra", "finestre").to_uppercase()
        } else {
            String::new()
        },
        headline: ctx.app_name.to_string(),
        deck: ctx.description.to_string(),
        badge: count_badge(ctx.windows),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn ctx<'a>(sys: &'a SysSnapshot) -> LiveCtx<'a> {
        LiveCtx {
            now: Local.with_ymd_and_hms(2026, 10, 14, 9, 41, 0).unwrap(),
            sys,
            windows: 2,
            app_name: "Firefox",
            description: "Navigare il web",
            notes: Some("# Lista\n- pane\n- caffè"),
            seconds: false,
        }
    }

    #[test]
    fn clock_and_calendar() {
        let sys = SysSnapshot::default();
        let c = compute(LiveKind::Clock, &ctx(&sys));
        assert_eq!(c.headline, "09:41");
        assert_eq!(c.kicker, "EDIZIONE DEL MATTINO");
        let cal = compute(LiveKind::Calendar, &ctx(&sys));
        assert_eq!(cal.headline, "14");
        assert_eq!(cal.kicker, "MERCOLEDÌ");
    }

    #[test]
    fn windows_and_notes() {
        let sys = SysSnapshot::default();
        let w = compute(LiveKind::Windows, &ctx(&sys));
        assert_eq!(w.headline, "2");
        assert_eq!(w.badge.as_deref(), Some("2"));
        let n = compute(LiveKind::Notes, &ctx(&sys));
        assert_eq!(n.headline, "Lista");
        assert_eq!(n.deck, "pane");
    }

    #[test]
    fn guesses() {
        assert_eq!(guess("org.gnome.Calendar.desktop", ""), LiveKind::Calendar);
        assert_eq!(
            guess("kitty.desktop", "System;TerminalEmulator;"),
            LiveKind::Uptime
        );
        assert_eq!(
            guess("firefox.desktop", "Network;WebBrowser;"),
            LiveKind::Windows
        );
    }
}

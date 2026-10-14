//! `folio doctor`: diagnosi di compatibilità dell'ambiente.
//! Lo usa anche `installer.sh` dopo la compilazione.

use folio_core::{hypr, ipc, paths, sys};
use std::process::{Command, ExitCode, Stdio};

#[derive(Clone, Copy, PartialEq)]
enum Level {
    Ok,
    Warn,
    Fail,
}

struct Report {
    fails: usize,
    warns: usize,
}

impl Report {
    fn line(&mut self, level: Level, what: &str, detail: &str) {
        let mark = match level {
            Level::Ok => "\x1b[32m✓\x1b[0m",
            Level::Warn => {
                self.warns += 1;
                "\x1b[33m!\x1b[0m"
            }
            Level::Fail => {
                self.fails += 1;
                "\x1b[31m✗\x1b[0m"
            }
        };
        println!("  {mark} {what:<34} {detail}");
    }

    fn section(&self, title: &str) {
        println!("\n\x1b[1m{title}\x1b[0m");
    }
}

fn output(prog: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(prog)
        .args(args)
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn version_tuple(v: &str) -> Option<(u32, u32)> {
    let v = v.trim_start_matches('v');
    let mut it = v
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty());
    Some((it.next()?.parse().ok()?, it.next()?.parse().ok()?))
}

/// Cerca dispositivi "diretti" (touchscreen) in /proc/bus/input/devices.
pub fn touchscreens() -> Vec<String> {
    let text = std::fs::read_to_string("/proc/bus/input/devices").unwrap_or_default();
    let mut found = Vec::new();
    for block in text.split("\n\n") {
        let name = block
            .lines()
            .find(|l| l.starts_with("N: Name="))
            .map(|l| {
                l.trim_start_matches("N: Name=")
                    .trim_matches('"')
                    .to_string()
            })
            .unwrap_or_default();
        let prop = block
            .lines()
            .find(|l| l.starts_with("B: PROP="))
            .and_then(|l| u64::from_str_radix(l.trim_start_matches("B: PROP=").trim(), 16).ok())
            .unwrap_or(0);
        let has_abs = block.lines().any(|l| l.starts_with("B: ABS="));
        // INPUT_PROP_DIRECT = bit 1; le tavolette grafiche con penna lo hanno ma sono "pen".
        if prop & 0b10 != 0 && has_abs && !name.to_lowercase().contains("pen") {
            found.push(name);
        }
    }
    found
}

pub fn run() -> ExitCode {
    let mut r = Report { fails: 0, warns: 0 };
    println!(
        "\x1b[1mFolio {} — controllo di compatibilità\x1b[0m",
        folio_core::VERSION
    );

    r.section("Sessione");
    match std::env::var("WAYLAND_DISPLAY") {
        Ok(d) => r.line(Level::Ok, "Wayland", &d),
        Err(_) => r.line(
            Level::Fail,
            "Wayland",
            "WAYLAND_DISPLAY non impostata (avvia da una sessione Hyprland)",
        ),
    }
    if hypr::available() {
        let v = hypr::version().unwrap_or_else(|| "?".into());
        match version_tuple(&v) {
            Some((0, minor)) if minor >= 55 => {
                r.line(Level::Ok, "Hyprland", &format!("{v} (configurazione Lua)"))
            }
            Some((0, minor)) if minor >= 50 => r.line(
                Level::Warn,
                "Hyprland",
                &format!("{v}: Folio funziona, ma i dotfile Lua richiedono ≥ 0.55"),
            ),
            Some(_) | None if v != "?" => r.line(Level::Ok, "Hyprland", &v),
            _ => r.line(Level::Warn, "Hyprland", "versione non leggibile"),
        }
        let lua_ok = hypr::request("dispatch hl.dsp.no_op()")
            .map(|s| s.trim() == "ok")
            .unwrap_or(false)
            || hypr::request("eval hl.dispatch(hl.dsp.no_op())")
                .map(|s| s.trim() == "ok")
                .unwrap_or(false);
        r.line(
            if lua_ok { Level::Ok } else { Level::Warn },
            "IPC dispatch",
            if lua_ok {
                "hl.dsp.* (Lua)"
            } else {
                "sintassi classica (fallback attivo)"
            },
        );
        let plugins = hypr::request("plugin list")
            .unwrap_or_default()
            .to_lowercase();
        for (p, why) in [
            ("hyprgrass", "gesti touch"),
            ("hyprbars", "barre del titolo"),
        ] {
            if plugins.contains(p) {
                r.line(Level::Ok, &format!("plugin {p}"), "caricato");
            } else {
                r.line(
                    Level::Warn,
                    &format!("plugin {p}"),
                    &format!("non caricato ({why}): hyprpm enable {p}"),
                );
            }
        }
    } else {
        r.line(
            Level::Fail,
            "Hyprland",
            "non in esecuzione (HYPRLAND_INSTANCE_SIGNATURE assente)",
        );
    }
    let lua = paths::home().join(".config/hypr/hyprland.lua");
    r.line(
        if lua.exists() { Level::Ok } else { Level::Warn },
        "hyprland.lua",
        if lua.exists() {
            "presente"
        } else {
            "assente: installa i dotfile con installer.sh"
        },
    );

    r.section("Grafica");
    if gtk::init().is_ok() {
        r.line(
            Level::Ok,
            "GTK",
            &format!(
                "{}.{}.{}",
                gtk::major_version(),
                gtk::minor_version(),
                gtk::micro_version()
            ),
        );
        if gtk::minor_version() < 12 {
            r.line(Level::Fail, "GTK ≥ 4.12", "richiesto");
        }
        if gtk4_layer_shell::is_supported() {
            r.line(Level::Ok, "wlr-layer-shell", "supportato");
        } else {
            r.line(
                Level::Fail,
                "wlr-layer-shell",
                "non supportato dal compositor",
            );
        }
    } else {
        r.line(
            Level::Fail,
            "GTK",
            "impossibile inizializzare (nessun display?)",
        );
    }

    r.section("Input touch");
    let ts = touchscreens();
    if ts.is_empty() {
        r.line(
            Level::Warn,
            "touchscreen",
            "nessuno rilevato (Folio funziona anche con mouse)",
        );
    } else {
        for t in ts {
            r.line(Level::Ok, "touchscreen", &t);
        }
    }
    let osk = std::env::var("FOLIO_OSK").unwrap_or_else(|_| "wvkbd-mobintl".into());
    r.line(
        if sys::in_path(&osk) {
            Level::Ok
        } else {
            Level::Warn
        },
        "tastiera a schermo",
        if sys::in_path(&osk) {
            &osk
        } else {
            "wvkbd non trovato"
        },
    );
    let iio = sys::in_path("iio-hyprland")
        || std::path::Path::new("/usr/bin/iio-sensor-proxy").exists()
        || std::path::Path::new("/usr/lib/iio-sensor-proxy").exists();
    r.line(
        if iio { Level::Ok } else { Level::Warn },
        "rotazione automatica",
        if iio {
            "sensori disponibili"
        } else {
            "facoltativa: iio-sensor-proxy + iio-hyprland"
        },
    );

    r.section("Strumenti");
    for (tool, why, required) in [
        ("grim", "anteprime del Sommario e istantanee", false),
        ("wpctl", "volume (WirePlumber)", false),
        ("nmcli", "Wi-Fi (NetworkManager)", false),
        ("bluetoothctl", "Bluetooth", false),
        ("playerctl", "controlli musicali", false),
        ("brightnessctl", "luminosità", false),
        ("curl", "meteo", false),
        ("hyprlock", "blocco schermo", false),
        ("hypridle", "inattività", false),
        ("hyprpm", "plugin di Hyprland", true),
        ("gsettings", "tema delle app GTK", false),
        ("fc-list", "verifica dei caratteri", false),
    ] {
        let ok = sys::in_path(tool);
        let level = match (ok, required) {
            (true, _) => Level::Ok,
            (false, true) => Level::Fail,
            (false, false) => Level::Warn,
        };
        r.line(level, tool, why);
    }

    r.section("Caratteri");
    let fonts = output("fc-list", &[":", "family"]).unwrap_or_default();
    for (family, fallback) in [
        ("Instrument Serif", "IBM Plex Serif / serif"),
        ("Inter", "Noto Sans / sans-serif"),
        ("JetBrains Mono", "monospace"),
    ] {
        if fonts
            .lines()
            .any(|l| l.split(',').any(|f| f.trim() == family))
        {
            r.line(Level::Ok, family, "installato");
        } else {
            r.line(
                Level::Warn,
                family,
                &format!("mancante, ripiego su {fallback}"),
            );
        }
    }

    r.section("Folio");
    match ipc::send("ping") {
        Ok(reply) if reply == "ok" => r.line(Level::Ok, "shell", "in esecuzione"),
        _ => r.line(
            Level::Warn,
            "shell",
            "non in esecuzione (si avvia con Hyprland)",
        ),
    }
    let cfg = folio_core::config::Config::path();
    match std::fs::read_to_string(&cfg) {
        Ok(t) => match folio_core::config::Config::parse(&t) {
            Ok(_) => r.line(Level::Ok, "config.toml", "valido"),
            Err(e) => r.line(Level::Fail, "config.toml", &format!("errore: {e}")),
        },
        Err(_) => r.line(Level::Ok, "config.toml", "assente (valori predefiniti)"),
    }

    println!();
    if r.fails > 0 {
        println!(
            "\x1b[31m{} problemi bloccanti\x1b[0m, {} avvisi.",
            r.fails, r.warns
        );
        ExitCode::FAILURE
    } else {
        println!(
            "\x1b[32mTutto pronto\x1b[0m ({} avvisi non bloccanti).",
            r.warns
        );
        ExitCode::SUCCESS
    }
}

#[cfg(test)]
mod tests {
    use super::version_tuple;

    #[test]
    fn versions() {
        assert_eq!(version_tuple("v0.55.1"), Some((0, 55)));
        assert_eq!(version_tuple("0.54.0-b"), Some((0, 54)));
    }
}

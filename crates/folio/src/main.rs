//! Folio — una shell touch per Hyprland impaginata come una rivista di design.
//!
//! `folio`              avvia la shell (testata, dock, copertina, indice, sommario…)
//! `folio msg <cmd>`    invia un comando alla shell in esecuzione
//! `folio doctor`       verifica la compatibilità dell'ambiente
//! `folio print-config` stampa la configurazione predefinita commentabile

mod apps;
mod doctor;
mod glyph;
mod layer;
mod livehub;
mod osk;
mod shell;
mod theme;
mod tile;
mod ui;

use folio_core::{config::Config, ipc};
use gtk::prelude::*;
use std::process::ExitCode;

const USAGE: &str = "\
Folio — shell touch per Hyprland

USO
  folio                  avvia la shell
  folio msg <comando>    invia un comando alla shell in esecuzione
  folio doctor           controlla la compatibilità dell'ambiente
  folio print-config     stampa la configurazione predefinita
  folio date [long|issue|edition|day|month|weekday] [--upper]
  folio --version

COMANDI (folio msg …)
  home [toggle|show|hide|edit]  Indice: widget e tutte le app
  overview [toggle|show|hide]   Sommario: spazi di lavoro e finestre (Recenti)
  shade [toggle|show|hide]      Notiziario: notifiche e impostazioni rapide
  dock [toggle|show|hide]       mostra/nasconde la dock
  chrome [toggle|show|hide]     popup dei comandi della finestra attiva
  layout                        modalità Impaginazione (sposta ogni elemento)
  back                          gesto Indietro (chiude overlay, Esc o Alt+←)
  minimize | restore            riduce / ripristina la finestra attiva
  gesture edge-up <frazione> <ms_fermo>
  theme [toggle|light|dark|auto]
  osk [toggle|show|hide]        tastiera a schermo
  rotate                        ruota schermo e touchscreen di 90°
  close                         chiude tutti gli overlay
  reload                        ricarica configurazione e stile
  quit
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("msg") => {
            if args.len() < 2 {
                eprintln!("uso: folio msg <comando>");
                return ExitCode::from(2);
            }
            match ipc::send(&args[1..].join(" ")) {
                Ok(reply) => {
                    println!("{reply}");
                    if reply == "ok" {
                        ExitCode::SUCCESS
                    } else {
                        ExitCode::FAILURE
                    }
                }
                Err(e) => {
                    eprintln!("Folio non risponde ({e}). È in esecuzione?");
                    ExitCode::FAILURE
                }
            }
        }
        Some("doctor") | Some("check") => doctor::run(),
        Some("date") => {
            // Usato da hyprlock: `folio date long|issue|edition|day|month|weekday`.
            let now = chrono::Local::now();
            let out = match args.get(1).map(String::as_str).unwrap_or("long") {
                "issue" => folio_core::locale::issue(&now),
                "edition" => folio_core::locale::edition(&now).to_string(),
                "day" => now.format("%-d").to_string(),
                "month" => folio_core::locale::month(&now).to_string(),
                "weekday" => folio_core::locale::weekday(&now).to_string(),
                _ => folio_core::locale::long_date(&now),
            };
            let upper = args.iter().any(|a| a == "--upper");
            println!("{}", if upper { out.to_uppercase() } else { out });
            ExitCode::SUCCESS
        }
        Some("print-config") => {
            match toml::to_string_pretty(&Config::default()) {
                Ok(t) => print!("{t}"),
                Err(e) => eprintln!("{e}"),
            }
            ExitCode::SUCCESS
        }
        Some("-V") | Some("--version") | Some("version") => {
            println!("folio {}", folio_core::VERSION);
            ExitCode::SUCCESS
        }
        Some("-h") | Some("--help") | Some("help") => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        None | Some("daemon") => run_shell(),
        Some(other) => {
            eprintln!("argomento sconosciuto: {other}\n");
            eprint!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn run_shell() -> ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();
    log::info!("Folio {}", folio_core::VERSION);

    let app = gtk::Application::builder()
        .application_id("io.github.folio.Shell")
        .build();
    app.connect_activate(shell::start);
    let code = app.run_with_args::<&str>(&[]);
    ipc::cleanup();
    if code == gtk::glib::ExitCode::SUCCESS {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

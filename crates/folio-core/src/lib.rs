//! Folio — nucleo senza dipendenze grafiche.
//!
//! Tutto ciò che può essere testato senza un compositor vive qui:
//! configurazione, stato persistente dell'impaginazione, client IPC di
//! Hyprland, socket di comando di Folio, lettura dei dati di sistema e
//! calcolo dei contenuti "live" dei widget.

pub mod config;
pub mod gesture;
pub mod grid;
pub mod hypr;
pub mod ipc;
pub mod live;
pub mod locale;
pub mod paths;
pub mod state;
pub mod sys;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

//! Interpretazione dei gesti "alla Android tablet".
//!
//! hyprgrass misura il gesto (in Lua, vedi `dotfiles/hypr/folio/gestures.lua`)
//! e Folio decide cosa farne. Tenere la decisione qui la rende testabile e
//! configurabile senza toccare il compositor.

use crate::config::GestureConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Swipe corto: mostra la dock (taskbar transitoria).
    Dock,
    /// Swipe e rilascio: schermata Home (l'Indice).
    Home,
    /// Swipe e pausa: Recenti (il Sommario con gli spazi di lavoro).
    Recents,
    Nothing,
}

/// `fraction`: distanza percorsa verso l'alto in frazione dell'altezza dello schermo.
/// `held_ms`: quanto il dito è rimasto fermo prima del rilascio.
/// `home_open`: se l'Indice è già aperto, lo swipe porta ai Recenti (come su Android).
pub fn edge_up(cfg: &GestureConfig, fraction: f64, held_ms: u32, home_open: bool) -> Outcome {
    if fraction <= 0.01 {
        return Outcome::Nothing;
    }
    if fraction < cfg.dock_threshold {
        return Outcome::Dock;
    }
    if home_open || held_ms >= cfg.hold_ms || fraction >= cfg.recents_threshold {
        return Outcome::Recents;
    }
    Outcome::Home
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn android_semantics() {
        let c = GestureConfig::default();
        assert_eq!(edge_up(&c, 0.04, 0, false), Outcome::Dock);
        assert_eq!(edge_up(&c, 0.25, 30, false), Outcome::Home);
        assert_eq!(edge_up(&c, 0.25, 400, false), Outcome::Recents);
        assert_eq!(edge_up(&c, 0.70, 0, false), Outcome::Recents);
        assert_eq!(edge_up(&c, 0.25, 0, true), Outcome::Recents);
        assert_eq!(edge_up(&c, 0.0, 0, false), Outcome::Nothing);
    }
}

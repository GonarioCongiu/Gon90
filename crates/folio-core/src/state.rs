//! Stato modificato dall'interfaccia (impaginazione, widget, dock).
//! Vive separato dalla configurazione scritta a mano, così le modifiche
//! fatte col dito non riscrivono mai `config.toml`.

use crate::config::TileSpec;
use crate::paths;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Edge {
    Top,
    Bottom,
    Left,
    Right,
    /// Libero, in qualunque punto dello schermo.
    Float,
}

impl Edge {
    pub fn is_vertical_side(self) -> bool {
        matches!(self, Edge::Left | Edge::Right)
    }
}

/// Posizione di un elemento sullo schermo, indipendente dalla risoluzione.
///
/// * Bordi `top`/`bottom`: `x` è il centro dell'elemento in frazione della larghezza
///   (0.5 = centrato).
/// * Bordi `left`/`right`: `y` è il centro in frazione dell'altezza.
/// * `float`: `x`,`y` sono l'angolo in alto a sinistra in frazioni dello schermo.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Placement {
    pub edge: Edge,
    #[serde(default = "half")]
    pub x: f64,
    #[serde(default = "half")]
    pub y: f64,
    /// Occupa tutta la lunghezza del bordo (testata "al vivo").
    #[serde(default)]
    pub stretch: bool,
    /// Orientamento usato quando l'elemento è libero.
    #[serde(default)]
    pub vertical: bool,
}

fn half() -> f64 {
    0.5
}

impl Placement {
    pub const fn docked(edge: Edge, stretch: bool) -> Self {
        Self {
            edge,
            x: 0.5,
            y: 0.5,
            stretch,
            vertical: false,
        }
    }

    pub fn is_vertical(&self) -> bool {
        match self.edge {
            Edge::Left | Edge::Right => true,
            Edge::Top | Edge::Bottom => false,
            Edge::Float => self.vertical,
        }
    }

    pub fn is_centered(&self) -> bool {
        match self.edge {
            Edge::Top | Edge::Bottom => (self.x - 0.5).abs() < 0.02,
            Edge::Left | Edge::Right => (self.y - 0.5).abs() < 0.02,
            Edge::Float => false,
        }
    }

    /// Converte la posizione in margini layer-shell.
    /// Restituisce `(margin_primario, margin_lungo_il_bordo)` dove il secondo è
    /// `None` se l'elemento è centrato o steso.
    pub fn along_margin(&self, monitor: (i32, i32), size: (i32, i32)) -> Option<i32> {
        if self.stretch || self.is_centered() {
            return None;
        }
        match self.edge {
            Edge::Top | Edge::Bottom => {
                let x = (self.x * monitor.0 as f64) as i32 - size.0 / 2;
                Some(x.clamp(0, (monitor.0 - size.0).max(0)))
            }
            Edge::Left | Edge::Right => {
                let y = (self.y * monitor.1 as f64) as i32 - size.1 / 2;
                Some(y.clamp(0, (monitor.1 - size.1).max(0)))
            }
            Edge::Float => None,
        }
    }

    /// Calcola il posizionamento a partire da un rettangolo in pixel, con
    /// aggancio magnetico ai bordi (entro `snap` px).
    pub fn from_rect(
        rect: (i32, i32, i32, i32),
        monitor: (i32, i32),
        snap: i32,
        keep_vertical: bool,
        stretch: bool,
    ) -> Self {
        let (x, y, w, h) = rect;
        let (mw, mh) = (monitor.0.max(1), monitor.1.max(1));
        let cx = (x + w / 2) as f64 / mw as f64;
        let cy = (y + h / 2) as f64 / mh as f64;
        let d_top = y;
        let d_bottom = mh - (y + h);
        let d_left = x;
        let d_right = mw - (x + w);
        let min = d_top.min(d_bottom).min(d_left).min(d_right);
        let snap_center = |v: f64| if (v - 0.5).abs() < 0.03 { 0.5 } else { v };
        if min <= snap {
            let edge = if min == d_top {
                Edge::Top
            } else if min == d_bottom {
                Edge::Bottom
            } else if min == d_left {
                Edge::Left
            } else {
                Edge::Right
            };
            return Self {
                edge,
                x: snap_center(cx),
                y: snap_center(cy),
                stretch,
                vertical: keep_vertical,
            };
        }
        Self {
            edge: Edge::Float,
            x: x as f64 / mw as f64,
            y: y as f64 / mh as f64,
            stretch: false,
            vertical: keep_vertical,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeskWidget {
    pub id: u32,
    pub tile: TileSpec,
    pub placement: Placement,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bar: Option<Placement>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dock: Option<Placement>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dock_pinned: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dark: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tiles: Option<Vec<TileSpec>>,
    pub desk: Vec<DeskWidget>,
}

impl State {
    pub fn path() -> std::path::PathBuf {
        paths::state_dir().join("state.toml")
    }

    pub fn load() -> Self {
        std::fs::read_to_string(Self::path())
            .ok()
            .and_then(|t| match toml::from_str(&t) {
                Ok(s) => Some(s),
                Err(e) => {
                    log::warn!("stato illeggibile, riparto da zero: {e}");
                    None
                }
            })
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let path = Self::path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        match toml::to_string_pretty(self) {
            Ok(text) => {
                // Scrittura atomica: un crash a metà non corrompe l'impaginazione.
                let tmp = path.with_extension("toml.tmp");
                if std::fs::write(&tmp, text).is_ok() {
                    let _ = std::fs::rename(&tmp, &path);
                }
            }
            Err(e) => log::error!("salvataggio stato: {e}"),
        }
    }

    pub fn next_desk_id(&self) -> u32 {
        self.desk.iter().map(|d| d.id).max().unwrap_or(0) + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{TileSize, TileStyle};

    #[test]
    fn snaps_to_nearest_edge() {
        let p = Placement::from_rect((900, 1040, 400, 56), (1920, 1080), 48, false, false);
        assert_eq!(p.edge, Edge::Bottom);
        assert!(
            (p.x - 0.5729).abs() < 0.001,
            "centro ≈ 0.573, fuori dalla zona di aggancio"
        );
        let c = Placement::from_rect((760, 1040, 400, 56), (1920, 1080), 48, false, false);
        assert_eq!(c.x, 0.5, "vicino al centro → centrato");
    }

    #[test]
    fn floats_in_the_middle() {
        let p = Placement::from_rect((600, 400, 300, 60), (1920, 1080), 48, true, false);
        assert_eq!(p.edge, Edge::Float);
        assert!(p.is_vertical());
    }

    #[test]
    fn left_edge_is_vertical() {
        let p = Placement::from_rect((4, 300, 60, 500), (1920, 1080), 48, false, false);
        assert_eq!(p.edge, Edge::Left);
        assert!(p.is_vertical());
    }

    #[test]
    fn state_roundtrip() {
        let mut s = State {
            bar: Some(Placement::docked(Edge::Top, true)),
            ..Default::default()
        };
        s.desk.push(DeskWidget {
            id: 1,
            tile: TileSpec {
                app: None,
                live: None,
                size: TileSize::Medium,
                style: TileStyle::Ink,
                label: None,
            },
            placement: Placement {
                edge: Edge::Float,
                x: 0.1,
                y: 0.2,
                stretch: false,
                vertical: false,
            },
        });
        let text = toml::to_string_pretty(&s).unwrap();
        let back: State = toml::from_str(&text).unwrap();
        assert_eq!(back.desk, s.desk);
        assert_eq!(back.bar, s.bar);
    }
}

pub mod bar;
pub mod chrome;
pub mod cover;
pub mod desk;
pub mod dock;
pub mod home;
pub mod layout;
pub mod notify;
pub mod overview;
pub mod shade;

use folio_core::state::Placement;
use gtk::prelude::*;

/// Un elemento che la modalità Impaginazione può spostare.
pub trait Placeable {
    fn label(&self) -> String;
    fn placement(&self) -> Placement;
    /// Applica e salva la nuova posizione.
    fn set_placement(&self, p: Placement);
    /// Dimensione attuale sullo schermo.
    fn size(&self) -> (i32, i32);
    fn margin(&self) -> i32;
    fn reset(&self);
    fn can_stretch(&self) -> bool {
        false
    }
}

/// Svuota un contenitore.
pub fn clear(container: &gtk::Box) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}

/// Etichetta con classi CSS.
pub fn text(content: &str, css: &[&str]) -> gtk::Label {
    let l = gtk::Label::new(Some(content));
    for c in css {
        l.add_css_class(c);
    }
    l
}

/// Filetto orizzontale (o verticale) da un pixel.
pub fn rule(vertical: bool) -> gtk::Separator {
    let s = gtk::Separator::new(if vertical {
        gtk::Orientation::Vertical
    } else {
        gtk::Orientation::Horizontal
    });
    s.add_css_class("rule");
    s
}

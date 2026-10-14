//! Superfici layer-shell e posizionamento libero degli elementi.

use crate::shell::{shell, try_shell};
use folio_core::state::{Edge as PEdge, Placement};
use gtk::gdk;
use gtk::prelude::*;
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

pub use gtk4_layer_shell::{KeyboardMode as Keyboard, Layer as Level};

/// Crea una finestra layer-shell trasparente con la classe `.folio`.
pub fn surface(namespace: &str, layer: Layer, keyboard: KeyboardMode) -> gtk::Window {
    let window = gtk::Window::new();
    if let Some(s) = try_shell() {
        window.set_application(Some(&s.app));
    }
    window.set_decorated(false);
    window.set_title(Some(namespace));
    window.add_css_class("folio");
    window.add_css_class(namespace);
    if gtk4_layer_shell::is_supported() {
        window.init_layer_shell();
        window.set_layer(layer);
        window.set_namespace(Some(namespace));
        window.set_keyboard_mode(keyboard);
    }
    window
}

/// Ancora la superficie a tutti i bordi ignorando le zone riservate:
/// è il modo di coprire l'intero schermo (overlay, copertina, impaginazione).
pub fn fill_screen(window: &gtk::Window) {
    for e in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
        window.set_anchor(e, true);
        window.set_margin(e, 0);
    }
    window.set_exclusive_zone(-1);
}

/// Come `fill_screen`, ma rispetta le zone riservate (Testata, Dock):
/// la Copertina occupa solo la "pagina" libera.
pub fn fill_free_area(window: &gtk::Window) {
    for e in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
        window.set_anchor(e, true);
        window.set_margin(e, 0);
    }
    window.set_exclusive_zone(0);
}

/// Monitor su cui mostrare gli overlay: quello che Hyprland considera attivo.
pub fn target_monitor() -> Option<gdk::Monitor> {
    let display = gdk::Display::default()?;
    let monitors = display.monitors();
    let all: Vec<gdk::Monitor> = (0..monitors.n_items())
        .filter_map(|i| monitors.item(i).and_downcast::<gdk::Monitor>())
        .collect();
    let wanted =
        try_shell().and_then(|s| s.hypr.borrow().focused_monitor().map(|m| m.name.clone()));
    if let Some(name) = wanted {
        if let Some(m) = all
            .iter()
            .find(|m| m.connector().map(|c| c.as_str() == name).unwrap_or(false))
        {
            return Some(m.clone());
        }
    }
    all.into_iter().next()
}

/// Dimensioni logiche del monitor di destinazione.
pub fn monitor_size() -> (i32, i32) {
    target_monitor()
        .map(|m| {
            let g = m.geometry();
            (g.width(), g.height())
        })
        .unwrap_or((1920, 1080))
}

/// Origine logica del monitor di destinazione (per convertire coordinate globali di Hyprland).
pub fn monitor_origin() -> (i32, i32) {
    let s = shell();
    let snap = s.hypr.borrow();
    snap.focused_monitor().map(|m| (m.x, m.y)).unwrap_or((0, 0))
}

pub fn on_monitor(window: &gtk::Window) {
    if gtk4_layer_shell::is_supported() {
        window.set_monitor(target_monitor().as_ref());
    }
}

/// Dimensione naturale di un widget (anche prima che sia mostrato).
pub fn natural_size(widget: &impl IsA<gtk::Widget>) -> (i32, i32) {
    let w = widget.as_ref();
    if w.width() > 0 && w.height() > 0 {
        return (w.width(), w.height());
    }
    let (_, nat) = w.preferred_size();
    (nat.width().max(1), nat.height().max(1))
}

/// Traduce un `Placement` in ancore e margini layer-shell.
pub fn apply_placement(
    window: &gtk::Window,
    p: &Placement,
    size: (i32, i32),
    margin: i32,
    reserve: bool,
) {
    if !gtk4_layer_shell::is_supported() {
        return;
    }
    let mon = monitor_size();
    for e in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
        window.set_anchor(e, false);
        window.set_margin(e, 0);
    }
    match p.edge {
        PEdge::Top | PEdge::Bottom => {
            let e = if p.edge == PEdge::Top {
                Edge::Top
            } else {
                Edge::Bottom
            };
            window.set_anchor(e, true);
            window.set_margin(e, margin);
            if p.stretch {
                window.set_anchor(Edge::Left, true);
                window.set_anchor(Edge::Right, true);
                window.set_margin(Edge::Left, margin);
                window.set_margin(Edge::Right, margin);
            } else if let Some(m) = p.along_margin(mon, size) {
                window.set_anchor(Edge::Left, true);
                window.set_margin(Edge::Left, m);
            }
        }
        PEdge::Left | PEdge::Right => {
            let e = if p.edge == PEdge::Left {
                Edge::Left
            } else {
                Edge::Right
            };
            window.set_anchor(e, true);
            window.set_margin(e, margin);
            if p.stretch {
                window.set_anchor(Edge::Top, true);
                window.set_anchor(Edge::Bottom, true);
                window.set_margin(Edge::Top, margin);
                window.set_margin(Edge::Bottom, margin);
            } else if let Some(m) = p.along_margin(mon, size) {
                window.set_anchor(Edge::Top, true);
                window.set_margin(Edge::Top, m);
            }
        }
        PEdge::Float => {
            window.set_anchor(Edge::Top, true);
            window.set_anchor(Edge::Left, true);
            let x = ((p.x * mon.0 as f64) as i32).clamp(0, (mon.0 - size.0).max(0));
            let y = ((p.y * mon.1 as f64) as i32).clamp(0, (mon.1 - size.1).max(0));
            window.set_margin(Edge::Left, x);
            window.set_margin(Edge::Top, y);
        }
    }
    if reserve && p.edge != PEdge::Float {
        window.auto_exclusive_zone_enable();
    } else {
        window.set_exclusive_zone(0);
    }
}

/// Rettangolo (x, y, w, h) occupato sullo schermo da un elemento posizionato.
pub fn placement_rect(p: &Placement, size: (i32, i32), margin: i32) -> (i32, i32, i32, i32) {
    let (mw, mh) = monitor_size();
    let (w, h) = size;
    let centered_x = (mw - w) / 2;
    let centered_y = (mh - h) / 2;
    match p.edge {
        PEdge::Top | PEdge::Bottom => {
            let (x, w) = if p.stretch {
                (margin, mw - 2 * margin)
            } else {
                (p.along_margin((mw, mh), size).unwrap_or(centered_x), w)
            };
            let y = if p.edge == PEdge::Top {
                margin
            } else {
                mh - h - margin
            };
            (x, y, w, h)
        }
        PEdge::Left | PEdge::Right => {
            let (y, h) = if p.stretch {
                (margin, mh - 2 * margin)
            } else {
                (p.along_margin((mw, mh), size).unwrap_or(centered_y), h)
            };
            let x = if p.edge == PEdge::Left {
                margin
            } else {
                mw - w - margin
            };
            (x, y, w, h)
        }
        PEdge::Float => (
            ((p.x * mw as f64) as i32).clamp(0, (mw - w).max(0)),
            ((p.y * mh as f64) as i32).clamp(0, (mh - h).max(0)),
            w,
            h,
        ),
    }
}

/// Spazio occupato ai bordi da Testata e Dock agganciate: [alto, basso, sinistra, destra].
pub fn reserved_insets() -> [i32; 4] {
    use crate::ui::Placeable;
    let s = shell();
    let mut inset = [0i32; 4];
    let mut reserve = |edge: PEdge, size: (i32, i32), margin: i32| match edge {
        PEdge::Top => inset[0] = inset[0].max(size.1 + margin),
        PEdge::Bottom => inset[1] = inset[1].max(size.1 + margin),
        PEdge::Left => inset[2] = inset[2].max(size.0 + margin),
        PEdge::Right => inset[3] = inset[3].max(size.0 + margin),
        PEdge::Float => {}
    };
    if let Some(bar) = s.bar.get() {
        reserve(bar.placement().edge, bar.size(), bar.margin());
    }
    let persistent = s.config.borrow().dock.mode == folio_core::config::DockMode::Persistent;
    if let (Some(dock), true) = (s.dock.get(), persistent) {
        reserve(dock.placement().edge, dock.size(), dock.margin());
    }
    inset
}

/// Esc chiude: comodo con una tastiera collegata.
pub fn close_on_escape(window: &gtk::Window, on_close: impl Fn() + 'static) {
    let keys = gtk::EventControllerKey::new();
    keys.connect_key_pressed(move |_, key, _, _| {
        if key == gdk::Key::Escape {
            on_close();
            gtk::glib::Propagation::Stop
        } else {
            gtk::glib::Propagation::Proceed
        }
    });
    window.add_controller(keys);
}

/// Il tocco arriva da un touchscreen? (per decidere se mostrare la tastiera a schermo)
pub fn is_touch(gesture: &impl IsA<gtk::Gesture>) -> bool {
    gesture
        .as_ref()
        .device()
        .map(|d| d.source() == gdk::InputSource::Touchscreen)
        .unwrap_or(false)
}

/// Long-press che non lascia passare il "clic" successivo.
pub fn on_long_press(
    widget: &impl IsA<gtk::Widget>,
    f: impl Fn(f64, f64) + 'static,
) -> gtk::GestureLongPress {
    let lp = gtk::GestureLongPress::new();
    lp.set_propagation_phase(gtk::PropagationPhase::Capture);
    lp.connect_pressed(move |g, x, y| {
        g.set_state(gtk::EventSequenceState::Claimed);
        f(x, y);
    });
    widget.as_ref().add_controller(lp.clone());
    lp
}

/// Tocco singolo (rilascio) — funziona uguale con dito, penna e mouse.
pub fn on_tap(widget: &impl IsA<gtk::Widget>, f: impl Fn() + 'static) -> gtk::GestureClick {
    let click = gtk::GestureClick::new();
    click.set_button(0);
    click.connect_released(move |g, n, _, _| {
        if n == 1 && g.current_button() <= 1 {
            f();
        }
    });
    widget.as_ref().add_controller(click.clone());
    click
}

/// Swipe con soglia di velocità; `f` riceve (vx, vy) in px/s.
pub fn on_swipe(
    widget: &impl IsA<gtk::Widget>,
    f: impl Fn(f64, f64) + 'static,
) -> gtk::GestureSwipe {
    let swipe = gtk::GestureSwipe::new();
    swipe.connect_swipe(move |_, vx, vy| f(vx, vy));
    widget.as_ref().add_controller(swipe.clone());
    swipe
}

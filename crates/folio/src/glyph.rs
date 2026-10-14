//! Iconografia disegnata a mano con cairo: tratto sottile, terminali arrotondati,
//! griglia 24×24. Prende il colore dal CSS del widget, quindi segue il tema.

use gtk::cairo::{Context, LineCap, LineJoin};
use gtk::prelude::*;
use std::f64::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Glyph {
    Close,
    Minimize,
    Maximize,
    Fullscreen,
    Float,
    Pin,
    SplitLeft,
    SplitRight,
    Center,
    Grid,
    Pages,
    Wifi,
    Bluetooth,
    Volume,
    VolumeMute,
    Sun,
    Moon,
    Power,
    Lock,
    Sleep,
    Restart,
    Logout,
    Keyboard,
    Rotate,
    Camera,
    Bell,
    BellOff,
    Layout,
    Play,
    Pause,
    Next,
    Prev,
    Plus,
    Resize,
    Palette,
    Desk,
    Dock,
    Battery,
}

/// Restituisce un'area di disegno quadrata con il glifo.
pub fn glyph(g: Glyph, size: i32) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.set_content_width(size);
    area.set_content_height(size);
    area.set_halign(gtk::Align::Center);
    area.set_valign(gtk::Align::Center);
    area.add_css_class("glyph");
    area.set_draw_func(move |a, cr, w, h| draw(g, a, cr, w, h));
    area
}

/// Pulsante piatto con glifo ed eventuale didascalia.
pub fn button(g: Glyph, size: i32, caption: Option<&str>) -> gtk::Button {
    let content = gtk::Box::new(gtk::Orientation::Vertical, 6);
    content.append(&glyph(g, size));
    if let Some(text) = caption {
        let l = gtk::Label::new(Some(text));
        l.add_css_class("caption");
        content.append(&l);
    }
    let b = gtk::Button::builder().child(&content).build();
    b.add_css_class("btn");
    b.add_css_class("glyph-btn");
    b
}

fn draw(g: Glyph, area: &gtk::DrawingArea, cr: &Context, w: i32, h: i32) {
    let c = area.color();
    cr.set_source_rgba(
        c.red() as f64,
        c.green() as f64,
        c.blue() as f64,
        c.alpha() as f64,
    );
    let s = (w.min(h) as f64) / 24.0;
    cr.translate((w as f64 - 24.0 * s) / 2.0, (h as f64 - 24.0 * s) / 2.0);
    cr.scale(s, s);
    cr.set_line_width(1.6);
    cr.set_line_cap(LineCap::Round);
    cr.set_line_join(LineJoin::Round);

    let line = |x1: f64, y1: f64, x2: f64, y2: f64| {
        cr.move_to(x1, y1);
        cr.line_to(x2, y2);
    };
    let rect = |x: f64, y: f64, w: f64, h: f64| cr.rectangle(x, y, w, h);
    let circle = |x: f64, y: f64, r: f64| {
        cr.new_sub_path();
        cr.arc(x, y, r, 0.0, 2.0 * PI);
    };
    let dot = |x: f64, y: f64, r: f64| {
        cr.new_sub_path();
        cr.arc(x, y, r, 0.0, 2.0 * PI);
        let _ = cr.fill();
    };

    match g {
        Glyph::Close => {
            line(6.0, 6.0, 18.0, 18.0);
            line(18.0, 6.0, 6.0, 18.0);
        }
        Glyph::Minimize => line(5.0, 17.0, 19.0, 17.0),
        Glyph::Maximize => rect(5.0, 5.0, 14.0, 14.0),
        Glyph::Fullscreen => {
            for (x, y, dx, dy) in [
                (4.0, 4.0, 1.0, 1.0),
                (20.0, 4.0, -1.0, 1.0),
                (4.0, 20.0, 1.0, -1.0),
                (20.0, 20.0, -1.0, -1.0),
            ] {
                cr.move_to(x + 5.0 * dx, y);
                cr.line_to(x, y);
                cr.line_to(x, y + 5.0 * dy);
            }
        }
        Glyph::Float => {
            rect(4.0, 8.0, 11.0, 11.0);
            cr.move_to(9.0, 8.0);
            cr.line_to(9.0, 4.0);
            cr.line_to(20.0, 4.0);
            cr.line_to(20.0, 15.0);
            cr.line_to(15.0, 15.0);
        }
        Glyph::Pin => {
            circle(12.0, 8.0, 4.0);
            line(12.0, 12.0, 12.0, 21.0);
            line(8.0, 14.0, 16.0, 14.0);
        }
        Glyph::SplitLeft | Glyph::SplitRight => {
            rect(4.0, 5.0, 16.0, 14.0);
            let _ = cr.stroke();
            let x = if g == Glyph::SplitLeft { 4.0 } else { 12.0 };
            rect(x, 5.0, 8.0, 14.0);
            let _ = cr.fill();
        }
        Glyph::Center => {
            rect(4.0, 4.0, 16.0, 16.0);
            let _ = cr.stroke();
            rect(8.5, 8.5, 7.0, 7.0);
            let _ = cr.fill();
        }
        Glyph::Grid => {
            for i in 0..3 {
                for j in 0..3 {
                    dot(6.0 + 6.0 * i as f64, 6.0 + 6.0 * j as f64, 1.6);
                }
            }
        }
        Glyph::Pages => {
            rect(3.0, 7.0, 7.0, 10.0);
            rect(14.0, 7.0, 7.0, 10.0);
            line(12.0, 5.0, 12.0, 19.0);
        }
        Glyph::Wifi => {
            for r in [4.0, 8.0, 12.0] {
                cr.new_sub_path();
                cr.arc(12.0, 19.0, r, PI * 1.25, PI * 1.75);
            }
            let _ = cr.stroke();
            dot(12.0, 19.0, 1.5);
        }
        Glyph::Bluetooth => {
            cr.move_to(7.0, 8.0);
            cr.line_to(17.0, 16.0);
            cr.line_to(12.0, 20.0);
            cr.line_to(12.0, 4.0);
            cr.line_to(17.0, 8.0);
            cr.line_to(7.0, 16.0);
        }
        Glyph::Volume | Glyph::VolumeMute => {
            cr.move_to(4.0, 10.0);
            cr.line_to(8.0, 10.0);
            cr.line_to(13.0, 5.0);
            cr.line_to(13.0, 19.0);
            cr.line_to(8.0, 14.0);
            cr.line_to(4.0, 14.0);
            cr.close_path();
            if g == Glyph::Volume {
                cr.new_sub_path();
                cr.arc(13.0, 12.0, 4.0, -PI / 3.0, PI / 3.0);
                cr.new_sub_path();
                cr.arc(13.0, 12.0, 7.5, -PI / 3.0, PI / 3.0);
            } else {
                line(16.5, 9.5, 21.0, 14.5);
                line(21.0, 9.5, 16.5, 14.5);
            }
        }
        Glyph::Sun => {
            circle(12.0, 12.0, 4.0);
            for i in 0..8 {
                let a = i as f64 * PI / 4.0;
                line(
                    12.0 + 7.0 * a.cos(),
                    12.0 + 7.0 * a.sin(),
                    12.0 + 9.5 * a.cos(),
                    12.0 + 9.5 * a.sin(),
                );
            }
        }
        Glyph::Moon => {
            cr.new_sub_path();
            cr.arc(12.0, 12.0, 8.0, PI * 0.35, PI * 1.65);
            cr.arc_negative(15.5, 10.0, 6.5, PI * 1.35, PI * 0.62);
            cr.close_path();
        }
        Glyph::Power => {
            cr.new_sub_path();
            cr.arc(12.0, 13.0, 7.5, -PI * 0.3, PI * 1.3);
            line(12.0, 3.5, 12.0, 11.0);
        }
        Glyph::Lock => {
            rect(5.0, 11.0, 14.0, 10.0);
            cr.move_to(8.0, 11.0);
            cr.line_to(8.0, 8.0);
            cr.arc(12.0, 8.0, 4.0, PI, 2.0 * PI);
            cr.line_to(16.0, 11.0);
        }
        Glyph::Sleep => {
            cr.move_to(6.0, 7.0);
            cr.line_to(12.0, 7.0);
            cr.line_to(6.0, 15.0);
            cr.line_to(12.0, 15.0);
            cr.move_to(14.0, 13.0);
            cr.line_to(18.0, 13.0);
            cr.line_to(14.0, 18.0);
            cr.line_to(18.0, 18.0);
        }
        Glyph::Restart => {
            cr.new_sub_path();
            cr.arc(12.0, 12.0, 7.5, -PI * 0.4, PI * 1.45);
            cr.move_to(17.0, 3.5);
            cr.line_to(16.6, 6.2);
            cr.line_to(19.4, 6.6);
        }
        Glyph::Logout => {
            cr.move_to(13.0, 4.0);
            cr.line_to(5.0, 4.0);
            cr.line_to(5.0, 20.0);
            cr.line_to(13.0, 20.0);
            line(10.0, 12.0, 20.0, 12.0);
            cr.move_to(16.5, 8.5);
            cr.line_to(20.0, 12.0);
            cr.line_to(16.5, 15.5);
        }
        Glyph::Keyboard => {
            rect(3.0, 6.0, 18.0, 12.0);
            let _ = cr.stroke();
            for i in 0..4 {
                dot(6.5 + 3.7 * i as f64, 10.0, 0.9);
            }
            line(8.0, 14.5, 16.0, 14.5);
        }
        Glyph::Rotate => {
            rect(7.0, 3.0, 10.0, 16.0);
            let _ = cr.stroke();
            cr.new_sub_path();
            cr.arc(12.0, 12.0, 10.0, PI * 0.15, PI * 0.55);
            cr.move_to(19.0, 20.5);
            cr.line_to(21.4, 17.7);
            cr.line_to(18.0, 16.6);
        }
        Glyph::Camera => {
            rect(3.0, 7.0, 18.0, 13.0);
            circle(12.0, 13.5, 3.5);
            cr.move_to(8.0, 7.0);
            cr.line_to(9.5, 4.5);
            cr.line_to(14.5, 4.5);
            cr.line_to(16.0, 7.0);
        }
        Glyph::Bell | Glyph::BellOff => {
            cr.move_to(6.0, 17.0);
            cr.line_to(6.0, 11.0);
            cr.arc(12.0, 11.0, 6.0, PI, 2.0 * PI);
            cr.line_to(18.0, 17.0);
            cr.close_path();
            line(4.0, 17.0, 20.0, 17.0);
            line(10.5, 20.0, 13.5, 20.0);
            if g == Glyph::BellOff {
                line(4.0, 4.0, 20.0, 20.0);
            }
        }
        Glyph::Layout => {
            rect(3.0, 4.0, 18.0, 16.0);
            line(3.0, 9.0, 21.0, 9.0);
            line(10.0, 9.0, 10.0, 20.0);
        }
        Glyph::Play => {
            cr.move_to(8.0, 5.0);
            cr.line_to(19.0, 12.0);
            cr.line_to(8.0, 19.0);
            cr.close_path();
            let _ = cr.fill();
        }
        Glyph::Pause => {
            rect(7.0, 5.0, 3.0, 14.0);
            rect(14.0, 5.0, 3.0, 14.0);
            let _ = cr.fill();
        }
        Glyph::Next | Glyph::Prev => {
            if g == Glyph::Prev {
                cr.translate(24.0, 0.0);
                cr.scale(-1.0, 1.0);
            }
            cr.move_to(6.0, 6.0);
            cr.line_to(15.0, 12.0);
            cr.line_to(6.0, 18.0);
            cr.close_path();
            let _ = cr.fill();
            rect(16.0, 6.0, 2.2, 12.0);
            let _ = cr.fill();
        }
        Glyph::Plus => {
            line(12.0, 5.0, 12.0, 19.0);
            line(5.0, 12.0, 19.0, 12.0);
        }
        Glyph::Resize => {
            cr.move_to(13.0, 4.0);
            cr.line_to(20.0, 4.0);
            cr.line_to(20.0, 11.0);
            cr.move_to(11.0, 20.0);
            cr.line_to(4.0, 20.0);
            cr.line_to(4.0, 13.0);
            line(20.0, 4.0, 14.0, 10.0);
            line(4.0, 20.0, 10.0, 14.0);
        }
        Glyph::Palette => {
            circle(12.0, 12.0, 8.0);
            let _ = cr.stroke();
            cr.new_sub_path();
            cr.arc(12.0, 12.0, 8.0, PI * 0.5, PI * 1.5);
            cr.close_path();
            let _ = cr.fill();
        }
        Glyph::Desk => {
            rect(3.0, 4.0, 18.0, 13.0);
            line(8.0, 21.0, 16.0, 21.0);
            line(12.0, 17.0, 12.0, 21.0);
            let _ = cr.stroke();
            rect(6.0, 7.0, 5.0, 4.0);
            let _ = cr.fill();
        }
        Glyph::Dock => {
            rect(3.0, 4.0, 18.0, 16.0);
            let _ = cr.stroke();
            rect(6.0, 15.0, 12.0, 2.5);
            let _ = cr.fill();
        }
        Glyph::Battery => {
            rect(3.0, 8.0, 16.0, 9.0);
            line(21.0, 11.0, 21.0, 14.0);
        }
    }
    let _ = cr.stroke();
}

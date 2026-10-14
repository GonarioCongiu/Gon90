//! Tastiera a schermo (wvkbd): compare quando si tocca un campo di testo di
//! Folio col dito, non quando lo si usa con una tastiera fisica.

use crate::layer;
use folio_core::config::OskConfig;
use folio_core::sys;
use gtk::glib;
use gtk::prelude::*;
use std::cell::RefCell;
use std::time::Duration;

pub struct Osk {
    cfg: RefCell<OskConfig>,
}

impl Osk {
    pub fn new(cfg: &OskConfig) -> Self {
        Self {
            cfg: RefCell::new(cfg.clone()),
        }
    }

    fn enabled(&self) -> bool {
        self.cfg.borrow().enabled
    }

    pub fn show(&self) {
        if !self.enabled() {
            return;
        }
        let cfg = self.cfg.borrow().clone();
        if sys::is_running(&cfg.process) {
            sys::signal(&cfg.process, "USR2");
        } else if sys::in_path(&cfg.process) {
            sys::spawn(&cfg.command);
            // wvkbd parte nascosta (--hidden): la mostriamo appena è pronta.
            glib::timeout_add_local_once(Duration::from_millis(450), move || {
                sys::signal(&cfg.process, "USR2");
            });
        }
    }

    pub fn hide(&self) {
        if self.enabled() {
            sys::signal(&self.cfg.borrow().process, "USR1");
        }
    }

    pub fn toggle(&self) {
        if !self.enabled() {
            return;
        }
        let p = self.cfg.borrow().process.clone();
        if !sys::signal(&p, "RTMIN") {
            self.show();
        }
    }

    /// Collega un campo di testo: tocco col dito → tastiera; fuoco perso → via.
    pub fn attach(&self, entry: &impl IsA<gtk::Widget>) {
        let press = gtk::GestureClick::new();
        press.set_propagation_phase(gtk::PropagationPhase::Capture);
        press.connect_pressed(|g, _, _, _| {
            let s = crate::shell::shell();
            if s.config.borrow().osk.auto_show && layer::is_touch(g) {
                s.osk.show();
            }
        });
        entry.as_ref().add_controller(press);
        let focus = gtk::EventControllerFocus::new();
        focus.connect_leave(|_| crate::shell::shell().osk.hide());
        entry.as_ref().add_controller(focus);
    }
}

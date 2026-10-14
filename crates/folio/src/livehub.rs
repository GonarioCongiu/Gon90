//! Abbonamenti ai contenuti vivi: ogni icona-widget registra una funzione di
//! aggiornamento che restituisce `false` quando il widget non esiste più.

use folio_core::live::LiveKind;
use std::cell::RefCell;

struct Sub {
    kind: LiveKind,
    refresh: Box<dyn Fn() -> bool>,
}

#[derive(Default)]
pub struct LiveHub {
    subs: RefCell<Vec<Sub>>,
}

impl LiveHub {
    pub fn subscribe(&self, kind: LiveKind, refresh: impl Fn() -> bool + 'static) {
        self.subs.borrow_mut().push(Sub {
            kind,
            refresh: Box::new(refresh),
        });
    }

    /// `only_ticking`: aggiorna solo ciò che cambia ogni secondo (l'orologio).
    pub fn refresh(&self, only_ticking: bool) {
        // Le callback possono registrare nuovi abbonati: si lavora su una copia.
        let subs = std::mem::take(&mut *self.subs.borrow_mut());
        let kept: Vec<Sub> = subs
            .into_iter()
            .filter(|s| {
                if only_ticking && !s.kind.ticks() {
                    true
                } else {
                    (s.refresh)()
                }
            })
            .collect();
        let mut current = self.subs.borrow_mut();
        let added = std::mem::take(&mut *current);
        *current = kept;
        current.extend(added);
    }
}

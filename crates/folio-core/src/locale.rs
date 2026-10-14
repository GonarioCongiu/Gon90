//! Date "da rivista" in italiano.

use chrono::{Datelike, Timelike};

pub const WEEKDAYS: [&str; 7] = [
    "Lunedì",
    "Martedì",
    "Mercoledì",
    "Giovedì",
    "Venerdì",
    "Sabato",
    "Domenica",
];
pub const MONTHS: [&str; 12] = [
    "Gennaio",
    "Febbraio",
    "Marzo",
    "Aprile",
    "Maggio",
    "Giugno",
    "Luglio",
    "Agosto",
    "Settembre",
    "Ottobre",
    "Novembre",
    "Dicembre",
];

pub fn weekday<D: Datelike>(d: &D) -> &'static str {
    WEEKDAYS[d.weekday().num_days_from_monday() as usize]
}

pub fn month<D: Datelike>(d: &D) -> &'static str {
    MONTHS[d.month0() as usize]
}

/// "MAR" — abbreviazione maiuscola per occhielli e testate strette.
pub fn weekday_short<D: Datelike>(d: &D) -> String {
    weekday(d)
        .chars()
        .take(3)
        .collect::<String>()
        .to_uppercase()
}

/// "Martedì 14 Ottobre"
pub fn long_date<D: Datelike>(d: &D) -> String {
    format!("{} {} {}", weekday(d), d.day(), month(d))
}

/// "N° 287 · Vol. XXVI" — numero del giorno nell'anno e volume dell'anno.
pub fn issue<D: Datelike>(d: &D) -> String {
    format!(
        "N° {} · Vol. {}",
        d.ordinal(),
        roman((d.year() - 2000).max(1) as u32)
    )
}

pub fn edition<T: Timelike>(t: &T) -> &'static str {
    match t.hour() {
        5..=11 => "Edizione del mattino",
        12..=17 => "Edizione del pomeriggio",
        18..=22 => "Edizione della sera",
        _ => "Edizione della notte",
    }
}

pub fn roman(mut n: u32) -> String {
    const TABLE: [(u32, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut out = String::new();
    for (value, sym) in TABLE {
        while n >= value {
            out.push_str(sym);
            n -= value;
        }
    }
    out
}

pub fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("{n} {one}")
    } else {
        format!("{n} {many}")
    }
}

/// "3 h 12 min"
pub fn duration(secs: u64) -> String {
    let d = secs / 86_400;
    let h = (secs % 86_400) / 3600;
    let m = (secs % 3600) / 60;
    if d > 0 {
        format!("{d} g {h} h")
    } else if h > 0 {
        format!("{h} h {m:02} min")
    } else {
        format!("{m} min")
    }
}

/// Numerazione a due cifre usata ovunque per spazi e widget: "01", "02"…
pub fn folio_number(n: usize) -> String {
    format!("{n:02}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn italian_dates() {
        let d = NaiveDate::from_ymd_opt(2026, 10, 14).unwrap();
        assert_eq!(long_date(&d), "Mercoledì 14 Ottobre");
        assert_eq!(weekday_short(&d), "MER");
        assert_eq!(issue(&d), "N° 287 · Vol. XXVI");
    }

    #[test]
    fn roman_numerals() {
        assert_eq!(roman(26), "XXVI");
        assert_eq!(roman(1994), "MCMXCIV");
    }
}

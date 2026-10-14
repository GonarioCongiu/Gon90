//! Impaginazione dei widget: riempimento "first-fit" su una griglia a colonne,
//! come una gabbia tipografica. Le tile più grandi trovano il primo spazio
//! libero leggendo da sinistra a destra, dall'alto in basso.

use crate::config::TileSize;

/// Restituisce per ogni tile (colonna, riga, larghezza, altezza) in celle.
pub fn pack(sizes: &[TileSize], columns: u32) -> Vec<(u32, u32, u32, u32)> {
    let cols = columns.max(1) as usize;
    let mut occupied: Vec<Vec<bool>> = Vec::new();
    let mut out = Vec::with_capacity(sizes.len());
    for size in sizes {
        let (w, h) = size.span();
        let (w, h) = ((w as usize).min(cols), h as usize);
        let mut row = 0usize;
        'search: loop {
            while occupied.len() < row + h {
                occupied.push(vec![false; cols]);
            }
            for col in 0..=(cols - w) {
                let free = (row..row + h).all(|r| (col..col + w).all(|c| !occupied[r][c]));
                if free {
                    for cells in occupied.iter_mut().skip(row).take(h) {
                        for cell in cells.iter_mut().skip(col).take(w) {
                            *cell = true;
                        }
                    }
                    out.push((col as u32, row as u32, w as u32, h as u32));
                    break 'search;
                }
            }
            row += 1;
        }
    }
    out
}

/// Numero di righe occupate.
pub fn rows(layout: &[(u32, u32, u32, u32)]) -> u32 {
    layout.iter().map(|(_, r, _, h)| r + h).max().unwrap_or(0)
}

/// Colonne che stanno in `width` px, arrotondate a un numero pari (gabbia simmetrica).
pub fn columns_for(width: i32, cell: i32, gap: i32) -> u32 {
    let n = ((width + gap) / (cell + gap).max(1)).clamp(4, 16) as u32;
    if n > 4 && n % 2 == 1 {
        n - 1
    } else {
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use TileSize::*;

    #[test]
    fn fills_holes() {
        // Grande 4×4, poi due medi accanto, poi i piccoli riempiono i buchi.
        let l = pack(&[Large, Medium, Medium, Small, Small], 8);
        assert_eq!(l[0], (0, 0, 4, 4));
        assert_eq!(l[1], (4, 0, 2, 2));
        assert_eq!(l[2], (6, 0, 2, 2));
        assert_eq!(l[3], (4, 2, 1, 1));
        assert_eq!(l[4], (5, 2, 1, 1));
        assert_eq!(rows(&l), 4);
    }

    #[test]
    fn narrow_screens_clamp_spans() {
        let l = pack(&[Wide, Large], 2);
        assert_eq!(l[0], (0, 0, 2, 2));
        assert_eq!(l[1], (0, 2, 2, 4));
    }

    #[test]
    fn even_columns() {
        assert_eq!(columns_for(1800, 92, 12), 16);
        assert_eq!(columns_for(1000, 92, 12), 8);
        assert_eq!(columns_for(300, 92, 12), 4);
    }
}

//! Static cached-value boundary for pinned DesignCraft XLSX/table import.
//!
//! `donor_cell_ref` is adapted from `storytold/designcraft` commit
//! `a9124a0517594ce9872f73637e7b841a4967bcf9`,
//! `crates/textimport/src/xlsx.rs` (MIT OR Apache-2.0; notices in
//! `vendor/designcraft-selected/`). The donor's `records()` yields cached
//! display strings, not formula source or a calculation engine. This module
//! never promotes an imported string beginning `=` into a live formula.

use crate::{CellAddr, CellInput, MAX_CELLS, MAX_COLS, Workbook};

/// Donor code: "B12" -> (zero-based row 11, column 1).
pub(crate) fn donor_cell_ref(r: &str) -> Option<(usize, usize)> {
    let letters: String = r.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    let row: usize = r[letters.len()..].parse().ok()?;
    let col = letters.chars().try_fold(0usize, |acc, c| {
        let digit = (c.to_ascii_uppercase() as u8).checked_sub(b'A')? as usize + 1;
        acc.checked_mul(26)?.checked_add(digit)
    })?;
    Some((row.checked_sub(1)?, col.checked_sub(1)?))
}

/// Convert the donor's already-extracted cached cell strings into static cells.
/// Formula sources, chart definitions, formatting, and cache freshness are not inferred.
pub fn table_to_workbook(name: &str, rows: &[Vec<String>]) -> Result<Workbook, String> {
    if name.trim().is_empty() || name.len() > 100 {
        return Err("sheet name must be 1 to 100 bytes".into());
    }
    if rows.is_empty() {
        return Err("donor table is empty".into());
    }
    let width = rows.iter().map(Vec::len).max().unwrap_or(0);
    if width == 0 || width > MAX_COLS as usize || rows.len().saturating_mul(width) > MAX_CELLS {
        return Err("donor table exceeds the supported static grid".into());
    }
    let mut book = Workbook::new(name);
    let sheet = book.active_sheet();
    for (row, values) in rows.iter().enumerate() {
        for (col, value) in values.iter().enumerate() {
            if value.is_empty() {
                continue;
            }
            let input = if !value.starts_with('=') {
                match value.parse::<f64>() {
                    Ok(number) if number.is_finite() => CellInput::Number(number),
                    _ => CellInput::Text(value.clone()),
                }
            } else {
                CellInput::Text(value.clone())
            };
            book.set(
                sheet,
                CellAddr {
                    row: row as u32 + 1,
                    col: col as u16,
                },
                input,
            )?;
        }
    }
    // Imported cached values are the baseline, not thousands of individual undo steps.
    book.history.clear();
    book.cursor = 0;
    Ok(book)
}

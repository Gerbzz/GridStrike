//! Loss-aware import from the bounded original `.gstrike` reader.

use std::path::Path;
use std::sync::Arc;

use gridcraft_core::{CellRef, Value};
use gridcraft_model::{Cell, Formula, Sheet, Workbook};
use legacy_gridstrike::{CellInput, CellValue};

use crate::{DocState, EngineError, Result};

pub fn convert_path(path: &Path) -> Result<Workbook> {
    // The old reader validates ZIP entries, schema, cell bounds and replayed edit history.
    let old = legacy_gridstrike::Workbook::open(path).map_err(EngineError::Other)?;
    let mut next = Workbook::new();
    next.sheets.clear();
    for source in old.sheets() {
        next.check_sheet_name(&source.name, None)
            .map_err(EngineError::Other)?;
        let mut sheet = Sheet::new(&source.name);
        for (address, input) in &source.cells {
            let at = CellRef::parse(address).ok_or_else(|| {
                EngineError::Other(format!("legacy cell {address} cannot be addressed"))
            })?;
            let cell = match input {
                CellInput::Empty => continue,
                CellInput::Text(text) => Cell::value(Value::text(text.as_str())),
                CellInput::Number(number) if number.is_finite() => {
                    Cell::value(Value::number(*number))
                }
                CellInput::Number(_) => {
                    return Err(EngineError::Other(format!(
                        "legacy cell {address} has a non-finite number"
                    )));
                }
                CellInput::Formula(text) => {
                    let body = text.strip_prefix('=').ok_or_else(|| {
                        EngineError::Other(format!("legacy formula {address} has no leading ="))
                    })?;
                    let parsed = gridcraft_formula::parse(body).map_err(|_| {
                        EngineError::Other(format!(
                            "legacy formula {address} uses syntax this engine cannot parse"
                        ))
                    })?;
                    Cell::formula(Formula {
                        text: body.to_string(),
                        expr: Some(Arc::new(parsed)),
                        array: None,
                    })
                }
            };
            sheet.cells.set(at, cell);
        }
        next.sheets.push(Arc::new(sheet));
    }
    next.active_sheet = old
        .sheets()
        .iter()
        .position(|sheet| sheet.id == old.active_sheet())
        .ok_or_else(|| EngineError::Other("legacy active sheet is missing".into()))?;

    // A formula that parses but calculates differently must not silently change the workbook.
    let checked = DocState::new(next, None, "Legacy comparison".into());
    for (index, source) in old.sheets().iter().enumerate() {
        let sheet = &checked.wb.sheets[index];
        for (address, input) in &source.cells {
            if !matches!(input, CellInput::Formula(_)) {
                continue;
            }
            let old_at = legacy_gridstrike::CellAddr::parse(address).map_err(EngineError::Other)?;
            let new_at = CellRef::parse(address)
                .ok_or_else(|| EngineError::Other(format!("invalid cell {address}")))?;
            let old_value = old.value(source.id, old_at);
            let new_value = sheet
                .cells
                .get(new_at)
                .map(|cell| &cell.value)
                .unwrap_or(&Value::Empty);
            let same = match (&old_value, new_value) {
                (CellValue::Empty, Value::Empty) => true,
                (CellValue::Text(a), Value::Text(b)) => a.as_str() == b.as_ref(),
                (CellValue::Number(a), Value::Number(b)) => {
                    (*a - *b).abs() <= 1e-10 * a.abs().max(b.abs()).max(1.0)
                }
                _ => false,
            };
            if !same {
                return Err(EngineError::Other(format!(
                    "legacy formula {address} on '{}' changes its result; import refused",
                    source.name
                )));
            }
        }
    }
    Ok((*checked.wb).clone())
}

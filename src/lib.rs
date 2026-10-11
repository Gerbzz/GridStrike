//! A local, sparse worksheet with explicit formula sources and versioned storage.

pub mod designcraft_table_adapter;
mod formula;
#[cfg(feature = "native-ui")]
pub mod native_ui;
mod store;

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const MAX_SHEETS: usize = 5;
pub const MAX_CELLS: usize = 50_000;
pub const MAX_HISTORY: usize = 10_000;
pub const MAX_ROWS: u32 = 1_048_576;
pub const MAX_COLS: u16 = 200;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CellAddr {
    pub row: u32,
    pub col: u16,
}

impl CellAddr {
    pub fn parse(source: &str) -> Result<Self, String> {
        let (row, col) = designcraft_table_adapter::donor_cell_ref(source)
            .ok_or_else(|| format!("invalid A1 cell reference: {source}"))?;
        let row = u32::try_from(row.checked_add(1).ok_or("row is too large")?)
            .map_err(|_| "row is too large")?;
        let col = u16::try_from(col).map_err(|_| "column is too large")?;
        let address = Self { row, col };
        address.validate()?;
        Ok(address)
    }

    pub fn validate(self) -> Result<(), String> {
        if self.row == 0 || self.row > MAX_ROWS || self.col >= MAX_COLS {
            return Err("cell address is outside the supported grid".into());
        }
        Ok(())
    }

    pub fn a1(self) -> String {
        let mut col = usize::from(self.col) + 1;
        let mut letters = String::new();
        while col > 0 {
            let digit = (col - 1) % 26;
            letters.insert(0, (b'A' + digit as u8) as char);
            col = (col - 1) / 26;
        }
        format!("{letters}{}", self.row)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CellInput {
    Empty,
    Text(String),
    Number(f64),
    Formula(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CellError {
    Cycle,
    DivZero,
    Ref,
    Formula,
    Num,
}

impl fmt::Display for CellError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Cycle => "#CYCLE!",
            Self::DivZero => "#DIV/0!",
            Self::Ref => "#REF!",
            Self::Formula => "#FORMULA!",
            Self::Num => "#NUM!",
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum CellValue {
    Empty,
    Text(String),
    Number(f64),
    Error(CellError),
}

impl fmt::Display for CellValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => Ok(()),
            Self::Text(text) => f.write_str(text),
            Self::Number(number) => write!(f, "{number}"),
            Self::Error(error) => error.fmt(f),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Sheet {
    pub id: Uuid,
    pub name: String,
    pub cells: BTreeMap<String, CellInput>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct HistoryEntry {
    pub sheet: Uuid,
    pub address: String,
    pub before: Option<CellInput>,
    pub after: Option<CellInput>,
}

pub struct Workbook {
    pub(crate) sheets: Vec<Sheet>,
    pub(crate) active: Uuid,
    pub(crate) history: Vec<HistoryEntry>,
    pub(crate) cursor: usize,
}

impl Workbook {
    pub fn new(name: &str) -> Self {
        let sheet = Sheet {
            id: Uuid::new_v4(),
            name: name.into(),
            cells: BTreeMap::new(),
        };
        Self {
            active: sheet.id,
            sheets: vec![sheet],
            history: Vec::new(),
            cursor: 0,
        }
    }

    pub fn active_sheet(&self) -> Uuid {
        self.active
    }

    pub fn select_sheet(&mut self, sheet: Uuid) -> Result<(), String> {
        if self.sheets.iter().any(|candidate| candidate.id == sheet) {
            self.active = sheet;
            Ok(())
        } else {
            Err("sheet does not exist".into())
        }
    }

    pub fn sheets(&self) -> &[Sheet] {
        &self.sheets
    }

    pub fn add_sheet(&mut self, name: &str) -> Result<Uuid, String> {
        if self.sheets.len() >= MAX_SHEETS {
            return Err("workbook already has five sheets".into());
        }
        if name.trim().is_empty()
            || name.len() > 100
            || self.sheets.iter().any(|sheet| sheet.name == name)
        {
            return Err("sheet needs a unique name of 1 to 100 bytes".into());
        }
        let id = Uuid::new_v4();
        self.sheets.push(Sheet {
            id,
            name: name.into(),
            cells: BTreeMap::new(),
        });
        self.active = id;
        Ok(id)
    }

    pub fn cell(&self, sheet: Uuid, address: CellAddr) -> Option<&CellInput> {
        self.sheets
            .iter()
            .find(|s| s.id == sheet)?
            .cells
            .get(&address.a1())
    }

    pub fn set(&mut self, sheet: Uuid, address: CellAddr, input: CellInput) -> Result<(), String> {
        address.validate()?;
        validate_input(&input)?;
        let new_cell = self.cell(sheet, address).is_none() && input != CellInput::Empty;
        if new_cell
            && self
                .sheets
                .iter()
                .map(|sheet| sheet.cells.len())
                .sum::<usize>()
                >= MAX_CELLS
        {
            return Err("workbook cell limit reached".into());
        }
        let target = self
            .sheets
            .iter_mut()
            .find(|s| s.id == sheet)
            .ok_or("sheet does not exist")?;
        let key = address.a1();
        let before = target.cells.get(&key).cloned();
        let after = (input != CellInput::Empty).then_some(input);
        if before == after {
            return Ok(());
        }
        if self.cursor >= MAX_HISTORY {
            return Err("workbook edit history limit reached".into());
        }
        set_raw(target, &key, after.clone());
        self.history.truncate(self.cursor);
        self.history.push(HistoryEntry {
            sheet,
            address: key,
            before,
            after,
        });
        self.cursor = self.history.len();
        Ok(())
    }

    pub fn undo(&mut self) -> Result<(), String> {
        if self.cursor == 0 {
            return Err("there is no edit to undo".into());
        }
        let entry = &self.history[self.cursor - 1];
        let sheet = self
            .sheets
            .iter_mut()
            .find(|s| s.id == entry.sheet)
            .ok_or("undo sheet is missing")?;
        set_raw(sheet, &entry.address, entry.before.clone());
        self.cursor -= 1;
        Ok(())
    }

    pub fn redo(&mut self) -> Result<(), String> {
        let entry = self
            .history
            .get(self.cursor)
            .ok_or("there is no edit to redo")?;
        let sheet = self
            .sheets
            .iter_mut()
            .find(|s| s.id == entry.sheet)
            .ok_or("redo sheet is missing")?;
        set_raw(sheet, &entry.address, entry.after.clone());
        self.cursor += 1;
        Ok(())
    }

    pub fn value(&self, sheet: Uuid, address: CellAddr) -> CellValue {
        if address.validate().is_err() || !self.sheets.iter().any(|s| s.id == sheet) {
            return CellValue::Error(CellError::Ref);
        }
        let mut evaluator = formula::Evaluator::new(self, sheet);
        evaluator.cell(address)
    }

    pub fn dependents(&self, sheet: Uuid, address: CellAddr) -> Vec<CellAddr> {
        self.sheets
            .iter()
            .find(|s| s.id == sheet)
            .into_iter()
            .flat_map(|s| &s.cells)
            .filter_map(|(key, input)| {
                let CellInput::Formula(source) = input else {
                    return None;
                };
                let expr = formula::parse(source).ok()?;
                expr.references(address)
                    .then(|| CellAddr::parse(key).ok())
                    .flatten()
            })
            .collect()
    }

    /// Export displayed values with apostrophe-guarded text for spreadsheet CSV readers.
    /// Text beginning with a formula marker, whitespace, or a control character is guarded.
    /// The native workbook preserves exact text; use `export_csv_raw` only for exact-value CSV.
    pub fn export_csv(&self, sheet: Uuid) -> Result<String, String> {
        self.export_csv_with_policy(sheet, false)
    }

    /// Export exact displayed values without guarding formula-like text. Consumers that
    /// interpret CSV as spreadsheet formulas may execute such text.
    pub fn export_csv_raw(&self, sheet: Uuid) -> Result<String, String> {
        self.export_csv_with_policy(sheet, true)
    }

    fn export_csv_with_policy(&self, sheet: Uuid, raw_text: bool) -> Result<String, String> {
        let target = self
            .sheets
            .iter()
            .find(|s| s.id == sheet)
            .ok_or("sheet does not exist")?;
        let mut last_row = 0u32;
        let mut last_col = 0u16;
        for key in target.cells.keys() {
            let address = CellAddr::parse(key)?;
            last_row = last_row.max(address.row);
            last_col = last_col.max(address.col);
        }
        if last_row == 0 {
            return Ok(String::new());
        }
        if u64::from(last_row) * (u64::from(last_col) + 1) > 100_000 {
            return Err(
                "CSV export area exceeds 100,000 cells; select a smaller used range".into(),
            );
        }
        let mut csv = String::new();
        for row in 1..=last_row {
            for col in 0..=last_col {
                if col > 0 {
                    csv.push(',');
                }
                let value = self.value(sheet, CellAddr { row, col });
                let text = match value {
                    CellValue::Text(text) if !raw_text && csv_text_needs_guard(&text) => {
                        format!("'{text}")
                    }
                    value => value.to_string(),
                };
                if text.chars().any(|ch| matches!(ch, ',' | '"' | '\r' | '\n')) {
                    csv.push('"');
                    csv.push_str(&text.replace('"', "\"\""));
                    csv.push('"');
                } else {
                    csv.push_str(&text);
                }
            }
            csv.push_str("\r\n");
        }
        Ok(csv)
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        store::save(self, path)
    }
    pub fn save_new(&self, path: &Path) -> Result<(), String> {
        store::save_new(self, path)
    }
    pub fn open(path: &Path) -> Result<Self, String> {
        store::open(path)
    }
}

fn csv_text_needs_guard(text: &str) -> bool {
    let Some(first) = text.chars().next() else {
        return false;
    };
    first.is_whitespace()
        || matches!(first, '=' | '+' | '-' | '@')
        || text.chars().any(|ch| ch.is_control() || ch == '\u{feff}')
}

fn set_raw(sheet: &mut Sheet, address: &str, input: Option<CellInput>) {
    if let Some(value) = input {
        sheet.cells.insert(address.into(), value);
    } else {
        sheet.cells.remove(address);
    }
}

pub(crate) fn validate_input(input: &CellInput) -> Result<(), String> {
    match input {
        CellInput::Empty => Ok(()),
        CellInput::Number(n) if n.is_finite() => Ok(()),
        CellInput::Number(_) => Err("numbers must be finite".into()),
        CellInput::Text(text) if text.len() <= 65_536 => Ok(()),
        CellInput::Text(_) => Err("cell text exceeds 64 KiB".into()),
        CellInput::Formula(source) if source.starts_with('=') && source.len() <= 4096 => Ok(()),
        CellInput::Formula(_) => Err("formula must start with = and fit within 4096 bytes".into()),
    }
}

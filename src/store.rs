use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::{
    CellAddr, HistoryEntry, MAX_CELLS, MAX_HISTORY, MAX_SHEETS, Sheet, Workbook, validate_input,
};

const MAX_ENTRY_BYTES: usize = 16 * 1024 * 1024;
const MAX_ARCHIVE_BYTES: u64 = 36 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
struct Manifest {
    version: u32,
    active: Uuid,
    history: Vec<HistoryEntry>,
    cursor: usize,
}

pub(crate) fn save(book: &Workbook, path: &Path) -> Result<(), String> {
    save_with_policy(book, path, true)
}

pub(crate) fn save_new(book: &Workbook, path: &Path) -> Result<(), String> {
    save_with_policy(book, path, false)
}

fn save_with_policy(book: &Workbook, path: &Path, replace: bool) -> Result<(), String> {
    validate(book)?;
    let manifest = Manifest {
        version: 1,
        active: book.active,
        history: book.history.clone(),
        cursor: book.cursor,
    };
    let json = serde_json::to_vec(&manifest).map_err(|e| e.to_string())?;
    let sheets = serde_json::to_vec(&book.sheets).map_err(|e| e.to_string())?;
    if json.len() > MAX_ENTRY_BYTES || sheets.len() > MAX_ENTRY_BYTES {
        return Err("workbook exceeds the 16 MiB entry limit".into());
    }
    let parent = path.parent().ok_or("workbook path has no parent")?;
    if !parent.is_dir() {
        return Err("workbook parent directory does not exist".into());
    }
    let temp = path.with_extension(format!("gstrike.{}.tmp", Uuid::new_v4()));
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|e| e.to_string())?;
    let staged = (|| -> Result<(), String> {
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        zip.start_file("manifest.json", options)
            .map_err(|e| e.to_string())?;
        zip.write_all(&json).map_err(|e| e.to_string())?;
        zip.start_file("sheets.json", options)
            .map_err(|e| e.to_string())?;
        zip.write_all(&sheets).map_err(|e| e.to_string())?;
        zip.finish()
            .map_err(|e| e.to_string())?
            .sync_all()
            .map_err(|e| e.to_string())?;
        Ok(())
    })();
    if let Err(error) = staged {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }
    let publish = if replace {
        fs::rename(&temp, path)
    } else {
        fs::hard_link(&temp, path)
    };
    if let Err(error) = publish {
        let _ = fs::remove_file(&temp);
        return Err(format!("could not publish staged workbook: {error}"));
    }
    if !replace {
        let _ = fs::remove_file(&temp);
    }
    Ok(())
}

pub(crate) fn open(path: &Path) -> Result<Workbook, String> {
    if fs::metadata(path).map_err(|e| e.to_string())?.len() > MAX_ARCHIVE_BYTES {
        return Err("workbook archive exceeds 36 MiB".into());
    }
    let mut zip =
        ZipArchive::new(File::open(path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    if zip.len() != 2 {
        return Err("workbook archive needs only manifest.json and sheets.json".into());
    }
    let mut manifest = Vec::new();
    let mut sheets = Vec::new();
    let mut seen_manifest = false;
    let mut seen_sheets = false;
    for i in 0..2 {
        let entry = zip.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().map_err(|e| e.to_string())?;
        let target = match name.as_ref() {
            "manifest.json" if !seen_manifest => {
                seen_manifest = true;
                &mut manifest
            }
            "sheets.json" if !seen_sheets => {
                seen_sheets = true;
                &mut sheets
            }
            _ => return Err("unsafe or duplicate workbook archive entry".into()),
        };
        if entry.size() > MAX_ENTRY_BYTES as u64 {
            return Err("workbook entry exceeds 16 MiB".into());
        }
        entry
            .take((MAX_ENTRY_BYTES + 1) as u64)
            .read_to_end(target)
            .map_err(|e| e.to_string())?;
        if target.len() > MAX_ENTRY_BYTES {
            return Err("workbook entry expanded beyond 16 MiB".into());
        }
    }
    if !seen_manifest || !seen_sheets {
        return Err("workbook archive is missing an entry".into());
    }
    let metadata: Manifest =
        serde_json::from_slice(&manifest).map_err(|e| format!("invalid manifest: {e}"))?;
    if metadata.version != 1 {
        return Err("unsupported workbook version".into());
    }
    let sheets: Vec<Sheet> =
        serde_json::from_slice(&sheets).map_err(|e| format!("invalid sheets: {e}"))?;
    let book = Workbook {
        sheets,
        active: metadata.active,
        history: metadata.history,
        cursor: metadata.cursor,
    };
    validate(&book)?;
    Ok(book)
}

fn validate(book: &Workbook) -> Result<(), String> {
    if book.sheets.is_empty()
        || book.sheets.len() > MAX_SHEETS
        || book.history.len() > MAX_HISTORY
        || book.cursor > book.history.len()
    {
        return Err("workbook sheet count or edit history is invalid".into());
    }
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    let mut count = 0usize;
    for sheet in &book.sheets {
        if !ids.insert(sheet.id)
            || !names.insert(&sheet.name)
            || sheet.name.trim().is_empty()
            || sheet.name.len() > 100
        {
            return Err("workbook has duplicate or invalid sheets".into());
        }
        count = count.saturating_add(sheet.cells.len());
        if count > MAX_CELLS {
            return Err("workbook exceeds 50,000 cells".into());
        }
        for (address, input) in &sheet.cells {
            if CellAddr::parse(address)?.a1() != *address {
                return Err("noncanonical cell address".into());
            }
            validate_input(input)?;
        }
    }
    if !ids.contains(&book.active) {
        return Err("active sheet is missing".into());
    }
    for edit in &book.history {
        if !ids.contains(&edit.sheet) || CellAddr::parse(&edit.address)?.a1() != edit.address {
            return Err("invalid history target".into());
        }
        if let Some(input) = &edit.before {
            validate_input(input)?;
            if *input == crate::CellInput::Empty {
                return Err("invalid empty history value".into());
            }
        }
        if let Some(input) = &edit.after {
            validate_input(input)?;
            if *input == crate::CellInput::Empty {
                return Err("invalid empty history value".into());
            }
        }
    }
    let current: BTreeMap<(Uuid, String), crate::CellInput> = book
        .sheets
        .iter()
        .flat_map(|sheet| {
            sheet
                .cells
                .iter()
                .map(move |(address, value)| ((sheet.id, address.clone()), value.clone()))
        })
        .collect();
    let mut prior = current.clone();
    for edit in book.history[..book.cursor].iter().rev() {
        replay_edit(&mut prior, edit, &edit.after, &edit.before)?;
    }
    let mut redo = current;
    for edit in &book.history[book.cursor..] {
        replay_edit(&mut redo, edit, &edit.before, &edit.after)?;
    }
    Ok(())
}

fn replay_edit(
    cells: &mut BTreeMap<(Uuid, String), crate::CellInput>,
    edit: &HistoryEntry,
    expected: &Option<crate::CellInput>,
    replacement: &Option<crate::CellInput>,
) -> Result<(), String> {
    let key = (edit.sheet, edit.address.clone());
    if cells.get(&key).cloned() != *expected {
        return Err("workbook history does not match current cells; source was not changed".into());
    }
    if let Some(value) = replacement {
        cells.insert(key, value.clone());
    } else {
        cells.remove(&key);
    }
    Ok(())
}

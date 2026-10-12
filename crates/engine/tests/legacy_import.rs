use gridcraft_core::{CellRef, Value};
use gridcraft_engine::Session;
use legacy_gridstrike::{CellAddr, CellInput, Workbook};
use serde_json::json;

fn scratch(name: &str) -> std::path::PathBuf {
    let id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "gridstrike-legacy-{name}-{}-{id}",
        std::process::id()
    ))
}

#[test]
fn two_sheet_formulas_import_edit_and_save_without_touching_source() {
    let source = scratch("source").with_extension("gstrike");
    let target = scratch("converted").with_extension("xlsx");
    let mut old = Workbook::new("Inputs");
    let first = old.active_sheet();
    old.set(
        first,
        CellAddr::parse("A1").unwrap(),
        CellInput::Number(2.0),
    )
    .unwrap();
    old.set(
        first,
        CellAddr::parse("B1").unwrap(),
        CellInput::Number(3.0),
    )
    .unwrap();
    old.set(
        first,
        CellAddr::parse("C1").unwrap(),
        CellInput::Formula("=A1*B1".into()),
    )
    .unwrap();
    old.set(
        first,
        CellAddr::parse("D1").unwrap(),
        CellInput::Text("@SUM(1,1)".into()),
    )
    .unwrap();
    let second = old.add_sheet("Notes").unwrap();
    old.set(
        second,
        CellAddr::parse("A1").unwrap(),
        CellInput::Text("Reviewed".into()),
    )
    .unwrap();
    old.select_sheet(first).unwrap();
    old.save(&source).unwrap();
    let original = std::fs::read(&source).unwrap();

    let mut session = Session::new();
    let report = session
        .execute("file.importLegacy", json!({"path": source}))
        .unwrap();
    assert_eq!(report["saveAsRequired"], true);
    assert_eq!(report["historyReset"], true);
    let doc = session.doc().unwrap();
    assert!(doc.path.is_none());
    assert_eq!(
        doc.wb
            .sheets
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>(),
        ["Inputs", "Notes"]
    );
    assert_eq!(doc.wb.active_sheet, 0);
    let sheet = doc.wb.active().unwrap();
    assert_eq!(
        sheet
            .cells
            .get(CellRef::parse("C1").unwrap())
            .unwrap()
            .formula
            .as_ref()
            .unwrap()
            .text,
        "A1*B1"
    );
    assert_eq!(
        sheet
            .cells
            .get(CellRef::parse("C1").unwrap())
            .unwrap()
            .value,
        Value::number(6.0)
    );
    assert_eq!(
        sheet
            .cells
            .get(CellRef::parse("D1").unwrap())
            .unwrap()
            .value,
        Value::text("@SUM(1,1)")
    );
    session
        .execute("cell.set", json!({"cell": "A1", "input": "4"}))
        .unwrap();
    session.execute("edit.undo", json!({})).unwrap();
    session.execute("edit.redo", json!({})).unwrap();
    session
        .execute("file.saveAs", json!({"path": target}))
        .unwrap();
    let mut reopened = Session::new();
    reopened
        .execute("file.open", json!({"path": target}))
        .unwrap();
    assert_eq!(reopened.doc().unwrap().wb.sheets.len(), 2);
    assert_eq!(std::fs::read(&source).unwrap(), original);
    let _ = std::fs::remove_file(source);
    let _ = std::fs::remove_file(target);
}

#[test]
fn malformed_archive_keeps_active_workbook_and_source_bytes() {
    let source = scratch("malformed").with_extension("gstrike");
    std::fs::write(&source, b"not a workbook").unwrap();
    let before = std::fs::read(&source).unwrap();
    let mut session = Session::new();
    session.execute("file.new", json!({})).unwrap();
    let count = session.documents().len();
    assert!(
        session
            .execute("file.importLegacy", json!({"path": source}))
            .is_err()
    );
    assert_eq!(session.documents().len(), count);
    assert_eq!(std::fs::read(&source).unwrap(), before);
    let _ = std::fs::remove_file(source);
}

//! Self-authored engine acceptance. Windows pointer/keyboard input remains a separate gate.

use gridcraft_core::{CellRef, Value};
use gridcraft_engine::{DocState, Session, io};
use serde_json::json;

fn value(session: &Session, addr: &str) -> Value {
    session.doc().unwrap().wb.active().unwrap().value(CellRef::parse(addr).unwrap())
}

#[test]
fn editable_workbook_recalculates_and_exports_guarded_csv() {
    let out = std::env::var_os("STRIKE_GRID_NATIVE_ACCEPTANCE_DIR").expect("set STRIKE_GRID_NATIVE_ACCEPTANCE_DIR to a scratch output directory");
    let out = std::path::PathBuf::from(out);
    std::fs::create_dir_all(&out).unwrap();
    let xlsx = out.join("self-authored-budget.xlsx");
    let csv = out.join("self-authored-budget.csv");

    let mut session = Session::new();
    session.new_workbook();
    for (cell, input) in [("A1", "Quantity"), ("B1", "Price"), ("C1", "Total"), ("A2", "2"), ("B2", "9"), ("C2", "=A2*B2"), ("D2", "@SUM(1,1)"), ("E2", " =1+1"), ("F2", "-7")] {
        session.execute("cell.set", json!({"cell": cell, "input": input})).unwrap();
    }
    assert_eq!(value(&session, "C2"), Value::number(18.0));
    assert_eq!(value(&session, "D2"), Value::text("@SUM(1,1)"));
    assert_eq!(value(&session, "E2"), Value::text(" =1+1"));
    assert_eq!(value(&session, "F2"), Value::number(-7.0));
    let first_bytes = io::save_bytes(&session.doc().unwrap().wb, "self-authored-budget.xlsx").unwrap();
    io::write_file(xlsx.to_str().unwrap(), &first_bytes).unwrap();

    let (reopened, _) = io::open_bytes("self-authored-budget.xlsx", &std::fs::read(&xlsx).unwrap()).unwrap();
    let mut session = Session::new();
    session.add_document(DocState::new(reopened, None, "Budget".into()));
    assert_eq!(value(&session, "C2"), Value::number(18.0));
    assert_eq!(session.doc().unwrap().wb.active().unwrap().cell(CellRef::parse("C2").unwrap()).unwrap().formula.as_ref().unwrap().text, "A2*B2");
    session.execute("cell.set", json!({"cell": "A2", "input": "3"})).unwrap();
    assert_eq!(value(&session, "C2"), Value::number(27.0));
    session.execute("edit.undo", json!({})).unwrap();
    assert_eq!(value(&session, "C2"), Value::number(18.0));
    session.execute("edit.redo", json!({})).unwrap();
    assert_eq!(value(&session, "C2"), Value::number(27.0));

    io::write_file(xlsx.to_str().unwrap(), &io::save_bytes(&session.doc().unwrap().wb, "self-authored-budget.xlsx").unwrap()).unwrap();
    let csv_bytes = io::save_bytes(&session.doc().unwrap().wb, "self-authored-budget.csv").unwrap();
    let csv_text = String::from_utf8(csv_bytes.clone()).unwrap();
    assert!(csv_text.contains("'@SUM(1,1)"), "formula-like text must be guarded in default CSV: {csv_text:?}");
    assert!(csv_text.contains("' =1+1"), "leading whitespace must be guarded in default CSV: {csv_text:?}");
    assert!(csv_text.contains(",-7\r\n"), "numeric negatives must remain numeric: {csv_text:?}");
    assert!(csv_text.contains("27"));
    io::write_file(csv.to_str().unwrap(), &csv_bytes).unwrap();
}

use std::fs;

use gridstrike::{CellAddr, CellInput, CellValue, Workbook};

fn at(address: &str) -> CellAddr {
    CellAddr::parse(address).unwrap()
}

#[test]
fn monthly_sales_recalculates_and_reopens_formula_sources() {
    let mut book = Workbook::new("Sales");
    let sheet = book.active_sheet();
    for (address, value) in [
        ("A1", CellInput::Text("Month".into())),
        ("B2", CellInput::Number(10.0)),
        ("C2", CellInput::Number(2.5)),
        ("B3", CellInput::Number(12.0)),
        ("C3", CellInput::Number(3.0)),
        ("B4", CellInput::Number(8.0)),
        ("C4", CellInput::Number(4.0)),
        ("D2", CellInput::Formula("=B2*C2".into())),
        ("D3", CellInput::Formula("=B3*C3".into())),
        ("D4", CellInput::Formula("=B4*C4".into())),
        ("D5", CellInput::Formula("=SUM(D2:D4)".into())),
    ] {
        book.set(sheet, at(address), value).unwrap();
    }
    assert_eq!(book.value(sheet, at("D2")), CellValue::Number(25.0));
    assert_eq!(book.value(sheet, at("D3")), CellValue::Number(36.0));
    assert_eq!(book.value(sheet, at("D4")), CellValue::Number(32.0));
    assert_eq!(book.value(sheet, at("D5")), CellValue::Number(93.0));
    assert!(book.dependents(sheet, at("B2")).contains(&at("D2")));
    let root = std::env::temp_dir().join(format!("gridstrike-sales-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let path = root.join("sales.gstrike");
    book.save(&path).unwrap();
    let reopened = Workbook::open(&path).unwrap();
    assert_eq!(
        reopened.cell(sheet, at("D5")),
        Some(&CellInput::Formula("=SUM(D2:D4)".into()))
    );
    assert_eq!(reopened.value(sheet, at("D5")), CellValue::Number(93.0));
    assert!(
        reopened
            .export_csv(sheet)
            .unwrap()
            .ends_with("\r\n,,,93\r\n")
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn errors_and_undo_are_explicit() {
    let mut book = Workbook::new("Errors");
    let sheet = book.active_sheet();
    book.set(sheet, at("A1"), CellInput::Formula("=B1".into()))
        .unwrap();
    book.set(sheet, at("B1"), CellInput::Formula("=A1".into()))
        .unwrap();
    book.set(sheet, at("C1"), CellInput::Formula("=1/0".into()))
        .unwrap();
    book.set(sheet, at("D1"), CellInput::Formula("=SUM(Z999999)".into()))
        .unwrap();
    book.set(sheet, at("E1"), CellInput::Formula("=SUM(".into()))
        .unwrap();
    assert_eq!(book.value(sheet, at("A1")).to_string(), "#CYCLE!");
    assert_eq!(book.value(sheet, at("B1")).to_string(), "#CYCLE!");
    assert_eq!(book.value(sheet, at("C1")).to_string(), "#DIV/0!");
    assert_eq!(book.value(sheet, at("D1")), CellValue::Number(0.0));
    assert_eq!(book.value(sheet, at("E1")).to_string(), "#FORMULA!");
    book.undo().unwrap();
    assert_eq!(book.value(sheet, at("E1")), CellValue::Empty);
    book.redo().unwrap();
    assert_eq!(book.value(sheet, at("E1")).to_string(), "#FORMULA!");
    assert!(
        book.set(sheet, at("F1"), CellInput::Number(f64::INFINITY))
            .is_err()
    );
}

#[test]
fn donor_cached_strings_remain_static_values() {
    let rows = vec![
        vec!["Month".into(), "Value".into()],
        vec!["Jan".into(), "93".into()],
        vec!["Feb".into(), "=SUM(B2:B3)".into()],
    ];
    let book = gridstrike::designcraft_table_adapter::table_to_workbook("Cached", &rows).unwrap();
    let sheet = book.active_sheet();
    assert_eq!(book.cell(sheet, at("B2")), Some(&CellInput::Number(93.0)));
    assert_eq!(
        book.cell(sheet, at("B3")),
        Some(&CellInput::Text("=SUM(B2:B3)".into()))
    );
    assert_eq!(
        book.value(sheet, at("B3")),
        CellValue::Text("=SUM(B2:B3)".into())
    );
}

#[test]
fn malformed_archive_is_rejected_without_mutating_a_workbook() {
    let root = std::env::temp_dir().join(format!("gridstrike-invalid-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let path = root.join("broken.gstrike");
    fs::write(&path, b"not a ZIP archive").unwrap();
    assert!(Workbook::open(&path).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"not a ZIP archive");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn new_workbook_never_replaces_an_existing_file() {
    let root = std::env::temp_dir().join(format!("gridstrike-no-clobber-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let path = root.join("existing.gstrike");
    fs::write(&path, b"KEEP ORIGINAL").unwrap();
    assert!(Workbook::new("Sales").save_new(&path).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"KEEP ORIGINAL");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn csv_defaults_to_guarded_text_and_raw_mode_is_explicit() {
    let mut book = Workbook::new("Export");
    let sheet = book.active_sheet();
    for (address, text) in [
        ("A1", "=1+1"),
        ("B1", "+SUM(1,2)"),
        ("C1", "-2"),
        ("D1", "@A1"),
        ("E1", "\t=1+1"),
        ("F1", "\u{0001}danger"),
        ("G1", "  =2+2"),
    ] {
        book.set(sheet, at(address), CellInput::Text(text.into()))
            .unwrap();
    }
    book.set(sheet, at("H1"), CellInput::Number(-2.0)).unwrap();
    book.set(sheet, at("I1"), CellInput::Formula("=1+1".into()))
        .unwrap();
    book.set(sheet, at("J1"), CellInput::Text("line\n=2+2".into()))
        .unwrap();
    let safe = book.export_csv(sheet).unwrap();
    assert!(safe.starts_with("'=1+1,\"'+SUM(1,2)\",'-2,'@A1,'\t=1+1,'\u{0001}danger,'  =2+2,-2,2"));
    let raw = book.export_csv_raw(sheet).unwrap();
    assert!(safe.contains("\"'line\n=2+2\""));
    assert!(raw.contains("\"line\n=2+2\""));
    assert!(raw.starts_with("=1+1,\"+SUM(1,2)\",-2,@A1,\t=1+1,\u{0001}danger,  =2+2,-2,2"));
}

#[test]
fn valid_shape_but_false_history_is_rejected_without_writing_source() {
    use std::io::Write;

    use serde_json::json;
    use uuid::Uuid;
    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

    let root = std::env::temp_dir().join(format!("gridstrike-false-history-{}", Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    let path = root.join("contradictory.gstrike");
    let id = Uuid::new_v4();
    let manifest = json!({"version":1,"active":id,"cursor":1,"history":[
        {"sheet":id,"address":"A1","before":{"Number":777.0},"after":{"Number":42.0}}
    ]});
    let sheets = json!([{"id":id,"name":"Sheet","cells":{"A1":{"Number":2.0}}}]);
    let file = fs::File::create(&path).unwrap();
    let mut zip = ZipWriter::new(file);
    zip.start_file("manifest.json", SimpleFileOptions::default())
        .unwrap();
    zip.write_all(manifest.to_string().as_bytes()).unwrap();
    zip.start_file("sheets.json", SimpleFileOptions::default())
        .unwrap();
    zip.write_all(sheets.to_string().as_bytes()).unwrap();
    zip.finish().unwrap();
    let before = fs::read(&path).unwrap();
    let error = Workbook::open(&path)
        .err()
        .expect("contradictory history must fail");
    assert!(error.contains("history"), "{error}");
    assert_eq!(fs::read(&path).unwrap(), before);
    fs::remove_dir_all(root).unwrap();
}

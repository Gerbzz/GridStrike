#![cfg(feature = "native-ui")]

use std::fs;

use gridstrike::native_ui::NativeEditor;
use uuid::Uuid;

#[test]
fn native_actions_edit_formula_save_reopen_undo_and_export() {
    let root = std::env::temp_dir().join(format!("gridstrike-native-{}", Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    let path = root.join("sales.gstrike");
    let mut editor = NativeEditor::open_or_create(path.clone()).unwrap();
    for (address, source) in [
        ("B2", "10"),
        ("C2", "2.5"),
        ("B3", "12"),
        ("C3", "3"),
        ("B4", "8"),
        ("C4", "4"),
        ("D2", "=B2*C2"),
        ("D3", "=B3*C3"),
        ("D4", "=B4*C4"),
        ("D5", "=SUM(D2:D4)"),
    ] {
        editor.select_address(address).unwrap();
        editor.set_draft(source);
        editor.commit_edit().unwrap();
    }
    assert_eq!(editor.display_value("D5").unwrap(), "93");
    editor.save().unwrap();
    let mut reopened = NativeEditor::open_or_create(path.clone()).unwrap();
    assert_eq!(reopened.display_value("D5").unwrap(), "93");
    reopened.undo().unwrap();
    assert_eq!(reopened.display_value("D5").unwrap(), "");
    reopened.redo().unwrap();
    assert_eq!(reopened.display_value("D5").unwrap(), "93");
    reopened.select_address("A1").unwrap();
    reopened.set_draft("=1+1 as text");
    reopened.set_text_literal(true);
    reopened.commit_edit().unwrap();
    let csv = root.join("safe.csv");
    reopened.export_csv(&csv).unwrap();
    assert!(
        fs::read_to_string(&csv)
            .unwrap()
            .starts_with("'=1+1 as text,")
    );
    assert!(reopened.export_csv(&csv).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn native_actions_reject_bad_address_and_nonfinite_number_without_mutation() {
    let root = std::env::temp_dir().join(format!("gridstrike-native-bad-{}", Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    let mut editor = NativeEditor::open_or_create(root.join("book.gstrike")).unwrap();
    assert!(editor.select_address("A0").is_err());
    editor.select_address("A1").unwrap();
    editor.set_draft("1e999");
    assert!(editor.commit_edit().is_err());
    assert_eq!(editor.display_value("A1").unwrap(), "");
    fs::remove_dir_all(root).unwrap();
}

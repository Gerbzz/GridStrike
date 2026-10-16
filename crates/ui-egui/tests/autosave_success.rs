//! AutoSave accepts the same uppercase XLSX extension as the engine and writes a real file.
use gridcraft_engine::{
    Session,
    core::{CellRef, Value},
};
use gridcraft_ui_egui::{Services, SheetApp};
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "gridstrike-autosave-success-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn uppercase_xlsx_autosave_writes_complete_workbook() {
    let scratch = Scratch::new();
    let target = scratch.0.join("Budget.XLSX");
    let mut session = Session::new();
    session.new_workbook();
    session
        .run("cell.set", json!({"cell": "A1", "input": "42"}))
        .unwrap();
    session.active_mut().unwrap().path = Some(target.to_string_lossy().into_owned());
    let mut h = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1200.0, 800.0))
        .build_ui_state(
            |ui, app: &mut SheetApp| {
                let ctx = ui.ctx().clone();
                app.logic(&ctx);
                app.ui(ui);
            },
            SheetApp::new(session, Services::default()),
        );
    h.state_mut().grid.autosave = true;
    h.state_mut().grid.last_autosave = f64::NEG_INFINITY;
    h.run_steps(6);

    assert!(
        target.is_file(),
        "AutoSave must write the uppercase XLSX path"
    );
    assert!(!h.state().session.active().unwrap().is_dirty());
    assert!(h.state().message.is_none());
    let bytes = fs::read(&target).unwrap();
    let (book, _) = gridcraft_engine::io::open_bytes("Budget.XLSX", &bytes).unwrap();
    assert_eq!(
        book.active().unwrap().value(CellRef::parse("A1").unwrap()),
        Value::Number(42.0)
    );
    assert_eq!(
        fs::read_dir(&scratch.0).unwrap().count(),
        1,
        "owned staging files must be cleaned up"
    );
}

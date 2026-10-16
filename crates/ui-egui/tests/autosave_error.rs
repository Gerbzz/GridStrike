//! A failed automatic save must be visible and leave the edited workbook unsaved.
use gridcraft_engine::Session;
use gridcraft_ui_egui::{Services, SheetApp};
use serde_json::json;

#[test]
fn autosave_failure_is_shown_and_pauses_retries() {
    let dir =
        std::env::temp_dir().join(format!("gridstrike-autosave-error-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let target = dir.join("blocked.xlsx");
    std::fs::create_dir(&target).unwrap();
    std::fs::write(target.join("keep.txt"), b"preserve me").unwrap();

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

    let (title, message) = h
        .state()
        .message
        .as_ref()
        .expect("autosave error must be shown");
    assert_eq!(title, "AutoSave failed");
    assert!(message.contains("Automatic saving is paused"), "{message}");
    assert!(message.contains("blocked.xlsx"), "{message}");
    assert!(
        !h.state().grid.autosave,
        "do not reopen the error on every frame"
    );
    assert!(h.state().session.active().unwrap().is_dirty());
    assert_eq!(
        std::fs::read(target.join("keep.txt")).unwrap(),
        b"preserve me"
    );
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}

use std::fs;
use std::process::Command;

#[test]
fn separate_processes_edit_calculate_undo_and_export() {
    let root = std::env::temp_dir().join(format!("gridstrike-cli-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let path = root.join("sales.gstrike");
    let file = path.to_str().unwrap();
    let bin = env!("CARGO_BIN_EXE_gridstrike");
    let run = |args: &[&str]| -> String {
        let output = Command::new(bin).args(args).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };
    run(&["new", file, "Sales"]);
    run(&["set", file, "B2", "number", "10"]);
    run(&["set", file, "C2", "number", "2.5"]);
    run(&["set", file, "D2", "formula", "=B2*C2"]);
    assert!(run(&["get", file, "D2"]).contains("25"));
    assert!(run(&["csv", file]).contains("10,2.5,25"));
    run(&["undo", file]);
    assert_eq!(run(&["get", file, "D2"]).trim(), "D2");
    run(&["redo", file]);
    assert!(run(&["get", file, "D2"]).contains("25"));
    run(&["set", file, "A1", "text", "=1+1"]);
    assert!(run(&["csv", file]).starts_with("'=1+1,"));
    assert!(run(&["csv-raw", file]).starts_with("=1+1,"));
    fs::remove_dir_all(root).unwrap();
}

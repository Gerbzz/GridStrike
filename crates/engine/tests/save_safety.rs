//! Exercise the production file writer with synthetic files in an owned scratch directory.
use gridcraft_engine::io;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "gridstrike-save-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn target(&self) -> PathBuf {
        self.0.join("budget.xlsx")
    }

    fn assert_no_owned_temps(&self) {
        for entry in fs::read_dir(&self.0).unwrap() {
            let name = entry.unwrap().file_name();
            assert!(
                !name.to_string_lossy().starts_with(".gridstrike-save-"),
                "owned staging file leaked: {name:?}"
            );
        }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn existing_sidecar_is_never_claimed_or_changed() {
    let d = Scratch::new();
    let target = d.target();
    let sidecar = d.0.join("budget.xlsx.sctmp");
    fs::write(&target, b"old workbook").unwrap();
    fs::write(&sidecar, b"unrelated sidecar").unwrap();

    io::write_file(target.to_str().unwrap(), b"new workbook").unwrap();
    assert_eq!(fs::read(&target).unwrap(), b"new workbook");
    assert_eq!(fs::read(&sidecar).unwrap(), b"unrelated sidecar");
    d.assert_no_owned_temps();
}

#[test]
fn concurrent_production_writes_commit_only_complete_workbooks() {
    let d = Scratch::new();
    let target = d.target();
    fs::write(&target, b"old workbook").unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
    let saves: Vec<_> = [b'A', b'B']
        .into_iter()
        .map(|byte| {
            let barrier = barrier.clone();
            let target = target.clone();
            std::thread::spawn(move || {
                let bytes = vec![byte; 1024 * 1024];
                barrier.wait();
                io::write_file(target.to_str().unwrap(), &bytes)
            })
        })
        .collect();
    barrier.wait();
    for save in saves {
        save.join().unwrap().unwrap();
    }
    let actual = fs::read(&target).unwrap();
    assert!(actual == vec![b'A'; 1024 * 1024] || actual == vec![b'B'; 1024 * 1024]);
    d.assert_no_owned_temps();
}

#[test]
fn failed_replacement_preserves_existing_destination_and_cleans_staging() {
    let d = Scratch::new();
    let target = d.target();
    fs::create_dir(&target).unwrap();
    let sentinel = target.join("keep.txt");
    fs::write(&sentinel, b"preserve me").unwrap();

    assert!(io::write_file(target.to_str().unwrap(), b"new workbook").is_err());
    assert_eq!(fs::read(&sentinel).unwrap(), b"preserve me");
    d.assert_no_owned_temps();
}


//! Save through a sibling file owned by one operation. The destination changes only after the
//! complete replacement is prepared. Adapted from WordStrike's MIT OR Apache-2.0 save helper.
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub(super) fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let permissions = match fs::metadata(path) {
        Ok(meta) if meta.is_file() => Some(meta.permissions()),
        Ok(_) => None,
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => return Err(e),
    };
    // Existing workbooks keep their permissions; a new file gets the platform's default.
    let private = permissions.is_some();
    replace_with(path, private, |file| {
        file.write_all(bytes)?;
        if let Some(permissions) = permissions {
            file.set_permissions(permissions)?;
        }
        file.sync_all()
    })
}

fn replace_with(
    path: &Path,
    private: bool,
    prepare: impl FnOnce(&mut File) -> io::Result<()>,
) -> io::Result<()> {
    let mut temp = Temporary::create(path, private)?;
    prepare(temp.file()?)?;
    temp.commit(path)
}

struct Temporary {
    path: PathBuf,
    file: Option<File>,
    committed: bool,
}

impl Temporary {
    fn create(destination: &Path, private: bool) -> io::Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        if destination.file_name().is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "save destination needs a file name",
            ));
        }
        // A short sibling name works even when the destination is at the filename limit.
        for _ in 0..128 {
            let id = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = destination
                .with_file_name(format!(".gridstrike-save-{}-{id}.tmp", std::process::id()));
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            if private {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            #[cfg(not(unix))]
            let _ = private;
            match options.open(&path) {
                Ok(file) => {
                    return Ok(Self {
                        path,
                        file: Some(file),
                        committed: false,
                    });
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not create a save temporary file",
        ))
    }

    fn file(&mut self) -> io::Result<&mut File> {
        self.file
            .as_mut()
            .ok_or_else(|| io::Error::other("save temporary file is closed"))
    }

    fn commit(mut self, destination: &Path) -> io::Result<()> {
        // Windows cannot rename an open staging file.
        drop(self.file.take());
        fs::rename(&self.path, destination)?;
        self.committed = true;
        Ok(())
    }
}

impl Drop for Temporary {
    fn drop(&mut self) {
        drop(self.file.take());
        if !self.committed {
            // Only remove the temporary path successfully created by this operation.
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "gridstrike-save-unit-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn target(&self) -> PathBuf {
            self.0.join("budget.xlsx")
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn failed_preparation_preserves_existing_workbook_and_cleans_owned_temp() {
        let d = Scratch::new();
        let path = d.target();
        fs::write(&path, b"original workbook").unwrap();
        let err = replace_with(&path, true, |file| {
            file.write_all(b"partial replacement")?;
            Err(io::Error::other("injected failure"))
        })
        .unwrap_err();
        assert!(err.to_string().contains("injected failure"));
        assert_eq!(fs::read(&path).unwrap(), b"original workbook");
        assert_eq!(fs::read_dir(&d.0).unwrap().count(), 1);
    }

    #[test]
    fn overlapping_saves_have_separate_staging_files_and_complete_commits() {
        let d = Scratch::new();
        let path = d.target();
        fs::write(&path, b"original workbook").unwrap();
        let mut first = Temporary::create(&path, true).unwrap();
        let mut second = Temporary::create(&path, true).unwrap();
        assert_ne!(first.path, second.path);
        first
            .file()
            .unwrap()
            .write_all(b"first complete workbook")
            .unwrap();
        second
            .file()
            .unwrap()
            .write_all(b"second complete workbook")
            .unwrap();
        first.file().unwrap().sync_all().unwrap();
        second.file().unwrap().sync_all().unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"original workbook");
        first.commit(&path).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"first complete workbook");
        second.commit(&path).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"second complete workbook");
        assert_eq!(fs::read_dir(&d.0).unwrap().count(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn replacement_preserves_permissions_and_staging_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let d = Scratch::new();
        let path = d.target();
        fs::write(&path, b"old").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        let temp = Temporary::create(&path, true).unwrap();
        assert_eq!(
            fs::metadata(&temp.path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        drop(temp);
        write(&path, b"new").unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o640
        );
    }
}


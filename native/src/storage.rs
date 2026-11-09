use fs2::FileExt;
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

pub fn read(path: &Path) -> io::Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

fn target(path: &Path) -> io::Result<PathBuf> {
    let mut p = path.to_owned();
    for _ in 0..40 {
        match fs::read_link(&p) {
            Ok(link) => {
                p = if link.is_absolute() {
                    link
                } else {
                    p.parent().unwrap_or(Path::new(".")).join(link)
                }
            }
            Err(e)
                if e.kind() == io::ErrorKind::InvalidInput
                    || e.kind() == io::ErrorKind::NotFound =>
            {
                return Ok(p)
            }
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::other("Too many symlinks"))
}

/// Cooperating instances lock the destination; a newer external version is never silently overwritten.
pub fn save(path: &Path, text: &str, expected: &Option<String>) -> io::Result<bool> {
    let dest = target(path)?;
    let dir = dest.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(dir)?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(dest.with_extension("pin-lock"))?;
    lock.lock_exclusive()?;
    if read(path)? != *expected || target(path)? != dest {
        return Ok(false);
    }
    let mut tmp = tempfile::Builder::new().prefix(".pin-").tempfile_in(dir)?;
    if let Ok(meta) = fs::metadata(&dest) {
        tmp.as_file().set_permissions(meta.permissions())?;
    }
    tmp.write_all(text.as_bytes())?;
    tmp.as_file().sync_all()?;
    if read(path)? != *expected || target(path)? != dest {
        return Ok(false);
    }
    tmp.persist(&dest).map_err(|e| e.error)?;
    File::open(dir)?.sync_all()?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn atomic_saves_keep_symlinks_and_refuse_a_newer_disk_version() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real.md");
        let link = dir.path().join("link.md");
        std::os::unix::fs::symlink("real.md", &link).unwrap();
        assert!(save(&link, "first", &None).unwrap());
        fs::write(&real, "external").unwrap();
        assert!(!save(&link, "mine", &Some("first".into())).unwrap());
        assert_eq!(read(&real).unwrap().as_deref(), Some("external"));
        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert!(save(&link, "mine", &Some("external".into())).unwrap());
    }
    #[test]
    fn concurrent_writers_cannot_both_replace_the_same_version() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("note.md");
        let workers: Vec<_> = (0..4)
            .map(|i| {
                let p = p.clone();
                std::thread::spawn(move || save(&p, &format!("{i}"), &None).unwrap())
            })
            .collect();
        assert_eq!(
            workers
                .into_iter()
                .filter_map(|t| t.join().ok())
                .filter(|v| *v)
                .count(),
            1
        );
    }
}

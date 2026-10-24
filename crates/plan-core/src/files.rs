//! Saving plans, and keeping a recovery copy next to the plan while there are unsaved edits.
//!
//! A plan `shop.jmx` has its recovery copy in `shop.jmx.bak` in the same folder. The copy is written while
//! edits are unsaved and removed when the plan is saved, so finding one means the app did not shut down
//! cleanly.

use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Where the recovery copy of `plan` lives.
pub fn recovery_path(plan: &Path) -> PathBuf {
    let mut name = plan.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(".bak");
    plan.with_file_name(name)
}

/// The largest plan or recovery file that is opened. Real plans are well under a few megabytes; the limit
/// keeps a wrong or hostile file from using all the memory.
pub const MAX_PLAN_BYTES: u64 = 256 * 1024 * 1024;

/// Read a text file, refusing one that is too large.
pub fn read_text(path: &Path) -> io::Result<String> {
    let len = std::fs::metadata(path)?.len();
    if len > MAX_PLAN_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("the file is {} MB; plans larger than {} MB are not opened", len / (1024 * 1024), MAX_PLAN_BYTES / (1024 * 1024)),
        ));
    }
    std::fs::read_to_string(path)
}

/// Write `text` so that a crash half way never leaves a half-written file: the text goes to a temporary
/// file in the same folder, which then replaces `path`. A plan that is a link is written through the link,
/// and a leftover temporary file (or a link in its place) is never followed.
pub fn write_atomic(path: &Path, text: &str) -> io::Result<()> {
    let target = if std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) { std::fs::canonicalize(path)? } else { path.to_path_buf() };
    let mut name = target.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(".tmp");
    let tmp = target.with_file_name(name);
    let _ = std::fs::remove_file(&tmp); // removes a link itself, not what it points to
    let written = std::fs::OpenOptions::new().write(true).create_new(true).open(&tmp).and_then(|mut f| {
        io::Write::write_all(&mut f, text.as_bytes())?;
        f.sync_all()
    });
    written.and_then(|()| std::fs::rename(&tmp, &target)).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// Save the plan and drop its recovery copy.
pub fn save(path: &Path, text: &str) -> io::Result<()> {
    write_atomic(path, text)?;
    discard_recovery(path);
    Ok(())
}

/// Write the recovery copy.
pub fn autosave(path: &Path, text: &str) -> io::Result<()> {
    write_atomic(&recovery_path(path), text)
}

pub fn discard_recovery(path: &Path) {
    let _ = std::fs::remove_file(recovery_path(path));
}

/// A recovery copy left behind by an earlier session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recovery {
    pub path: PathBuf,
    pub text: String,
    /// Whether the copy is newer than the plan file (or the plan file is gone).
    pub newer: bool,
}

/// The recovery copy for `plan`, if there is one that differs from the saved file.
pub fn find_recovery(plan: &Path) -> Option<Recovery> {
    let path = recovery_path(plan);
    let text = read_text(&path).ok()?;
    if read_text(plan).is_ok_and(|saved| saved == text) {
        discard_recovery(plan); // identical to the file: nothing to recover
        return None;
    }
    let modified = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH);
    let newer = !plan.exists() || modified(&path) >= modified(plan);
    Some(Recovery { path, text, newer })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("jmeter-ui-files-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn the_recovery_copy_sits_next_to_the_plan() {
        assert_eq!(recovery_path(Path::new("C:/plans/shop.jmx")), PathBuf::from("C:/plans/shop.jmx.bak"));
        assert_eq!(recovery_path(Path::new("a.jmx")), PathBuf::from("a.jmx.bak"));
    }

    #[test]
    fn save_replaces_the_file_and_removes_the_recovery_copy() {
        let d = dir("save");
        let p = d.join("shop.jmx");
        std::fs::write(&p, "old").unwrap();
        autosave(&p, "draft").unwrap();
        assert!(recovery_path(&p).exists());
        save(&p, "new").unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "new");
        assert!(!recovery_path(&p).exists());
        assert!(!d.join("shop.jmx.tmp").exists(), "no temporary file is left");
    }

    #[test]
    fn a_failed_save_leaves_the_old_file() {
        let d = dir("fail");
        let p = d.join("shop.jmx");
        std::fs::write(&p, "old").unwrap();
        // A folder in the way of the temporary file makes the write fail.
        std::fs::create_dir(d.join("shop.jmx.tmp")).unwrap();
        assert!(save(&p, "new").is_err());
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "old");
    }

    #[test]
    fn a_leftover_copy_is_found_when_it_differs() {
        let d = dir("find");
        let p = d.join("shop.jmx");
        std::fs::write(&p, "saved").unwrap();
        assert!(find_recovery(&p).is_none());
        autosave(&p, "unsaved edits").unwrap();
        let r = find_recovery(&p).unwrap();
        assert_eq!(r.text, "unsaved edits");
        assert!(r.newer);
    }

    #[test]
    fn a_copy_identical_to_the_file_is_not_a_recovery() {
        let d = dir("same");
        let p = d.join("shop.jmx");
        std::fs::write(&p, "same").unwrap();
        autosave(&p, "same").unwrap();
        assert!(find_recovery(&p).is_none());
        assert!(!recovery_path(&p).exists(), "and it is cleaned up");
    }

    #[test]
    fn a_copy_without_its_plan_is_still_offered() {
        let d = dir("orphan");
        let p = d.join("gone.jmx");
        autosave(&p, "work").unwrap();
        let r = find_recovery(&p).unwrap();
        assert!(r.newer);
    }

    #[test]
    fn an_older_copy_is_marked_as_not_newer() {
        let d = dir("older");
        let p = d.join("shop.jmx");
        autosave(&p, "old draft").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        std::fs::write(&p, "saved later by another program").unwrap();
        let r = find_recovery(&p).unwrap();
        assert!(!r.newer);
    }
}

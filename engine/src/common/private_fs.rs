//! Files the engine writes **about itself** — the audit trail, receipts, attestations, templates — are
//! created readable by their owner only (#1658).
//!
//! # The measurement
//!
//! Item 2 of the #1658 sweep was "reported, not reproduced", so it was not changed. It is reproduced now:
//! on a host with the usual `umask 022`, a file created with `std::fs::write` is **0644** and a directory
//! with `std::fs::create_dir_all` is **0755** — so every envelope under `.rigorix/audit/` was readable by
//! every account on the machine, and an envelope records the planning prompt, the file paths and the
//! findings. The agent's own writes were never in that state: `fs_at` opens them with `mode(0o600)`.
//!
//! # What this is not
//!
//! Not a permission model. It does not defend against another account running as the owner, and it does
//! not tighten files written before it existed — those are tightened the next time the store replaces
//! them, because the atomic write's temp file is what becomes the destination. A deployment that needs the
//! old files closed now must walk them itself.

use std::io::Write;
use std::path::{Path, PathBuf};

/// The mode every state file is created with.
pub const OWNER_ONLY_FILE: u32 = 0o600;

/// The mode every state directory is created with.
pub const OWNER_ONLY_DIR: u32 = 0o700;

/// Open (creating or truncating) `path` owner-only — for a caller that already owns its own
/// temp-and-rename dance and only needs the mode.
#[cfg(unix)]
pub fn create_private(path: &Path) -> std::io::Result<std::fs::File> {
    open_private(path)
}

/// See [`create_private`].
#[cfg(not(unix))]
pub fn create_private(path: &Path) -> std::io::Result<std::fs::File> {
    open_private(path)
}

#[cfg(unix)]
fn open_private(path: &Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    // sink: state:private-file
    std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        // `mode` is masked by the process umask, so 0600 is the *ceiling* — a umask can only remove
        // bits, and 0600 has none to remove from group or other.
        .mode(OWNER_ONLY_FILE)
        .open(path)
}

#[cfg(not(unix))]
fn open_private(path: &Path) -> std::io::Result<std::fs::File> {
    // No mode bits on this target; the platform's own ACL default applies.
    std::fs::File::create(path)
}

#[cfg(unix)]
fn set_dir_mode(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    // sink: state:private-file
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(OWNER_ONLY_DIR))
}

#[cfg(not(unix))]
fn set_dir_mode(_: &Path) -> std::io::Result<()> {
    Ok(())
}

/// Create `path` and its parents with [`OWNER_ONLY_DIR`], and tighten it once it exists.
///
/// `create_dir_all` uses the umask for each component it creates, and skips components that exist — so
/// the mode is set afterwards, which is what makes the answer independent of which of the two happened.
pub fn create_dir_all(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true).mode(OWNER_ONLY_DIR);
        // An existing directory is not an error: `create_dir_all`'s contract, kept.
        match builder.create(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    #[cfg(not(unix))]
    // sink: state:private-file
    std::fs::create_dir_all(path)?;
    set_dir_mode(path)
}

/// Write `bytes` to `path` through a temp file in the same directory, created owner-only.
///
/// Same-directory temp plus rename: the destination is either the old bytes or the new ones, never a
/// partial file, and the *temp* is what carries the mode into the destination.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let directory = path.parent().unwrap_or_else(|| Path::new("."));
    let temp: PathBuf = directory.join(format!(
        ".{}.tmp",
        path.file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "state".to_string())
    ));
    {
        let mut file = open_private(&temp)?;
        if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
            drop(file);
            // sink: state:private-file
            let _ = std::fs::remove_file(&temp);
            return Err(error);
        }
    }
    // sink: state:private-file
    if let Err(error) = std::fs::rename(&temp, path) {
        // sink: state:private-file
        let _ = std::fs::remove_file(&temp);
        return Err(error);
    }
    Ok(())
}

/// The mode `path` currently has, as `0oNNN` — the measurement a test asserts on, in one place.
#[cfg(unix)]
pub fn mode_of(path: &Path) -> std::io::Result<u32> {
    use std::os::unix::fs::PermissionsExt;
    Ok(std::fs::metadata(path)?.permissions().mode() & 0o777)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn a_written_file_is_owner_only_and_so_is_its_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = dir.path().join(".rigorix").join("audit");
        create_dir_all(&state).expect("the state directory");
        let path = state.join("envelope.json");
        write_atomic(&path, b"{}").expect("the write");

        assert_eq!(
            mode_of(&path).expect("mode"),
            OWNER_ONLY_FILE,
            "a state file is not readable by another account"
        );
        assert_eq!(
            mode_of(&state).expect("mode"),
            OWNER_ONLY_DIR,
            "and neither is the directory that holds it"
        );
        assert!(
            !dir.path()
                .join(".rigorix")
                .join("audit")
                .join(".envelope.json.tmp")
                .exists()
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_file_written_before_this_rule_is_tightened_when_it_is_replaced() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("envelope.json");
        std::fs::write(&path, b"old").expect("the legacy write");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).expect("chmod");

        write_atomic(&path, b"new").expect("the write");

        assert_eq!(
            std::fs::metadata(&path).expect("mode").permissions().mode() & 0o777,
            OWNER_ONLY_FILE,
            "the temp file is what becomes the destination, so the mode travels with it"
        );
        assert_eq!(std::fs::read(&path).expect("bytes"), b"new");
    }
}

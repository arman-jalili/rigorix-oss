//! `fs_at` — the one module allowed to resolve a path and then mutate it.
//!
//! @canonical .pi/architecture/modules/tool-system.md#file-write
//! Implements: SINK-2 (#1479) — validation and mutation share a handle
//! Issue: #1479
//!
//! # The defect this module removes
//!
//! `file_write_tool.rs` used to canonicalize the deepest existing ancestor, return
//! a `PathBuf`, and then call `create_dir_all` / `fs::write` / `fs::rename` on that
//! path three more times. Each of those calls re-resolved the path through the
//! filesystem, so between the check and the write a process with write access
//! inside the workspace could replace a validated component with a symlink and
//! redirect the write. That is not a proof that the check was wrong; it is a proof
//! that a check and a use in separate events are not a control
//! (`.pi/architecture/sink-register.tsv`, `tools:file-write-path`).
//!
//! # The shape that fixes it
//!
//! [`resolve_for_write`] walks each component with `openat(…, O_DIRECTORY|O_NOFOLLOW)`
//! and keeps the **opened parent directory** — a handle pins the inode, so a rename
//! or symlink swap afterwards changes what a *path* means, not what the handle means.
//! [`write_atomic`] creates the temp file in that directory (`O_CREAT|O_EXCL|O_NOFOLLOW`)
//! and `renameat`s it to the final name **within the same handle**, so the rename
//! cannot cross a directory the check never saw.
//!
//! `ValidatedWritePath`'s fields are private and it exposes no [`std::path::PathBuf`].
//! A caller cannot hold the resolved path and hand it to `fs::write`; the only way to
//! mutate is to pass the value back to [`write_atomic`] or [`append_to_file`].
//!
//! # What it does not do
//!
//! It does not survive a full disk, and it does not stop a process with write access
//! to the *parent directory* from replacing the entry after the rename. The control
//! is over redirection through path re-resolution, not over the directory's owner.

#[cfg(unix)]
mod imp {
    use std::ffi::{CString, OsStr, OsString};
    use std::io;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    use std::path::{Path, PathBuf};

    use crate::tools::domain::ToolError;

    /// A write target whose parent directory is already open.
    ///
    /// Fields are private on purpose: the value is the guard. The only operations a
    /// caller can perform on it are the ones in this module, and they all act on the
    /// pinned handle rather than re-resolving `display`.
    pub struct ValidatedWritePath {
        /// The opened parent directory. Pinned for the lifetime of this value.
        dir: std::fs::File,
        /// The final path component, relative to `dir`.
        name: OsString,
        /// The path as requested, for error messages only — never used to mutate.
        display: PathBuf,
    }

    impl std::fmt::Debug for ValidatedWritePath {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("ValidatedWritePath")
                .field("display", &self.display)
                .field("dir", &"<open handle>")
                .finish()
        }
    }

    impl ValidatedWritePath {
        /// The path as requested. Informational: it is never re-opened.
        pub fn display(&self) -> &Path {
            &self.display
        }
    }

    fn cstring(name: &OsStr) -> Result<CString, ToolError> {
        CString::new(name.as_bytes()).map_err(|_| {
            ToolError::InvalidInput(format!(
                "Path component {:?} contains a NUL byte",
                name.to_string_lossy()
            ))
        })
    }

    fn os_error(what: &str, path: &Path, error: io::Error) -> ToolError {
        ToolError::ExecutionFailed(format!("{what} '{}': {error}", path.display()))
    }

    /// Reject anything that is not a plain relative path of ordinary components.
    ///
    /// `..` is refused before any syscall: the handle walk cannot escape the root,
    /// but a caller that wrote `..` was asking to, and the refusal should say so
    /// rather than silently normalizing it away.
    fn relative_components(path_str: &str) -> Result<Vec<OsString>, ToolError> {
        let normalized = path_str.replace('\\', "/");
        let path = Path::new(&normalized);
        if path.is_absolute() {
            return Err(ToolError::PathDenied(format!(
                "Path '{}' is absolute; file operations are relative to the workspace root",
                path_str
            )));
        }
        let mut components = Vec::new();
        for component in path.components() {
            match component {
                std::path::Component::Normal(part) => components.push(part.to_os_string()),
                std::path::Component::CurDir => {}
                std::path::Component::ParentDir => {
                    return Err(ToolError::PathDenied(format!(
                        "Path '{}' contains '..' and could escape workspace root",
                        path_str
                    )));
                }
                std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                    return Err(ToolError::PathDenied(format!(
                        "Path '{}' is not a relative path",
                        path_str
                    )));
                }
            }
        }
        if components.is_empty() {
            return Err(ToolError::InvalidInput(format!(
                "Path '{}' names no file",
                path_str
            )));
        }
        Ok(components)
    }

    fn open_dir_at(dir: &std::fs::File, name: &OsStr, display: &Path) -> io::Result<std::fs::File> {
        let c_name = CString::new(name.as_bytes())
            .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
        let flags = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
        // SAFETY: `dir` is a live directory descriptor and `c_name` is a valid
        // NUL-terminated C string for the duration of the call. The returned
        // descriptor is owned immediately.
        let fd = unsafe { libc::openat(dir.as_raw_fd(), c_name.as_ptr(), flags) };
        if fd < 0 {
            let error = io::Error::last_os_error();
            // The kernel refuses a symlink opened with O_NOFOLLOW|O_DIRECTORY in one
            // of three ways depending on the platform (ELOOP on Linux, EMLINK on
            // some BSDs, ENOTDIR on macOS). All three mean the same thing here: the
            // component is a link, or is not a directory. Both are refused by name.
            let raw = error.raw_os_error();
            if raw == Some(libc::ELOOP) || raw == Some(libc::EMLINK) || raw == Some(libc::ENOTDIR) {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!(
                        "'{}' is a symlink or not a directory; file operations refuse to follow it",
                        display.display()
                    ),
                ));
            }
            return Err(error);
        }
        // SAFETY: `fd` is a freshly opened descriptor we own and never used again.
        Ok(unsafe { std::fs::File::from_raw_fd(fd) })
    }

    fn mkdir_at(dir: &std::fs::File, name: &OsStr, display: &Path) -> io::Result<()> {
        let c_name = CString::new(name.as_bytes())
            .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
        // Only the owner needs to reach it; the umask still applies.
        let mode = 0o777;
        // SAFETY: as above — live descriptor, valid C string.
        let rc = unsafe { libc::mkdirat(dir.as_raw_fd(), c_name.as_ptr(), mode) };
        if rc != 0 && io::Error::last_os_error().kind() != io::ErrorKind::AlreadyExists {
            return Err(io::Error::new(
                io::Error::last_os_error().kind(),
                format!(
                    "cannot create '{}': {}",
                    display.display(),
                    io::Error::last_os_error()
                ),
            ));
        }
        Ok(())
    }

    /// Resolve a workspace-relative path to an opened parent directory handle.
    ///
    /// Every component is opened with `O_NOFOLLOW`, so a symlinked component is
    /// refused with its name rather than followed. A missing component is created
    /// and then opened the same way — creation and traversal use the same handle
    /// chain, so a component cannot be swapped between them.
    pub fn resolve_for_write(
        workspace_root: &str,
        path_str: &str,
    ) -> Result<ValidatedWritePath, ToolError> {
        let root = Path::new(workspace_root);
        let root = root.canonicalize().map_err(|e| {
            ToolError::ExecutionFailed(format!(
                "Cannot resolve workspace root '{}': {e}",
                workspace_root
            ))
        })?;
        let components = relative_components(path_str)?;

        let mut dir = std::fs::File::open(&root)
            .map_err(|e| os_error("Cannot open workspace root", &root, e))?;

        let Some((name, parents)) = components.split_last() else {
            return Err(ToolError::InvalidInput(format!(
                "Path '{}' names no file",
                path_str
            )));
        };
        // `child_display` is built forward from the root, component by component, so
        // the message and the handle always describe the same directory.
        let mut child_display = root.clone();
        for component in parents {
            child_display.push(component);
            match open_dir_at(&dir, component, &child_display) {
                Ok(next) => dir = next,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    mkdir_at(&dir, component, &child_display)
                        .map_err(|e| os_error("Cannot create directory", &child_display, e))?;
                    dir = open_dir_at(&dir, component, &child_display).map_err(|e| {
                        if e.kind() == io::ErrorKind::PermissionDenied {
                            ToolError::PathDenied(format!(
                                "'{}' was replaced with a symlink while being created",
                                child_display.display()
                            ))
                        } else {
                            os_error("Cannot open", &child_display, e)
                        }
                    })?;
                }
                Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                    return Err(ToolError::PathDenied(format!(
                        "'{}' is a symlink or not a directory; file operations refuse to follow it",
                        child_display.display()
                    )));
                }
                Err(error) => return Err(os_error("Cannot open", &child_display, error)),
            }
        }

        Ok(ValidatedWritePath {
            dir,
            name: name.clone(),
            display: child_display.join(name),
        })
    }

    fn create_temp(target: &ValidatedWritePath) -> Result<(OsString, std::fs::File), ToolError> {
        let name = format!(".rigorix-tmp-{}", uuid::Uuid::new_v4());
        let name = OsString::from(name);
        let c_name = cstring(&name)?;
        let flags =
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC;
        // SAFETY: live directory descriptor, valid C string, owned result.
        let fd = unsafe { libc::openat(target.dir.as_raw_fd(), c_name.as_ptr(), flags, 0o600) };
        if fd < 0 {
            return Err(ToolError::ExecutionFailed(format!(
                "Failed to create a temporary file beside '{}': {}",
                target.display.display(),
                io::Error::last_os_error()
            )));
        }
        // SAFETY: `fd` is a freshly opened descriptor we own.
        Ok((name, unsafe { std::fs::File::from_raw_fd(fd) }))
    }

    fn rename_within(target: &ValidatedWritePath, from: &OsStr) -> Result<(), ToolError> {
        let c_from = cstring(from)?;
        let c_to = cstring(&target.name)?;
        // SAFETY: both names are valid C strings and both resolve relative to the
        // pinned directory descriptor — the rename cannot leave this directory.
        let rc = unsafe {
            libc::renameat(
                target.dir.as_raw_fd(),
                c_from.as_ptr(),
                target.dir.as_raw_fd(),
                c_to.as_ptr(),
            )
        };
        if rc != 0 {
            return Err(ToolError::ExecutionFailed(format!(
                "Failed to atomically write '{}': {}",
                target.display.display(),
                io::Error::last_os_error()
            )));
        }
        Ok(())
    }

    fn unlink_within(target: &ValidatedWritePath, name: &OsStr) {
        if let Ok(c_name) = CString::new(name.as_bytes()) {
            // SAFETY: live directory descriptor, valid C string. Cleanup only.
            unsafe {
                libc::unlinkat(target.dir.as_raw_fd(), c_name.as_ptr(), 0);
            }
        }
    }

    /// Write `content` to `target` atomically, inside the pinned directory.
    ///
    /// Temp file in the same directory, `fsync`, then `renameat` within the same
    /// handle: no step re-resolves a path, and the rename cannot cross a boundary
    /// the resolution never saw.
    pub fn write_atomic(target: &ValidatedWritePath, content: &[u8]) -> Result<(), ToolError> {
        use std::io::Write;

        let (temp_name, mut file) = create_temp(target)?;
        if let Err(error) = file
            .write_all(content)
            .and_then(|()| file.sync_all())
            .map_err(|e| {
                ToolError::ExecutionFailed(format!(
                    "Failed to write '{}': {e}",
                    target.display.display()
                ))
            })
        {
            drop(file);
            unlink_within(target, &temp_name);
            return Err(error);
        }
        drop(file);
        if let Err(error) = rename_within(target, &temp_name) {
            unlink_within(target, &temp_name);
            return Err(error);
        }
        Ok(())
    }

    /// Append `content` to an existing `target` through the pinned directory.
    ///
    /// `O_NOFOLLOW` means an existing symlink at the final component is refused
    /// instead of written through.
    pub fn append_to_file(target: &ValidatedWritePath, content: &[u8]) -> Result<(), ToolError> {
        use std::io::Write;

        let c_name = cstring(&target.name)?;
        let flags = libc::O_WRONLY | libc::O_APPEND | libc::O_NOFOLLOW | libc::O_CLOEXEC;
        // SAFETY: live directory descriptor, valid C string, owned result.
        let fd = unsafe { libc::openat(target.dir.as_raw_fd(), c_name.as_ptr(), flags) };
        if fd < 0 {
            let error = io::Error::last_os_error();
            return Err(if error.kind() == io::ErrorKind::NotFound {
                ToolError::ExecutionFailed(format!(
                    "Failed to append to '{}': file does not exist",
                    target.display.display()
                ))
            } else {
                ToolError::ExecutionFailed(format!(
                    "Failed to append to '{}': {error}",
                    target.display.display()
                ))
            });
        }
        // SAFETY: `fd` is a freshly opened descriptor we own.
        let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
        file.write_all(content)
            .and_then(|()| file.sync_all())
            .map_err(|e| {
                ToolError::ExecutionFailed(format!(
                    "Failed to append to '{}': {e}",
                    target.display.display()
                ))
            })
    }
}

#[cfg(not(unix))]
mod imp {
    //! Non-unix targets have no `openat`/`renameat`, so the handle-coupled
    //! guarantee cannot be made. Refusing is the honest option: a path-based
    //! fallback would reintroduce exactly the re-resolution SINK-2 removed.

    use std::path::{Path, PathBuf};

    use crate::tools::domain::ToolError;

    /// Placeholder so the API is identical on non-unix targets (it is never
    /// constructible there — resolution always fails).
    #[derive(Debug)]
    pub struct ValidatedWritePath {
        display: PathBuf,
    }

    impl ValidatedWritePath {
        /// The path as requested.
        pub fn display(&self) -> &Path {
            &self.display
        }
    }

    fn unsupported<T>() -> Result<T, ToolError> {
        Err(ToolError::ExecutionFailed(
            "file operations need openat/renameat semantics; this target has none, and a \
             path-based fallback would re-resolve between the check and the write (#1479)"
                .to_string(),
        ))
    }

    /// Always refuses on this target.
    pub fn resolve_for_write(_: &str, _: &str) -> Result<ValidatedWritePath, ToolError> {
        unsupported()
    }

    /// Always refuses on this target.
    pub fn write_atomic(_: &ValidatedWritePath, _: &[u8]) -> Result<(), ToolError> {
        unsupported()
    }

    /// Always refuses on this target.
    pub fn append_to_file(_: &ValidatedWritePath, _: &[u8]) -> Result<(), ToolError> {
        unsupported()
    }
}

pub use imp::{ValidatedWritePath, append_to_file, resolve_for_write, write_atomic};

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::tools::domain::ToolError;
    use std::os::unix::fs::symlink;
    use tempfile::TempDir;

    fn temp_root() -> TempDir {
        TempDir::new().expect("temp dir")
    }

    #[test]
    fn a_new_file_is_written_inside_the_root() {
        let root = temp_root();
        let target = resolve_for_write(root.path().to_str().unwrap(), "sub/dir/file.txt").unwrap();
        write_atomic(&target, b"hello").unwrap();
        assert_eq!(
            std::fs::read_to_string(root.path().join("sub/dir/file.txt")).unwrap(),
            "hello"
        );
    }

    /// #1479 acceptance 3: a symlinked component is refused by name, not followed.
    #[test]
    fn a_symlinked_component_is_refused() {
        let root = temp_root();
        let outside = temp_root();
        symlink(outside.path(), root.path().join("a")).unwrap();

        let error = resolve_for_write(root.path().to_str().unwrap(), "a/file.txt").unwrap_err();
        assert!(matches!(error, ToolError::PathDenied(_)), "{error:?}");
        assert!(
            !outside.path().join("file.txt").exists(),
            "nothing may be created outside the root"
        );
    }

    /// #1479 acceptance 2: swap a component between validation and mutation. The
    /// handle pins the directory, so the write lands where the check looked and the
    /// swapped-in symlink target is never touched.
    #[test]
    fn a_component_swapped_for_a_symlink_cannot_redirect_the_write() {
        let root = temp_root();
        let outside = temp_root();
        std::fs::create_dir(root.path().join("a")).unwrap();

        // Validation: `a` is a real directory inside the root.
        let target = resolve_for_write(root.path().to_str().unwrap(), "a/file.txt").unwrap();

        // Mutation-time race: `a` is moved aside and replaced by a symlink.
        std::fs::rename(root.path().join("a"), root.path().join("a-real")).unwrap();
        symlink(outside.path(), root.path().join("a")).unwrap();

        write_atomic(&target, b"hello").unwrap();

        assert!(
            !outside.path().join("file.txt").exists(),
            "the write followed the swapped symlink out of the root"
        );
        assert_eq!(
            std::fs::read_to_string(root.path().join("a-real/file.txt")).unwrap(),
            "hello",
            "the write must land in the directory the handle was opened on"
        );
    }

    /// #1479 acceptance 3: an existing symlink at the final component is refused
    /// for append rather than written through.
    #[test]
    fn append_refuses_a_symlink_target() {
        let root = temp_root();
        let outside = temp_root();
        let victim = outside.path().join("victim.txt");
        std::fs::write(&victim, "original").unwrap();
        symlink(&victim, root.path().join("link.txt")).unwrap();

        let target = resolve_for_write(root.path().to_str().unwrap(), "link.txt").unwrap();
        let error = append_to_file(&target, b" appended").unwrap_err();
        assert!(matches!(error, ToolError::ExecutionFailed(_)), "{error:?}");
        assert_eq!(
            std::fs::read_to_string(&victim).unwrap(),
            "original",
            "the symlink target must not be modified"
        );
    }

    #[test]
    fn traversal_and_absolute_paths_are_refused() {
        let root = temp_root();
        for path in ["../escape.txt", "a/../../escape.txt", "/etc/passwd", ""] {
            let error = resolve_for_write(root.path().to_str().unwrap(), path).unwrap_err();
            assert!(
                matches!(error, ToolError::PathDenied(_) | ToolError::InvalidInput(_)),
                "{path:?} must be refused, got {error:?}"
            );
        }
    }

    #[test]
    fn a_missing_intermediate_component_is_created() {
        let root = temp_root();
        let target = resolve_for_write(root.path().to_str().unwrap(), "a/b/c.txt").unwrap();
        write_atomic(&target, b"nested").unwrap();
        assert_eq!(
            std::fs::read_to_string(root.path().join("a/b/c.txt")).unwrap(),
            "nested"
        );
    }
}

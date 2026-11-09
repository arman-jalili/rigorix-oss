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
    /// A child process's working directory, resolved through an open handle.
    ///
    /// #1487's sibling (#1488): `cwd` is caller-supplied, and `cargo build` runs
    /// the `build.rs` of the directory it is started in. A `starts_with` check on a
    /// string is not a boundary — the string is resolved again by the child. This
    /// value pins the **directory inode** the check saw and applies it to the child
    /// with `fchdir` in the child's own process, so the directory the child enters
    /// is the directory that was validated, not whatever a path means at spawn time.
    pub struct ValidatedChildDir {
        /// The opened directory. Kept so `fchdir` addresses the same inode.
        dir: std::fs::File,
        /// The path as validated, for messages only — never used to `chdir`.
        display: PathBuf,
    }

    impl ValidatedChildDir {
        /// The path that was validated (for error messages and evidence).
        pub fn display(&self) -> &Path {
            &self.display
        }

        /// The validated directory's raw descriptor.
        ///
        /// The sink module applies it with `fchdir` in the child. It lives here so
        /// the module that owns the handle is the module that hands it out, and the
        /// spawn site stays the one place that names a `Command` (the sink lint).
        ///
        /// The descriptor is valid for as long as this value is alive; a caller that
        /// drops it before spawning is holding a dangling descriptor, so the value is
        /// not `Copy` and the borrow in `spawn_*` signatures keeps it alive.
        pub fn raw_fd(&self) -> std::os::fd::RawFd {
            use std::os::fd::AsRawFd;
            self.dir.as_raw_fd()
        }
    }

    /// Resolve a caller-supplied `cwd` against `workspace_root` (#1488).
    ///
    /// Every component is opened with `O_NOFOLLOW` from the opened root, so a
    /// symlinked component is refused with its name rather than followed, `..` is
    /// refused outright, and an absolute path is only accepted when it is inside
    /// the root. The returned value holds the pinned directory handle; see
    /// [`ValidatedChildDir::apply_to`].
    pub fn resolve_dir_for_child(
        workspace_root: &str,
        cwd: &str,
    ) -> Result<ValidatedChildDir, ToolError> {
        let root = Path::new(workspace_root).canonicalize().map_err(|e| {
            ToolError::ExecutionFailed(format!(
                "Cannot resolve workspace root '{}': {e}",
                workspace_root
            ))
        })?;

        let trimmed = cwd.trim();
        if trimmed.is_empty() {
            return Err(ToolError::InvalidInput("cwd is empty".to_string()));
        }
        let requested = Path::new(trimmed);

        // An absolute cwd is accepted only when it lies inside the root; strip the
        // root prefix and treat the rest as components. Anything else would have to
        // be re-resolved, which is the defect.
        let relative: PathBuf = if requested.is_absolute() {
            let canonical = requested.canonicalize().map_err(|e| {
                ToolError::PathDenied(format!(
                    "cwd '{}' cannot be resolved ({e}); cwd must be inside the workspace root '{}'",
                    cwd,
                    root.display()
                ))
            })?;
            match canonical.strip_prefix(&root) {
                Ok(rest) => rest.to_path_buf(),
                Err(_) => {
                    return Err(ToolError::PathDenied(format!(
                        "cwd '{}' is outside the workspace root '{}'; an allowlisted tool runs in                          the directory it is started in, so the directory is part of the boundary",
                        cwd,
                        root.display()
                    )));
                }
            }
        } else {
            requested.to_path_buf()
        };

        let mut dir = std::fs::File::open(&root)
            .map_err(|e| os_error("Cannot open workspace root", &root, e))?;
        let mut current_display = root.clone();
        for component in relative.components() {
            match component {
                std::path::Component::Normal(part) => {
                    current_display.push(part);
                    dir = open_dir_at(&dir, part, &current_display).map_err(|e| {
                        if e.kind() == io::ErrorKind::PermissionDenied {
                            ToolError::PathDenied(format!(
                                "cwd '{}' passes through '{}', which is a symlink or not a                                  directory; the child's working directory refuses to follow it",
                                cwd,
                                current_display.display()
                            ))
                        } else {
                            ToolError::PathDenied(format!(
                                "cwd '{}' is not a directory inside the workspace root '{}': {}",
                                cwd,
                                root.display(),
                                e
                            ))
                        }
                    })?;
                }
                std::path::Component::CurDir => {}
                std::path::Component::ParentDir => {
                    return Err(ToolError::PathDenied(format!(
                        "cwd '{}' contains '..' and could leave the workspace root '{}'",
                        cwd,
                        root.display()
                    )));
                }
                std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                    return Err(ToolError::PathDenied(format!(
                        "cwd '{}' is not inside the workspace root '{}'",
                        cwd,
                        root.display()
                    )));
                }
            }
        }

        Ok(ValidatedChildDir {
            dir,
            display: current_display,
        })
    }

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

    /// Resolve the trusted base once, creating it if configuration named a
    /// directory that does not exist yet.
    ///
    /// Creating the base is the one place a link is followed, and it is deliberate:
    /// the base came from configuration (the workspace root, the repository root),
    /// not from the caller whose path is being resolved. Everything *below* the base
    /// is walked with `O_NOFOLLOW`, which is where the caller's string lives. A base
    /// that a caller can name must not be passed here — see
    /// [`resolve_absolute_for_write`].
    fn canonical_base(base: &Path) -> Result<PathBuf, ToolError> {
        if let Ok(resolved) = base.canonicalize() {
            return Ok(resolved);
        }
        // sink: tools:fs-at-handle
        std::fs::create_dir_all(base)
            .map_err(|e| os_error("Cannot create the trusted base", base, e))?;
        base.canonicalize().map_err(|e| {
            ToolError::ExecutionFailed(format!(
                "Cannot resolve the trusted base '{}': {e}",
                base.display()
            ))
        })
    }

    /// Walk `components` from an opened `root`, creating missing directories, and
    /// pin the parent of the final name.
    ///
    /// The single walk for every resolver in this module: creation and traversal
    /// share the handle chain, so a component cannot be swapped between the
    /// `mkdirat` that creates it and the `openat` that enters it.
    fn walk_to_parent(
        root: &Path,
        components: Vec<OsString>,
        requested: &str,
    ) -> Result<ValidatedWritePath, ToolError> {
        let Some((name, parents)) = components.split_last() else {
            return Err(ToolError::InvalidInput(format!(
                "Path '{requested}' names no file"
            )));
        };
        let mut dir = std::fs::File::open(root).map_err(|e| os_error("Cannot open", root, e))?;
        // `child_display` is built forward from the root, component by component, so
        // the message and the handle always describe the same directory.
        let mut child_display = root.to_path_buf();
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
        let root = canonical_base(Path::new(workspace_root))?;
        let components = relative_components(path_str)?;
        walk_to_parent(&root, components, path_str)
    }

    /// Resolve `path` under a trusted `base`, walking every remaining component
    /// from an opened handle (#1493, SINK-9).
    ///
    /// # Why a base and not just a path
    ///
    /// `resolve_absolute_for_write` used to `create_dir_all(parent)` and then
    /// `parent.canonicalize()` — which **follows symlinks**. Judging the requested
    /// string and then resolving it through the filesystem is the SINK-2 shape one
    /// layer up: `ln -s .rigorix tmp` passes a `.rigorix` string test and every
    /// later step lands inside `.rigorix`.
    ///
    /// A base is what makes the walk possible at all. An `O_NOFOLLOW` walk from `/`
    /// is not an option: on macOS `/var → /private/var` and `/tmp → /private/tmp`,
    /// so every temporary path in the test suite would be refused. The base is the
    /// operator's configured root — it came from configuration, not from the caller
    /// — so canonicalizing *it* once is the operator's decision, and canonicalizing
    /// the caller's path is the defect. Everything below the base is walked, and a
    /// symlinked component there is refused by name.
    ///
    /// An absolute path is accepted only when it is already under the base; that is
    /// the caller's policy, stated as a refusal rather than resolved away.
    pub fn resolve_under(base: &Path, path: &Path) -> Result<ValidatedWritePath, ToolError> {
        // Both forms: the base may itself be named through an alias the operator's
        // configuration used (`/var` vs `/private/var` on macOS), and a path built by
        // joining that name must still strip.
        let given = base;
        let base = canonical_base(given)?;
        let relative = if path.is_absolute() {
            let canonical_given = given.canonicalize().unwrap_or_else(|_| base.clone());
            match path
                .strip_prefix(&base)
                .or_else(|_| path.strip_prefix(given))
                .or_else(|_| path.strip_prefix(&canonical_given))
            {
                Ok(rest) => rest.to_path_buf(),
                Err(_) => {
                    return Err(ToolError::PathDenied(format!(
                        "'{}' is outside the trusted base '{}'; an absolute write path is opened                          only when the caller's own base contains it, never resolved into place",
                        path.display(),
                        base.display()
                    )));
                }
            }
        } else {
            path.to_path_buf()
        };
        let mut components = Vec::new();
        for component in relative.components() {
            match component {
                std::path::Component::Normal(part) => components.push(part.to_os_string()),
                std::path::Component::CurDir => {}
                std::path::Component::ParentDir => {
                    return Err(ToolError::PathDenied(format!(
                        "'{}' contains '..' and could leave the trusted base '{}'",
                        path.display(),
                        base.display()
                    )));
                }
                std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                    return Err(ToolError::PathDenied(format!(
                        "'{}' is not inside the trusted base '{}'",
                        path.display(),
                        base.display()
                    )));
                }
            }
        }
        walk_to_parent(&base, components, &path.display().to_string())
    }

    /// Read the target, through the same pinned directory handle as the write.
    ///
    /// Separate `fs::read_to_string(path)` then `fs::write(path)` is the SINK-2
    /// shape one step removed: the read would resolve again. Reading by
    /// `openat(name, O_NOFOLLOW)` from the pinned parent makes the bytes an edit
    /// transforms and the file the write lands in the same inode family.
    pub fn read_file(target: &ValidatedWritePath) -> Result<Vec<u8>, ToolError> {
        let c_name = cstring(&target.name)?;
        let flags = libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
        // SAFETY: live directory descriptor, valid NUL-terminated name, owned result.
        let fd = unsafe { libc::openat(target.dir.as_raw_fd(), c_name.as_ptr(), flags) };
        if fd < 0 {
            let error = io::Error::last_os_error();
            return Err(match error.kind() {
                io::ErrorKind::NotFound => {
                    ToolError::NotFound(format!("File not found: {}", target.display.display()))
                }
                io::ErrorKind::PermissionDenied => ToolError::PathDenied(format!(
                    "'{}' is a symlink or is not readable; file operations refuse to follow it",
                    target.display.display()
                )),
                _ => os_error("Cannot read", &target.display, error),
            });
        }
        use std::io::Read;
        // SAFETY: `fd` is freshly opened, owned, and closed by `File` on drop.
        let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(|e| os_error("Cannot read", &target.display, e))?;
        Ok(bytes)
    }

    /// Resolve an engine-owned store path to a pinned parent directory (#1490).
    ///
    /// Engine-owned stores (policy config, symbol cache, hook config) choose their
    /// own path from configuration; they still benefit from the handle discipline,
    /// because "write the path, then rename the path" is the same shape wherever it
    /// appears.
    ///
    /// The trusted base is the store's own parent directory, so the walk covers one
    /// component and the file lands where configuration said. **Use
    /// [`resolve_under`] for a path a caller can influence.** The distinction is not
    /// cosmetic: `path.parent()` is derived from the same string that is about to be
    /// written, so this function cannot refuse an alias inside that parent — a
    /// caller-supplied path must be resolved against a base the caller does not
    /// choose (#1493, SINK-9).
    pub fn resolve_absolute_for_write(path: &Path) -> Result<ValidatedWritePath, ToolError> {
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .ok_or_else(|| {
                ToolError::InvalidInput(format!(
                    "'{}' has no parent directory; a relative store path has no configured base",
                    path.display()
                ))
            })?;
        resolve_under(parent, path)
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
        append_with(target, content, false)
    }

    /// Append `content`, creating the file when it does not exist (#1494).
    ///
    /// The dispatcher's `file_append` step created the file (`OpenOptions::create`),
    /// and it did so **through a path**, so a symlink leaf was followed. `O_CREAT`
    /// with `O_NOFOLLOW` keeps the creation contract and refuses the link: the kernel
    /// fails the open rather than opening the target. `FileAppendTool` keeps
    /// [`append_to_file`], whose contract is "existing file"; the two differ in one
    /// flag and are named for the difference.
    pub fn append_or_create(target: &ValidatedWritePath, content: &[u8]) -> Result<(), ToolError> {
        append_with(target, content, true)
    }

    fn append_with(
        target: &ValidatedWritePath,
        content: &[u8],
        create: bool,
    ) -> Result<(), ToolError> {
        use std::io::Write;

        let c_name = cstring(&target.name)?;
        let flags = libc::O_WRONLY
            | libc::O_APPEND
            | libc::O_NOFOLLOW
            | libc::O_CLOEXEC
            | if create { libc::O_CREAT } else { 0 };
        // SAFETY: live directory descriptor, valid C string, owned result.
        let fd = unsafe { libc::openat(target.dir.as_raw_fd(), c_name.as_ptr(), flags, 0o644) };
        if fd < 0 {
            let error = io::Error::last_os_error();
            return Err(if error.kind() == io::ErrorKind::NotFound && !create {
                ToolError::ExecutionFailed(format!(
                    "Failed to append to '{}': file does not exist",
                    target.display.display()
                ))
            } else if error.raw_os_error() == Some(libc::ELOOP)
                || error.raw_os_error() == Some(libc::EMLINK)
            {
                ToolError::PathDenied(format!(
                    "'{}' is a symlink; an append refuses to follow it",
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
    pub fn read_file(_: &ValidatedWritePath) -> Result<Vec<u8>, ToolError> {
        unsupported()
    }

    /// Always refuses on this target.
    pub fn resolve_absolute_for_write(_: &Path) -> Result<ValidatedWritePath, ToolError> {
        unsupported()
    }

    /// Always refuses on this target.
    pub fn resolve_under(_: &Path, _: &Path) -> Result<ValidatedWritePath, ToolError> {
        unsupported()
    }

    /// Always refuses on this target.
    pub fn append_to_file(_: &ValidatedWritePath, _: &[u8]) -> Result<(), ToolError> {
        unsupported()
    }

    /// Always refuses on this target.
    pub fn append_or_create(_: &ValidatedWritePath, _: &[u8]) -> Result<(), ToolError> {
        unsupported()
    }

    /// Placeholder so the API is identical on non-unix targets (never constructible).
    #[derive(Debug)]
    pub struct ValidatedChildDir {
        display: PathBuf,
    }

    impl ValidatedChildDir {
        /// The path as requested.
        pub fn display(&self) -> &Path {
            &self.display
        }

        /// Always refuses on this target.
        pub fn raw_fd(&self) -> std::os::fd::RawFd {
            -1
        }
    }

    /// Always refuses on this target: a child's working directory needs
    /// `fchdir` semantics, and a path-based fallback would re-resolve (#1488).
    pub fn resolve_dir_for_child(_: &str, _: &str) -> Result<ValidatedChildDir, ToolError> {
        unsupported()
    }
}

pub use imp::{
    ValidatedChildDir, ValidatedWritePath, append_or_create, append_to_file, read_file,
    resolve_absolute_for_write, resolve_dir_for_child, resolve_for_write, resolve_under,
    write_atomic,
};

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
        // #1494: the refusal names the link. It was `ExecutionFailed` from the raw
        // `ELOOP`, which reads as a transient I/O problem rather than "this is a
        // symlink and an append will not follow it".
        assert!(matches!(error, ToolError::PathDenied(_)), "{error:?}");
        assert!(error.to_string().contains("symlink"), "{error}");
        assert_eq!(
            std::fs::read_to_string(&victim).unwrap(),
            "original",
            "the symlink target must not be modified"
        );
    }

    /// #1494 acceptance 2: the dispatcher's `file_append` creates a missing file the
    /// way it always did, and still refuses a symlink leaf.
    #[test]
    fn append_or_create_creates_the_file_and_refuses_a_link() {
        let root = temp_root();
        let target = resolve_for_write(root.path().to_str().unwrap(), "new.txt").unwrap();
        append_or_create(&target, b"first\n").unwrap();
        assert_eq!(
            std::fs::read_to_string(root.path().join("new.txt")).unwrap(),
            "first\n"
        );

        let outside = temp_root();
        let victim = outside.path().join("victim.txt");
        std::fs::write(&victim, "ORIGINAL\n").unwrap();
        symlink(&victim, root.path().join("link.txt")).unwrap();
        let link = resolve_for_write(root.path().to_str().unwrap(), "link.txt").unwrap();
        let error = append_or_create(&link, b"INJECTED\n").unwrap_err();
        assert!(matches!(error, ToolError::PathDenied(_)), "{error:?}");
        assert_eq!(
            std::fs::read_to_string(&victim).unwrap(),
            "ORIGINAL\n",
            "the append step must not write through the link (#1494)"
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

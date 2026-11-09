//! Shell-command policy shared by every place the engine spawns a command.
//!
//! @canonical .pi/architecture/modules/tool-system.md#run-cmd
//! Implements: ISSUE-RUN-COMMAND-POLICY-SEES-WRONG-STRING (#1067) and
//!   ISSUE-COMMAND-INHERITS-ENGINE-SECRETS (#1068)
//! Issue: #1067, #1068
//!
//! Two defects lived at this seam and both were "the control sees a different
//! string than the thing it is supposed to control":
//!
//! 1. An allowlist that matched a **prefix** of a compound shell string passed
//!    `echo ok; curl evil` to `/bin/sh -c`. The control inspected a prefix; the
//!    shell executed the whole string. [`find_shell_control_operator`] refuses
//!    the compound form instead of trying to split-and-check it.
//! 2. The bash classifier was handed the raw JSON intent, so it classified a
//!    fragment of JSON rather than the command. [`command_from_intent`] is now
//!    the single extraction used by **both** the classifier and the executor,
//!    so the bytes the classifier sees are the bytes the shell receives.
//!
//! # Environment
//!
//! [`apply_child_env`] clears the inherited environment and re-adds only
//! [`INHERITED_ENV_ALLOWLIST`]. The engine signs audit envelopes with
//! `RIGORIX_HMAC_KEY`; a child that inherits it can forge the records meant to
//! constrain it. Clearing the environment removes that gift — it does **not**
//! sandbox the child, which still runs as the engine's user with the engine's
//! filesystem access. A real isolation boundary is the deployment's job.
//!
//! # The isolation fact (#1478, SINK-1)
//!
//! "A real isolation boundary is the deployment's job" used to end there, in a
//! source comment the operator never reads. [`SandboxDeclaration`] promotes that
//! sentence into a value the deployment must supply: `RIGORIX_SANDBOX` must say
//! `container` or `vm`, and `none` is refused at construction with the reach
//! named. Filtering decides which executable starts; it never decides what the
//! child can do once started, and the register records that distinction as
//! `does_not` (`.pi/architecture/sink-register.tsv`).
//!
//! # Execution grain (#1478)
//!
//! A proven-single command is executed as **argv** ([`argv_from_simple`]), not
//! handed to `sh -c`. Nothing is expanded, globbed or resolved by a shell; a
//! string that would need one is refused instead. The allowlist therefore
//! matches the *executable* ([`SINK_GRAIN`]): `cargo` with free arguments is
//! still arbitrary code execution (`build.rs`), which is why the deployment doc
//! carries the executable-by-executable table.

// #1083: approved process-spawning sink — the policy itself — `apply_child_env` takes the child `Command` by reference.
// See `.pi/architecture/modules/tool-system.md#process-spawning-sinks`.
#![allow(clippy::disallowed_types)]

use std::borrow::Cow;

/// Environment variables a child command inherits.
///
/// Kept deliberately small. `PATH`/`HOME` are needed to resolve tools and their
/// configuration; the locale and temp-dir names only affect output formatting.
/// Anything else — notably `RIGORIX_HMAC_KEY` and any anchor or cloud
/// credentials — is withheld.
pub const INHERITED_ENV_ALLOWLIST: &[&str] =
    &["PATH", "HOME", "LANG", "LC_ALL", "TMPDIR", "SHELL", "TERM"];

/// Environment keys a caller may never set on a child (#1489).
///
/// The allowlist compares a *word*; the kernel executes a *path*. `PATH` decides
/// which file a bare name resolves to, so an override of it replaces the
/// executable the policy approved. The loader variables are the same class one
/// step earlier: they run code before the program's own `main`. The runtime hooks
/// are the same class for the interpreters the allowlist is meant to admit.
///
/// Each entry carries the reason, because "refused" without "because" is the
/// sentence that gets removed again.
pub const RESERVED_ENV_KEYS: &[(&str, &str)] = &[
    (
        "PATH",
        "the program is resolved through the child's PATH, so the override decides which file \
         executes — the allowlist compared the name, not the path",
    ),
    (
        "BASH_ENV",
        "a non-interactive bash sources this file before the command it was given",
    ),
    (
        "ENV",
        "a POSIX shell sources this file before the command it was given",
    ),
    (
        "PYTHONSTARTUP",
        "the interpreter executes this file before the source it was given",
    ),
    (
        "NODE_OPTIONS",
        "the runtime applies these flags, including `--require`, before the program runs",
    ),
    (
        "PERL5OPT",
        "the interpreter applies these flags, including `-M`, before the program runs",
    ),
    (
        "RUBYOPT",
        "the interpreter applies these flags, including `-r`, before the program runs",
    ),
    (
        "GCONV_PATH",
        "the C library loads its conversion modules from this path, which is code execution",
    ),
    (
        "IFS",
        "word splitting is decided by the child, and every shell string was validated against \
         the engine's IFS",
    ),
];

/// Prefixes whose whole family runs code outside the program (#1489).
///
/// `LD_PRELOAD`/`LD_LIBRARY_PATH` (glibc), `DYLD_*` (macOS) and `LD_AUDIT` all
/// name code the loader maps before `main`. Listing one name each would be the
/// same mistake the sink register exists to prevent, so the families are matched.
pub const RESERVED_ENV_PREFIXES: &[(&str, &str)] = &[
    (
        "LD_",
        "the dynamic loader executes what this variable names before the program starts",
    ),
    (
        "DYLD_",
        "the macOS dynamic loader executes what this variable names before the program starts",
    ),
];

/// Why a caller-supplied environment entry was refused (#1489).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvOverrideError {
    /// The key is in the reserved set: setting it changes what runs, not how.
    ReservedKey {
        /// The key as supplied.
        key: String,
        /// Why it is reserved, in the register's language.
        reason: &'static str,
    },
    /// The key is not in the set the tool declared it needs.
    UndeclaredKey {
        /// The key as supplied.
        key: String,
        /// The declared set, for the message.
        declared: Vec<String>,
    },
}

impl std::fmt::Display for EnvOverrideError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EnvOverrideError::ReservedKey { key, reason } => write!(
                f,
                "environment key {key:?} is reserved and cannot be set by a caller: {reason}. \
                 The child's environment is not a parameter channel for choosing what executes."
            ),
            EnvOverrideError::UndeclaredKey { key, declared } => write!(
                f,
                "environment key {key:?} is not declared by this tool, so it would be an env \
                 entry the deployment never agreed to. Declared keys: {}.",
                if declared.is_empty() {
                    "none — this tool accepts no caller environment at all".to_string()
                } else {
                    declared.join(", ")
                }
            ),
        }
    }
}

impl std::error::Error for EnvOverrideError {}

/// Refuse a caller-supplied environment that could change *what executes* (#1489).
///
/// Two rules, in this order so the message is the specific one:
/// 1. a reserved key is refused by name, with the reason (it changes the
///    executable or the loader, not the command's inputs);
/// 2. every other key must be in `declared` — the set the tool said it needs.
///    An undeclared key is an error, not an environment entry: "anything the
///    caller sends" is what made the reserved list insufficient to begin with.
///
/// Names are compared case-insensitively, because `Path` is `PATH` to the kernel
/// on the platforms that matter.
pub fn validate_env_overrides(
    overrides: &[(String, String)],
    declared: &std::collections::HashSet<String>,
) -> Result<(), EnvOverrideError> {
    for (key, _) in overrides {
        let upper = key.to_ascii_uppercase();
        if let Some((_, reason)) = RESERVED_ENV_KEYS.iter().find(|(name, _)| *name == upper) {
            return Err(EnvOverrideError::ReservedKey {
                key: key.clone(),
                reason,
            });
        }
        if let Some((_, reason)) = RESERVED_ENV_PREFIXES
            .iter()
            .find(|(prefix, _)| upper.starts_with(prefix))
        {
            return Err(EnvOverrideError::ReservedKey {
                key: key.clone(),
                reason,
            });
        }
        if !declared.contains(key) && !declared.contains(&upper) {
            return Err(EnvOverrideError::UndeclaredKey {
                key: key.clone(),
                declared: {
                    let mut names: Vec<String> = declared.iter().cloned().collect();
                    names.sort();
                    names
                },
            });
        }
    }
    Ok(())
}

/// Resolve the executable a bare name refers to, in the **engine's** environment.
///
/// #1489: `Command::new("echo")` resolves through the child's environment, which
/// is where a caller-supplied `PATH` used to take over. Resolving here — before
/// any override is applied — makes the **path** the one the policy saw, and returns
/// it so the caller can record it: the evidence then names the binary, not the word
/// that was approved.
///
/// It does **not** pin the file (#1495). `execve` opens the returned path again, so
/// a process with write access to that directory can replace the file in between and
/// the executed bytes are not the resolved ones. The row's `does_not` says so; the
/// fix, if it is ever worth its cost, is `execveat` on a descriptor opened here.
///
/// A name containing a path separator is taken literally (and must exist); a bare
/// name is searched in the engine's `PATH` in order.
pub fn resolve_program(program: &str) -> Result<std::path::PathBuf, String> {
    use std::path::Path;
    if program.is_empty() {
        return Err("the command has no executable".to_string());
    }
    if program.contains('/') {
        let path = Path::new(program);
        return if path.is_file() {
            Ok(path.to_path_buf())
        } else {
            Err(format!("{program:?} is not an executable file"))
        };
    }
    let search = std::env::var_os("PATH").unwrap_or_default();
    for dir in std::env::split_paths(&search) {
        let candidate = dir.join(program);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(format!(
        "{program:?} was not found in the engine's PATH; the executable is resolved before any \
         caller-supplied environment is applied, so a caller cannot point it at another file"
    ))
}

/// **Global** options a `git_read` step may never use, with the reason (#1498).
///
/// A git global option comes **before** the subcommand: `git -c k=v status` sets a config
/// value, and `git status -c k=v` is not that option at all (git answers `unknown switch 'c'`).
/// Verified on 2.50.1, which is why this table is consulted in the leading position only
/// (#1502): scanning every word refused `git grep -c` and `git log -c`, where the same word is
/// the subcommand's own read flag.
///
/// Each of these makes git run a **program**, or point it at one:
///
/// * `-c` / `--config` / `--config-env` set a config value for this invocation, and
///   `-c alias.pwn=!curl …` is a shell command. Measured on `d834cc41`, with the
///   argv the dispatcher built from `args: "-c alias.pwn=!touch pwn /tmp/x"`:
///   the file was created. `!`-prefixed aliases run through a shell, and the
///   one-token restriction of `split_whitespace` is not a constraint — `!curl`
///   plus ordinary arguments is a download.
/// * `--exec-path` points git at a different binary directory.
/// * `--upload-pack` / `--receive-pack` name the program the *other* side of a
///   transport runs.
/// * `--paginate` / `--open-files-in-pager` run a pager.
/// * `--git-dir`, `--work-tree`, `--namespace` retarget the invocation at something
///   other than the repository the step was given.
/// * `ext::` is a transport that names a program.
///
/// A read step needs none of them. The list is refusals **by name**, because a
/// refusal that says "invalid argument" teaches the next author nothing.
pub const REFUSED_GIT_FORMS: &[(&str, &str)] = &[
    (
        "-c",
        "sets a config value for this invocation, and `-c alias.x=!cmd` runs a shell command",
    ),
    (
        "--config",
        "sets a config value for this invocation (`-c` long form)",
    ),
    (
        "--config-env",
        "sets a config value from the environment for this invocation",
    ),
    ("--exec-path", "points git at a different binary directory"),
    (
        "--upload-pack",
        "names the program the receiving side of a transport runs",
    ),
    (
        "--receive-pack",
        "names the program the sending side of a transport runs",
    ),
    ("--paginate", "runs a pager program"),
    ("--open-files-in-pager", "names the pager program to run"),
    (
        "--git-dir",
        "retargets the invocation at another repository",
    ),
    (
        "--work-tree",
        "retargets the invocation at another working tree",
    ),
    (
        "--namespace",
        "retargets the invocation at another namespace",
    ),
    (
        "-C",
        "runs git as if it had been started in another directory",
    ),
    ("ext::", "a transport that names a program"),
];

/// Flags that make a `git_read` subcommand **write**, not read (#1501).
///
/// The allowlist used to answer one question — "can this execute?" — and `#1501` is the other:
/// "can this write?". Measured on 2.50.1 against the list below, all four succeeded:
///
/// ```text
/// git symbolic-ref HEAD refs/heads/extra       → HEAD moved
/// git tag -d v1                                → the tag was deleted
/// git log --output=/tmp/x                      → the file was written
/// git branch newb                              → the branch was created
/// ```
///
/// A write is a consequence, and a step kind is part of the consequence gate's input
/// (ADR-017): a step called `git_read` that moves HEAD presents as a read to every policy
/// that looks at the name. `--output` is the sharpest of these — it writes **anywhere the
/// engine's uid can write**, and no register row described it, because the write happened
/// inside git.
///
/// Where a form is refused rather than the subcommand removed, the reason is that the read is
/// real and common (`branch --list`, `tag --list`, `git log`) — but the flag table is a list
/// someone must keep complete, so anything a read needs *more* than one word to express is a
/// form this refuses, never a form it silently allows.
pub const REFUSED_GIT_READ_FLAGS: &[(&str, &str)] = &[
    (
        "--output",
        "writes the result to a file, anywhere the engine's user can write",
    ),
    (
        "--ext-diff",
        "runs the external diff program the repository configures",
    ),
    ("--delete", "deletes (a ref, a tag, a branch)"),
    ("--move", "moves or renames a branch"),
    ("--copy", "copies a branch"),
    ("--force", "forces a write (a ref, a tag, a branch)"),
    (
        "--set-upstream-to",
        "sets a branch's upstream, which writes `.git/config`",
    ),
    ("--unset-upstream", "writes `.git/config`"),
    ("--edit-description", "opens an editor and writes `.git/`"),
    ("--create-reflog", "creates a ref"),
    ("--sign", "signs, and runs gpg"),
    ("--local-user", "names the signing key and runs gpg"),
    ("--cleanup", "rewrites a message"),
];

/// Short flags that write, refused **only** for the subcommands that write refs (#1501).
///
/// These letters are not globally safe to refuse: git reuses them across families, and `-c`,
/// `-C` and `-m` are *read* flags for `log`, `show` and `diff` (`git grep -c` counts matches,
/// `git log -c` shows combined diffs, `git log -m` shows merge diffs). Refusing them for every
/// subcommand is what #1502 found; refusing them where they write is the distinction git's own
/// grammar draws, and it is why this table is consulted only for `branch`, `tag` and
/// `symbolic-ref`.
pub const REFUSED_REF_WRITE_FLAGS: &[(&str, &str)] = &[
    ("-d", "deletes (a ref, a tag, a branch)"),
    ("-D", "deletes a branch even when it is not merged"),
    ("-m", "writes a message, or moves a branch"),
    ("-M", "moves a branch, overwriting"),
    ("-c", "copies a branch"),
    ("-C", "copies a branch, overwriting"),
    ("-f", "forces a write"),
    ("-u", "sets a branch's upstream, which writes `.git/config`"),
    ("-s", "signs, and runs gpg"),
    ("-F", "reads a message file and writes a tag"),
    ("-e", "opens an editor and writes `.git/`"),
];

/// Subcommands a `git_read` step may run: reads, and nothing that writes or executes.
///
/// The dispatcher's `git_read` is a read step, so the list is what the name promises, and
/// #1501 is the reason the promise now has two clauses. The previous version answered only
/// "can this execute?" and carried `symbolic-ref` — which moves HEAD — plus `branch` and `tag`,
/// which delete refs. A subcommand that is not here is refused, which is the point: the
/// dangerous forms are not a list someone has to keep complete, they are anything not
/// enumerated. `git_stage` and `git_commit` build their own argv and do not come through here.
///
/// `branch`, `tag` and `symbolic-ref` stay because their **read** forms are ordinary and
/// common; each is listed in `GIT_READ_FORM_SPECS` with the words that name its read form.
pub const GIT_READ_SUBCOMMANDS: &[&str] = &[
    "blame",
    "cat-file",
    "branch",
    "describe",
    "diff",
    "for-each-ref",
    "grep",
    "log",
    "ls-files",
    "ls-tree",
    "merge-base",
    "name-rev",
    "rev-list",
    "rev-parse",
    "shortlog",
    "show",
    "show-ref",
    "status",
    "symbolic-ref",
    "tag",
    "whatchanged",
];

/// The subcommands that have a write form one word away from their read form, and how a read
/// states which one it means (#1501).
///
/// Each entry is `(subcommand, read selector, positional words the read form takes)`. A
/// selector is required rather than inferred, and the positionals are counted, because
/// `branch --contains main` takes a word that is a *value* and `branch main` takes a word that
/// is a *target*; a counter cannot tell them apart without the flag list, so the flag list is
/// what the selector replaces.
pub const GIT_READ_FORM_SPECS: &[(&str, &str, usize)] = &[
    (
        "branch",
        "branch --list (or --contains/--merged/--no-merged/--points-at)",
        0,
    ),
    ("tag", "tag --list (or --contains/--merged/--points-at)", 0),
    (
        "symbolic-ref",
        "symbolic-ref <ref> (the query form; one word)",
        1,
    ),
];

/// Selectors that make `branch`/`tag` a read, and the words they consume beyond themselves.
///
/// A selector is an **allow** marker: a read flag that nobody listed here causes a refusal,
/// not a write. That is the direction a fail-closed default has to fail in.
const READ_SELECTORS: &[&str] = &[
    "--list",
    "-l",
    "--contains",
    "--merged",
    "--no-merged",
    "--points-at",
    "--format",
    "--sort",
    "-n",
    "--show-current",
    "-a",
    "-r",
    "-v",
    "-vv",
    "-i",
    "--ignore-case",
    "--column",
];

/// Refuse a `git_read` argument list that could run a program or write (#1498, #1501, #1502).
///
/// Four rules, in the order that produces the better message:
///
/// 1. **Leading global options** are consulted against `REFUSED_GIT_FORMS`. This is the only
///    position where `-c` is a config value: git's own grammar puts global options before the
///    subcommand, and `git status -c k=v` is `unknown switch 'c'`, not a config. Scanning every
///    word refused `git grep -c` and `git log -c`, where the word is the subcommand's own read
///    flag (#1502).
/// 2. **The subcommand** must be on the allowlist — deny by default, so the forms nobody
///    enumerated are refused rather than the dangerous ones being a list to keep complete.
/// 3. **The subcommand's read form** must be named, where the write form is one word away
///    (`branch --list` vs `branch newb`).
/// 4. **The subcommand's arguments** may not name a write (`--output`, `-d`, `--set-upstream-to`,
///    `--ext-diff`, …).
pub fn validate_git_read_args(args: &[&str]) -> Result<(), String> {
    let words: Vec<&str> = args
        .iter()
        .copied()
        .filter(|word| !word.is_empty())
        .collect();
    let first = *words.first().ok_or_else(|| {
        "the git step has no subcommand; it needs one of: ".to_string()
            + &GIT_READ_SUBCOMMANDS.join(", ")
            + ", branch --list, tag --list"
    })?;

    // 1. A leading word that starts with `-` is a global option, and a read needs none.
    if first.starts_with('-') {
        // `-c` and its attached form are the ones worth a paragraph: the same word after a
        // subcommand is a read flag, so the refusal has to say which position it is in.
        if first == "-c" || (first.starts_with("-c") && !first.starts_with("--")) {
            return Err(format!(
                "git argument '{first}' is refused **in this position**: before the subcommand \
                 `-c` means `-c <key>=<value>` for this invocation, and `-c alias.x=!cmd` runs a \
                 shell command. After the subcommand the same word is that subcommand's own \
                 flag (`git grep -c`, `git log -c`), which is why it is accepted there. Put the \
                 subcommand first — and use `--count` if you meant `grep -c` (#1498, #1502)"
            ));
        }
        for (form, why) in REFUSED_GIT_FORMS {
            if first == *form || first.starts_with(&format!("{form}=")) {
                return Err(format!(
                    "git argument '{first}' is refused: as a leading option {form} {why}. A \
                     `git_read` step reads the repository as it was given; it never retargets the \
                     invocation or names a program (#1498)"
                ));
            }
        }
        return Err(format!(
            "'{first}' is refused: a git option before the subcommand is a global option, and a \
             read needs none of them (they set config, retarget the repository, or run a \
             program). Start the step with one of: {} (#1498)",
            GIT_READ_SUBCOMMANDS.join(", ")
        ));
    }

    // `ext::` is a transport, so it is a value rather than a leading option.
    if words.iter().any(|word| word.contains("ext::")) {
        return Err(
            "git argument containing 'ext::' is refused: a transport that names a program \
             (#1498)"
                .to_string(),
        );
    }

    // 2. Deny by default.
    let spec = GIT_READ_FORM_SPECS
        .iter()
        .find(|(name, _, _)| *name == first);
    if !GIT_READ_SUBCOMMANDS.contains(&first) && spec.is_none() {
        return Err(format!(
            "'{first}' is not a read subcommand. A `git_read` step may run: {}. \
             Anything else can write, fetch or execute, which is what `git_stage`, `git_commit` \
             and `run_command` are for — and those require a declared isolation (#1498)",
            GIT_READ_SUBCOMMANDS.join(", ")
        ));
    }

    let rest = &words[1..];

    // 4. before 3, so `branch -D main` is refused for the reason it deserves rather than for
    // the missing selector.
    for word in rest {
        let refused = REFUSED_GIT_READ_FLAGS.iter().chain(if spec.is_some() {
            REFUSED_REF_WRITE_FLAGS.iter()
        } else {
            [].iter()
        });
        for (form, why) in refused {
            if *word == *form || word.starts_with(&format!("{form}=")) {
                return Err(format!(
                    "git argument '{word}' is refused: for a `git_read` step {form} {why}. A \
                     read does not change a ref, the index or a file; a write is `git_stage`, \
                     `git_commit` or `run_command`, and each requires a declared isolation \
                     (#1501)"
                ));
            }
        }
    }

    // 3. The read form has to be named.
    if let Some((_, selector, max_positionals)) = spec {
        let named = rest.iter().any(|word| {
            READ_SELECTORS
                .iter()
                .any(|s| word == s || word.starts_with(&format!("{s}=")))
        });
        let positionals = rest.iter().filter(|word| !word.starts_with('-')).count();
        let fits_query_form = positionals <= *max_positionals && (*max_positionals > 0);
        if !named && !fits_query_form {
            return Err(format!(
                "'{first}' has a write form one word away from its read form (`{first} <name>` \
                 creates; `{first} -d/-D <name>` deletes), so a `git_read` step must name the \
                 read form: {selector}. Measured on 2.50.1: `git branch newb` created a branch \
                 and `git symbolic-ref HEAD refs/heads/x` moved HEAD (#1501)"
            ));
        }
    }

    Ok(())
}

/// The deployment fact that must exist before `run-command` may be enabled.
///
/// `none` is a recognized value and a refusal: declaring that there is no
/// boundary is not a declaration of one.
pub const SANDBOX_ENV: &str = "RIGORIX_SANDBOX";

/// What the allowlist actually matches.
///
/// The unit is the executable, not the argv. `is_command_allowed` refuses a
/// control operator and checks the first word, so `cargo build` passes for an
/// allowlisted `cargo` while `build.rs` runs whatever it likes. Stating the
/// grain here is the point: an allowlist of build tools is an allowlist of
/// arbitrary code execution.
pub const SINK_GRAIN: &str = "the executable (argv[0]); the arguments are unconstrained";

/// What a child reaches when no isolation boundary exists.
///
/// This is the sentence that decides whether the feature is safe, so it lives in
/// the type the operator must construct rather than in a comment.
pub const UNSANDBOXED_REACH: &str = "the child runs as the engine's user, with the engine's \
filesystem access, network reach and any credential it can find on disk — filtering decides which \
executable starts, not what it can do once started";

/// The isolation regime a deployment declares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxRegime {
    /// No boundary. Recognized so it can be refused by name.
    None,
    /// The child runs inside a container boundary.
    Container,
    /// The child runs inside a VM boundary.
    Vm,
}

impl SandboxRegime {
    /// The value as written in `RIGORIX_SANDBOX`.
    pub fn as_str(self) -> &'static str {
        match self {
            SandboxRegime::None => "none",
            SandboxRegime::Container => "container",
            SandboxRegime::Vm => "vm",
        }
    }

    /// Whether this regime is an isolation boundary at all.
    pub fn is_isolation(self) -> bool {
        !matches!(self, SandboxRegime::None)
    }
}

/// A refusal to declare, or a declaration of nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SandboxDeclarationError {
    /// `RIGORIX_SANDBOX` is unset — the deployment never stated the fact.
    Missing,
    /// The value is not one of `none` | `container` | `vm`.
    Unrecognized {
        /// The value that was supplied.
        value: String,
    },
    /// The deployment declared `none`: no boundary, so the sink stays off.
    NoIsolation,
}

impl std::fmt::Display for SandboxDeclarationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SandboxDeclarationError::Missing => write!(
                f,
                "{SANDBOX_ENV} is unset. run-command spawns a process outside the engine's type \
                 system, and filtering is not isolation: {UNSANDBOXED_REACH}. Set \
                 {SANDBOX_ENV}=container (or =vm) to declare the boundary the deployment provides, \
                 or leave run-command disabled."
            ),
            SandboxDeclarationError::Unrecognized { value } => write!(
                f,
                "{SANDBOX_ENV}={value:?} is not a declared fact — expected one of none | container \
                 | vm."
            ),
            SandboxDeclarationError::NoIsolation => write!(
                f,
                "{SANDBOX_ENV}=none declares that no boundary exists, so run-command is not \
                 enabled: {UNSANDBOXED_REACH}. Provide a container or VM boundary, or leave the \
                 tool disabled."
            ),
        }
    }
}

impl std::error::Error for SandboxDeclarationError {}

/// The declared isolation a deployment provides for spawned children.
///
/// This is a *declaration*, not a proof: constructing one is how an operator
/// states the fact, and `RunCommandTool` cannot be built without one. The
/// declaration is recorded in the signed envelope metadata by the deployment
/// that supplies it, so the evidence says which regime produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxDeclaration {
    regime: SandboxRegime,
    declared: String,
}

impl SandboxDeclaration {
    /// Parse a declaration value (`none` | `container` | `vm`, case-insensitive).
    ///
    /// `none` is parsed and then refused: it is a valid *statement* that is not
    /// a valid *declaration of isolation*.
    pub fn from_value(value: &str) -> Result<Self, SandboxDeclarationError> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            // An empty value declared nothing, so it is refused as a missing
            // declaration — and carries the reach with it.
            return Err(SandboxDeclarationError::Missing);
        }
        let regime = match trimmed.to_ascii_lowercase().as_str() {
            "container" => SandboxRegime::Container,
            "vm" => SandboxRegime::Vm,
            "none" => SandboxRegime::None,
            _ => {
                return Err(SandboxDeclarationError::Unrecognized {
                    value: trimmed.to_string(),
                });
            }
        };
        if !regime.is_isolation() {
            return Err(SandboxDeclarationError::NoIsolation);
        }
        Ok(Self {
            regime,
            declared: trimmed.to_ascii_lowercase(),
        })
    }

    /// Read the declaration from the deployment (`RIGORIX_SANDBOX`).
    pub fn from_env() -> Result<Self, SandboxDeclarationError> {
        // sink: tools:shell-policy
        match std::env::var(SANDBOX_ENV) {
            Ok(value) => Self::from_value(&value),
            Err(std::env::VarError::NotPresent) => Err(SandboxDeclarationError::Missing),
            Err(std::env::VarError::NotUnicode(_)) => Err(SandboxDeclarationError::Unrecognized {
                value: "<non-unicode>".to_string(),
            }),
        }
    }

    /// A container declaration (tests and composition roots that own one).
    pub fn container() -> Self {
        Self {
            regime: SandboxRegime::Container,
            declared: "container".to_string(),
        }
    }

    /// A VM declaration.
    pub fn vm() -> Self {
        Self {
            regime: SandboxRegime::Vm,
            declared: "vm".to_string(),
        }
    }

    /// The declared regime.
    pub fn regime(&self) -> SandboxRegime {
        self.regime
    }

    /// The value as declared, for recording alongside the evidence.
    pub fn declared(&self) -> &str {
        &self.declared
    }

    /// What this declaration relies on, in the register's `requires` column
    /// vocabulary.
    pub fn requires(&self) -> &'static str {
        match self.regime {
            SandboxRegime::Container => "deployment isolation (RIGORIX_SANDBOX=container)",
            SandboxRegime::Vm => "deployment isolation (RIGORIX_SANDBOX=vm)",
            SandboxRegime::None => "nothing — filtering only (refused)",
        }
    }
}

/// Resolve the declaration a host should attach at startup, warning once.
///
/// #1487: a host that runs plan steps must declare the isolation those steps run
/// under, but it must not refuse to *start*: a plan with no `run_command` step is
/// perfectly serviceable, and turning a missing declaration into a startup failure
/// would make the failure appear before anyone asked for a shell. So the remedy is
/// printed once, here, and every `run_command` step then fails with
/// `sandbox_undeclared` — the refusal happens at the step, where it is evidence.
///
/// `host` names the composition root so the message says which binary to fix.
pub fn declaration_for_host(host: &str) -> Option<std::sync::Arc<SandboxDeclaration>> {
    match SandboxDeclaration::from_env() {
        Ok(declaration) => Some(std::sync::Arc::new(declaration)),
        Err(error) => {
            tracing::warn!(
                host = %host,
                "{error} Until then every run_command step will fail with sandbox_undeclared."
            );
            None
        }
    }
}

/// Shell constructs that turn one command into a compound.
///
/// Ordering only affects which operator is named in the rejection message;
/// `&&`/`||` are listed first so the message is specific rather than reporting
/// a bare `&` or `|`.
const CONTROL_OPERATORS: &[&str] = &["&&", "||", ";", "|", "`", "$(", ">", "<", "\n", "\r", "&"];

/// Return the first shell control operator present in `command`.
///
/// `None` means the string is a single simple command and is safe to hand to
/// `sh -c` after the allowlist check.
pub fn find_shell_control_operator(command: &str) -> Option<&'static str> {
    CONTROL_OPERATORS
        .iter()
        .copied()
        .find(|operator| command.contains(operator))
}

/// Extract one string field from a JSON tool intent.
///
/// Falls back to the whole intent when it is not JSON or lacks the field, which
/// preserves the behaviour for callers that pass a bare command string.
pub fn field_from_intent<'a>(intent: &'a str, field: &str) -> Cow<'a, str> {
    match serde_json::from_str::<serde_json::Value>(intent) {
        Ok(value) => match value.get(field).and_then(|value| value.as_str()) {
            Some(field_value) => Cow::Owned(field_value.to_string()),
            None => Cow::Borrowed(intent),
        },
        Err(_) => Cow::Borrowed(intent),
    }
}

/// The command a `run_command` intent means to execute.
///
/// This is the one extraction used by the bash classifier and by the executor;
/// keeping it single is the invariant, not an implementation detail.
pub fn command_from_intent(intent: &str) -> Cow<'_, str> {
    field_from_intent(intent, "command")
}

/// A command that cannot be executed as argv without a shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimpleCommandError {
    /// Nothing to execute.
    Empty,
    /// A quote was opened and never closed.
    UnbalancedQuote {
        /// The quote that remained open.
        quote: char,
    },
    /// The string ended in a backslash.
    TrailingEscape,
    /// A character a shell would expand, in a position where it would expand.
    ShellExpansion {
        /// The character found.
        character: char,
        /// Its byte offset in the command.
        position: usize,
    },
}

impl std::fmt::Display for SimpleCommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SimpleCommandError::Empty => write!(f, "the command is empty"),
            SimpleCommandError::UnbalancedQuote { quote } => {
                write!(f, "unbalanced {quote} quote")
            }
            SimpleCommandError::TrailingEscape => write!(f, "the command ends in a backslash"),
            SimpleCommandError::ShellExpansion {
                character,
                position,
            } => write!(
                f,
                "{character:?} at byte {position} would be expanded by a shell, and \
                 run-command does not use one — quote it (single quotes expand nothing) or \
                 remove it"
            ),
        }
    }
}

impl std::error::Error for SimpleCommandError {}

/// Characters a shell would transform rather than pass through.
///
/// Outside quotes all of these are refused. Inside double quotes only `$` and
/// the backtick still expand; inside single quotes nothing does, so a literal
/// `[` or `*` in a filename is expressible and stays expressible.
const SHELL_EXPANSION_CHARS: &[char] = &[
    '$', '`', '*', '?', '[', ']', '{', '}', '~', '!', ';', '&', '|', '<', '>', '(', ')', '#',
];

/// Split a proven-single command into the argv a shell would have produced.
///
/// The returned vector is what [`std::process::Command`] receives: `argv[0]` is
/// the program and the rest are arguments, with no shell in between. Quoting
/// follows the shell's rules for a *single simple command*:
///
/// - whitespace separates tokens;
/// - `'…'` is literal (nothing inside is special);
/// - `"…"` is literal except `\"` and `\\`;
/// - a backslash outside quotes escapes the next character.
///
/// Anything that would need the shell — `$`, a backtick, a glob, a brace, a
/// subshell — is refused rather than silently passed through unexpanded, because
/// a caller who wrote it expected the shell.
pub fn argv_from_simple(command: &str) -> Result<Vec<String>, SimpleCommandError> {
    #[derive(PartialEq)]
    enum Quote {
        None,
        Single,
        Double,
    }

    let mut argv: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut started = false;
    let mut quote = Quote::None;
    let mut escaped = false;

    for (position, character) in command.char_indices() {
        if escaped {
            current.push(character);
            escaped = false;
            started = true;
            continue;
        }
        match quote {
            Quote::Single => {
                if character == '\'' {
                    quote = Quote::None;
                } else {
                    current.push(character);
                }
            }
            Quote::Double => match character {
                '"' => quote = Quote::None,
                '\\' => escaped = true,
                '$' | '`' => {
                    return Err(SimpleCommandError::ShellExpansion {
                        character,
                        position,
                    });
                }
                other => current.push(other),
            },
            Quote::None => match character {
                '\'' => {
                    quote = Quote::Single;
                    started = true;
                }
                '"' => {
                    quote = Quote::Double;
                    started = true;
                }
                '\\' => {
                    escaped = true;
                    started = true;
                }
                c if c.is_whitespace() => {
                    if started {
                        argv.push(std::mem::take(&mut current));
                        started = false;
                    }
                }
                c if SHELL_EXPANSION_CHARS.contains(&c) => {
                    return Err(SimpleCommandError::ShellExpansion {
                        character: c,
                        position,
                    });
                }
                other => {
                    current.push(other);
                    started = true;
                }
            },
        }
    }

    if escaped {
        return Err(SimpleCommandError::TrailingEscape);
    }
    match quote {
        Quote::Single => {
            return Err(SimpleCommandError::UnbalancedQuote { quote: '\'' });
        }
        Quote::Double => return Err(SimpleCommandError::UnbalancedQuote { quote: '"' }),
        Quote::None => {}
    }
    if started {
        argv.push(current);
    }
    if argv.is_empty() {
        return Err(SimpleCommandError::Empty);
    }
    Ok(argv)
}

/// Give a child command a minimal, explicit environment.
///
/// Clears everything inherited, re-adds the allowlist from the engine's own
/// environment when present, then applies the per-call overrides last so a plan
/// can set a variable the command needs.
pub fn apply_child_env(cmd: &mut tokio::process::Command, overrides: &[(String, String)]) {
    cmd.env_clear();
    for key in INHERITED_ENV_ALLOWLIST {
        // sink: tools:shell-policy-env
        if let Ok(value) = std::env::var(key) {
            cmd.env(key, value);
        }
    }
    for (key, value) in overrides {
        cmd.env(key, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #1495 acceptance 2: the comment does not claim more than the mechanism does.
    ///
    /// `resolve_program` returns a `PathBuf` and the caller passes it to
    /// `Command::new`, which re-opens that path in the child. A writer in the
    /// directory can replace the file in between. It pinned the *path*, not the
    /// inode, and the doc comment said otherwise — which is the defect class these
    /// batches are about: the sentence promising the control is the sentence that
    /// prevents anyone from looking.
    #[test]
    fn the_resolver_does_not_claim_to_pin_the_file() {
        let source = include_str!("shell_policy.rs");
        let docs = source
            .split("pub fn resolve_program")
            .next()
            .expect("the function exists");
        assert!(
            !docs.contains("makes the executed file the one the policy saw"),
            "the doc comment must not claim the executed file is the resolved one (#1495)"
        );
        assert!(
            docs.contains("does **not** pin the file"),
            "the doc comment must state the gap the register records (#1495)"
        );
    }

    #[test]
    fn control_operators_are_detected() {
        for command in [
            "echo ok; rm -rf /",
            "echo ok && rm -rf /",
            "echo ok || rm -rf /",
            "echo ok | sh",
            "echo ok & ",
            "echo `id`",
            "echo $(id)",
            "echo ok > /etc/passwd",
            "echo ok < /etc/passwd",
            "echo ok\nrm -rf /",
        ] {
            assert!(
                find_shell_control_operator(command).is_some(),
                "{command:?} must be refused"
            );
        }
    }

    #[test]
    fn simple_commands_have_no_operator() {
        for command in [
            "cargo test",
            "echo hello",
            "git log --oneline -5",
            "ls -la src/",
        ] {
            assert_eq!(find_shell_control_operator(command), None, "{command:?}");
        }
    }

    #[test]
    fn command_is_extracted_from_the_json_intent() {
        let intent = r#"{"tool":"run_command","command":"rm -rf /"}"#;
        assert_eq!(command_from_intent(intent), "rm -rf /");
    }

    #[test]
    fn bare_intent_is_its_own_command() {
        assert_eq!(command_from_intent("cargo test"), "cargo test");
    }

    #[test]
    fn the_secret_key_is_not_inherited() {
        assert!(!INHERITED_ENV_ALLOWLIST.contains(&"RIGORIX_HMAC_KEY"));
        assert!(INHERITED_ENV_ALLOWLIST.contains(&"PATH"));
    }

    // -----------------------------------------------------------------------
    // #1478: the declared isolation fact
    // -----------------------------------------------------------------------

    #[test]
    fn an_undeclared_sandbox_is_refused_and_names_the_reach() {
        let message = SandboxDeclaration::from_value(" ").unwrap_err().to_string();
        assert!(
            message.contains("runs as the engine's user"),
            "the refusal must name what the child can reach: {message}"
        );
    }

    #[test]
    fn no_isolated_value_is_refused() {
        let honest = SandboxDeclaration::from_value("none").unwrap_err();
        assert_eq!(honest, SandboxDeclarationError::NoIsolation);
        assert!(
            honest.to_string().contains("not enabled"),
            "declaring no boundary must leave the sink off: {honest}"
        );
        assert!(SandboxDeclaration::from_value("container").is_ok());
        assert!(SandboxDeclaration::from_value("VM").is_ok());
        assert!(matches!(
            SandboxDeclaration::from_value("chroot-ish"),
            Err(SandboxDeclarationError::Unrecognized { .. })
        ));
    }

    #[test]
    fn a_declaration_names_what_it_relies_on() {
        assert_eq!(
            SandboxDeclaration::container().requires(),
            "deployment isolation (RIGORIX_SANDBOX=container)"
        );
        assert!(SandboxDeclaration::vm().regime().is_isolation());
        assert!(!SandboxRegime::None.is_isolation());
    }

    // -----------------------------------------------------------------------
    // #1478: argv, not a shell
    // -----------------------------------------------------------------------

    #[test]
    fn a_simple_command_becomes_argv() {
        assert_eq!(
            argv_from_simple("cargo test --lib").unwrap(),
            vec!["cargo", "test", "--lib"]
        );
        assert_eq!(
            argv_from_simple("echo 'hello world'").unwrap(),
            vec!["echo", "hello world"]
        );
        assert_eq!(
            argv_from_simple("grep -n \"a b\" src/main.rs").unwrap(),
            vec!["grep", "-n", "a b", "src/main.rs"]
        );
        assert_eq!(
            argv_from_simple("ls -la ./dir/").unwrap(),
            vec!["ls", "-la", "./dir/"]
        );
    }

    #[test]
    fn quoting_rules_are_the_documented_ones() {
        // single quotes expand nothing: a glob stays a literal argument
        assert_eq!(
            argv_from_simple("find . -name '*.rs'").unwrap(),
            vec!["find", ".", "-name", "*.rs"]
        );
        // an escape makes a metacharacter literal
        assert_eq!(
            argv_from_simple("echo \\$HOME").unwrap(),
            vec!["echo", "$HOME"]
        );
        // empty quoted token is preserved
        assert_eq!(argv_from_simple("echo ''").unwrap(), vec!["echo", ""]);
    }

    #[test]
    fn a_string_that_needs_a_shell_is_refused() {
        for command in [
            "echo $HOME",
            "echo `id`",
            "echo a*",
            "echo \"$HOME\"",
            "echo {a,b}",
            "echo ~/x",
            "echo 'unbalanced",
            "echo \"unbalanced",
            "echo trailing\\",
            "   ",
        ] {
            assert!(
                argv_from_simple(command).is_err(),
                "{command:?} must be refused, not passed through"
            );
        }
    }

    // ── #1501: a read may not write ──────────────────────────────────────────

    /// Every form below was measured to succeed against real git before it was refused here
    /// (2.50.1): `symbolic-ref HEAD refs/heads/extra` moved HEAD, `tag -d v1` deleted the tag,
    /// `log --output=<path>` wrote the file. A refusal that only asserts `is_err` would pass
    /// against a validator that refused everything, which is what #1501 is about.
    #[test]
    fn a_read_step_cannot_write() {
        for args in [
            vec!["symbolic-ref", "HEAD", "refs/heads/extra"],
            vec!["branch", "-D", "main"],
            vec!["branch", "--delete", "main"],
            vec!["branch", "newb"],
            vec!["branch", "--set-upstream-to=origin/main"],
            vec!["branch", "-m", "a", "b"],
            vec!["tag", "-d", "v1"],
            vec!["tag", "v1"],
            vec!["tag", "--delete", "v1"],
            vec!["log", "--output=/tmp/x"],
            vec!["log", "--output", "/tmp/x"],
            vec!["diff", "--ext-diff"],
            vec!["show", "--output=/tmp/x"],
            vec!["symbolic-ref", "-d", "refs/heads/x"],
        ] {
            let err = crate::tools::application::shell_policy::validate_git_read_args(&args)
                .expect_err(&format!("{args:?} writes and must be refused"));
            assert!(
                err.contains(args[0]) || err.contains("git argument"),
                "the refusal must name the form: {err}"
            );
        }
    }

    /// The mirror: the reads that the flag table would have refused if it had been applied to
    /// every subcommand (`-c`, `-C`, `-m` are read flags for these), plus the ref subcommands'
    /// read forms. Each was run against git 2.50.1 and exited 0.
    #[test]
    fn an_ordinary_read_still_reads() {
        for args in [
            vec!["log", "--oneline", "-5"],
            vec!["status", "--porcelain"],
            vec!["show", "HEAD"],
            vec!["diff"],
            vec!["diff", "-c"],
            vec!["grep", "-c", "TODO"],
            vec!["grep", "--count", "TODO"],
            vec!["log", "-c", "--oneline", "-1"],
            vec!["log", "--name-only"],
            vec!["rev-parse", "HEAD"],
            vec!["rev-parse", "--abbrev-ref", "HEAD"],
            vec!["branch", "--list"],
            vec!["branch", "--list", "ma*"],
            vec!["branch", "-a"],
            vec!["branch", "--contains", "HEAD"],
            vec!["tag", "--list"],
            vec!["tag", "-l", "v*"],
            vec!["symbolic-ref", "HEAD"],
            vec!["for-each-ref", "refs/tags"],
            vec!["ls-files"],
            vec!["cat-file", "-p", "HEAD"],
        ] {
            crate::tools::application::shell_policy::validate_git_read_args(&args)
                .unwrap_or_else(|err| panic!("{args:?} is a read and must be accepted: {err}"));
        }
    }

    /// `-c` is a config value before the subcommand and a read flag after it, and the refusal
    /// has to say which position it is in — otherwise an author who typed `git grep -c` is told
    /// they tried to set a config value (#1502).
    #[test]
    fn the_config_form_is_refused_in_the_leading_position_only() {
        for args in [
            vec!["-c", "alias.pwn=!touch", "pwn"],
            vec!["-calias.pwn=!touch", "pwn"],
            vec!["--config", "core.fsmonitor=/x", "status"],
        ] {
            let err = crate::tools::application::shell_policy::validate_git_read_args(&args)
                .expect_err("a leading config form must be refused");
            assert!(
                err.contains("position") || err.contains("config"),
                "the refusal must explain the position: {err}"
            );
        }
        // The same word after the subcommand is the subcommand's own flag.
        crate::tools::application::shell_policy::validate_git_read_args(&["grep", "-c", "TODO"])
            .expect("`git grep -c` counts matches; it is not the config form");
    }
}

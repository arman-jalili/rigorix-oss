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
}

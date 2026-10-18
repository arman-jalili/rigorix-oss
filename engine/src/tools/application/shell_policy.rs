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

use std::borrow::Cow;

/// Environment variables a child command inherits.
///
/// Kept deliberately small. `PATH`/`HOME` are needed to resolve tools and their
/// configuration; the locale and temp-dir names only affect output formatting.
/// Anything else — notably `RIGORIX_HMAC_KEY` and any anchor or cloud
/// credentials — is withheld.
pub const INHERITED_ENV_ALLOWLIST: &[&str] =
    &["PATH", "HOME", "LANG", "LC_ALL", "TMPDIR", "SHELL", "TERM"];

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
}

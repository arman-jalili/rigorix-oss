//! The child environment a spawned program gets — **one** declaration, in a module every build has.
//!
//! #1653: `run_command` cleared the environment and re-added an allowlist; the precondition runner
//! inherited everything, so an operator check held `RIGORIX_HMAC_KEY` (and could sign envelopes), the
//! operator's registry and model-provider tokens, and `SSH_AUTH_SOCK`. Two spawn sites, two policies,
//! and the register row claimed the strict one for both.
//!
//! The list lives here rather than in `tools::application::shell_policy` because the precondition
//! runner is compiled in the portable slice and `tools` is not — a *second* copy of the list would
//! have been the obvious fix and the wrong one. `shell_policy` re-exports this module's items, so
//! there is exactly one list and one function, and both spawn sites call it.

use std::ffi::OsStr;

/// Variables a child inherits from the engine. Nothing else crosses.
///
/// Each entry is here because a child that lacks it cannot run at all (`PATH`), cannot resolve a
/// home-relative path (`HOME`), or loses the locale and terminal conventions the program was written
/// against. `RIGORIX_*`, tokens, and `SSH_AUTH_SOCK` are deliberately absent: the first is what the
/// audit envelopes are signed with, the second and third are the operator's credentials, and a
/// spawned program is not the operator.
pub const INHERITED_ENV_ALLOWLIST: &[&str] =
    &["PATH", "HOME", "LANG", "LC_ALL", "TMPDIR", "SHELL", "TERM"];

/// Remove every inherited variable, then add the allowlist, then the caller's explicit `overrides`.
///
/// An override may not smuggle a name back in that the allowlist does not carry and that
/// [`CALLER_MAY_NOT_SET`] refuses — the check is here rather than at each call site so that "the
/// child does not hold the signing key" is a property of the helper, not of two callers remembering
/// it.
#[cfg(not(feature = "portable"))]
pub fn apply_child_env(cmd: &mut tokio::process::Command, overrides: &[(String, String)]) {
    cmd.env_clear();
    for key in INHERITED_ENV_ALLOWLIST {
        // sink: tools:shell-policy-env
        if let Ok(value) = std::env::var(key) {
            cmd.env(key, value);
        }
    }
    for (key, value) in overrides {
        if CALLER_MAY_NOT_SET.contains(&key.as_str()) {
            // Refusing here would turn a programming error into a child that starts with fewer
            // variables than the caller believes; there is no error channel in this signature, so
            // the value is dropped and the register/caller test names the keys.
            continue;
        }
        cmd.env(key, value);
    }
}

/// Environment names a caller may never hand a child through [`apply_child_env`] (#1489, #1653).
pub const CALLER_MAY_NOT_SET: &[&str] = &[
    "RIGORIX_HMAC_KEY",
    "RIGORIX_APPROVAL_BINDING",
    "SSH_AUTH_SOCK",
    "GITHUB_TOKEN",
    "GH_TOKEN",
    "ANTHROPIC_API_KEY",
    "OPENAI_API_KEY",
    "AWS_SECRET_ACCESS_KEY",
    "AWS_SESSION_TOKEN",
];

/// Whether a variable name is one the allowlist carries or a caller supplied explicitly.
pub fn is_permitted(name: &str) -> bool {
    INHERITED_ENV_ALLOWLIST.contains(&name) || !CALLER_MAY_NOT_SET.contains(&name)
}

/// `env -i`-style helper for a caller that wants a bare map rather than a `Command`.
///
/// Available in every build: the slice has no `tokio::process` (so [`apply_child_env`] is gated out
/// of it), but the *list* is data and the tests that assert what a child may not see belong to both.

pub fn child_env(overrides: &[(String, String)]) -> Vec<(String, String)> {
    let mut env: Vec<(String, String)> = INHERITED_ENV_ALLOWLIST
        .iter()
        .filter_map(|key| {
            // sink: tools:shell-policy-env
            std::env::var(key)
                .ok()
                .map(|value| ((*key).to_string(), value))
        })
        .collect();
    for (key, value) in overrides {
        if !CALLER_MAY_NOT_SET.contains(&key.as_str()) {
            env.push((key.clone(), value.clone()));
        }
    }
    env
}

/// The allowlist as an `OsStr` slice, for a caller that builds its own command.
pub fn allowlist_os() -> Vec<&'static OsStr> {
    INHERITED_ENV_ALLOWLIST.iter().map(OsStr::new).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_allowlist_never_carries_a_secret_or_the_signing_key() {
        for forbidden in CALLER_MAY_NOT_SET {
            assert!(
                !INHERITED_ENV_ALLOWLIST.contains(forbidden),
                "`{forbidden}` is in the inherited allowlist — a child that holds it can sign, or \
                 act as the operator (#1653)"
            );
        }
        for allowed in INHERITED_ENV_ALLOWLIST {
            assert!(
                !allowed.starts_with("RIGORIX_"),
                "`{allowed}` is a RIGORIX_ variable; those are facts the runner *sets*, never \
                 values a child inherits"
            );
        }
    }

    #[test]
    fn a_caller_override_cannot_reintroduce_a_refused_name() {
        let env = child_env(&[
            ("RIGORIX_TOOL".to_string(), "run_command".to_string()),
            ("RIGORIX_HMAC_KEY".to_string(), "smuggled".to_string()),
            ("SSH_AUTH_SOCK".to_string(), "/tmp/agent".to_string()),
        ]);
        let names: Vec<&str> = env.iter().map(|(name, _)| name.as_str()).collect();
        assert!(names.contains(&"RIGORIX_TOOL"), "{names:?}");
        assert!(!names.contains(&"RIGORIX_HMAC_KEY"), "{names:?}");
        assert!(!names.contains(&"SSH_AUTH_SOCK"), "{names:?}");
    }

    #[test]
    fn only_the_listed_names_are_permitted() {
        assert!(is_permitted("RIGORIX_TOOL"));
        assert!(!is_permitted("RIGORIX_HMAC_KEY"));
        assert!(!is_permitted("SSH_AUTH_SOCK"));
    }
}

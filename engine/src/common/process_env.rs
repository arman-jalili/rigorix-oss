// This module *is* the spawn-environment declaration: a `Command` is handed to it, never constructed
// here, so the disallowed-type lint (which keeps spawn sites in the audited modules) has nothing to
// find. The allow is scoped to this file and covers only the parameter type.
#![allow(clippy::disallowed_types)]

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

/// Expand the words of a shell command the way the **child's** shell will, for a decision (#1654).
///
/// `run_command` executes `sh -c <string>`: `~`, `$VAR` and globs are expanded by the shell *after*
/// the allowlist decision, so a decision taken on the literal words decides a different operand than
/// the one that gets used — `cp $CFG .rigorix/hooks.toml` was allowed on the strength of `$CFG` and
/// then wrote the operator's config file.
///
/// Two properties make this exact rather than approximate:
///
/// 1. the child's environment is an **allowlist** ([`INHERITED_ENV_ALLOWLIST`] plus the caller's
///    overrides), so the values a `$VAR` resolves to are the same values the decision uses;
/// 2. only the three forms the audit measured are expanded — `~`, `$VAR`/`${VAR}`, and a shell glob
///    against the directory the pattern names. Command substitution (`$(…)`, backticks) is refused
///    before this runs by the control-operator check, so it cannot assemble a path here.
///
/// The result is the operand set, not a command to execute: this function never builds a string that
/// is handed to a shell. A word that names no variable the child has stays literal — that is what the
/// child's shell would do too, and a variable the child cannot see cannot be the vector.
pub fn expand_words(command: &str, env: &[(String, String)]) -> Vec<String> {
    fn lookup<'a>(env: &'a [(String, String)], name: &str) -> Option<&'a str> {
        env.iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
    let home = lookup(env, "HOME").unwrap_or("");
    let mut words = Vec::new();
    for raw in command.split_whitespace() {
        let word = raw.trim_matches(|c: char| matches!(c, '"' | '\''));
        if word.is_empty() {
            continue;
        }
        let expanded = expand_one(word, home, env);
        // A glob that matches nothing stays literal (the shell's own rule), and a pattern with no
        // metacharacter is one path.
        if expanded.contains('*') || expanded.contains('?') || expanded.contains('[') {
            words.extend(glob_paths(&expanded));
        } else {
            words.push(expanded);
        }
    }
    words
}

fn expand_one(word: &str, home: &str, env: &[(String, String)]) -> String {
    // `~` and `~/x` — only as the first character, which is where the shell expands it.
    let mut out = if word == "~" {
        home.to_string()
    } else if let Some(rest) = word.strip_prefix("~/") {
        format!("{home}/{rest}")
    } else {
        word.to_string()
    };

    // `$NAME` and `${NAME}`.
    let mut result = String::with_capacity(out.len());
    let bytes: Vec<char> = out.chars().collect();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != '$' {
            result.push(bytes[index]);
            index += 1;
            continue;
        }
        let braced = bytes.get(index + 1) == Some(&'{');
        let start = if braced { index + 2 } else { index + 1 };
        let mut end = start;
        while end < bytes.len()
            && (bytes[end].is_ascii_alphanumeric() || bytes[end] == '_')
            && !(end == start && bytes[end].is_ascii_digit())
        {
            end += 1;
        }
        if end == start || (braced && bytes.get(end) != Some(&'}')) {
            result.push('$');
            index += 1;
            continue;
        }
        let name: String = bytes[start..end].iter().collect();
        // Not in the child's environment: the child's shell expands it to the empty string. Keeping
        // the literal would make the decision *more* permissive than the execution, which is the
        // wrong direction; expanding to "" is exact.
        if let Some(value) = env
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value.as_str())
        {
            result.push_str(value);
        }
        index = end + if braced { 1 } else { 0 };
    }
    out = result;
    out
}

/// The paths a shell glob names, restricted to the directory the pattern starts in.
///
/// A missing directory yields the pattern itself, which is what an unmatched glob is; the caller
/// applies a *refusal* rule to it, so a conservative non-expansion cannot admit anything.
fn glob_paths(pattern: &str) -> Vec<String> {
    let (dir, file_pattern) = match pattern.rsplit_once('/') {
        Some((dir, rest)) => (if dir.is_empty() { "/" } else { dir }, rest),
        None => (".", pattern),
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return vec![pattern.to_string()];
    };
    let mut matches: Vec<String> = entries
        .flatten()
        .filter_map(|entry| entry.file_name().to_str().map(str::to_string))
        .filter(|name| glob_match(file_pattern, name))
        .map(|name| {
            if dir == "." {
                name
            } else {
                format!("{dir}/{name}")
            }
        })
        .collect();
    if matches.is_empty() {
        return vec![pattern.to_string()];
    }
    matches.sort();
    matches
}

/// `*`, `?` and a single character class — the subset a path operand uses.
fn glob_match(pattern: &str, name: &str) -> bool {
    fn walk(pattern: &[char], name: &[char]) -> bool {
        match (pattern.first(), name.first()) {
            (None, None) => true,
            (Some('*'), _) => {
                walk(&pattern[1..], name) || (!name.is_empty() && walk(pattern, &name[1..]))
            }
            (Some('?'), Some(_)) => walk(&pattern[1..], &name[1..]),
            (Some('['), _) => {
                let Some(close) = pattern.iter().position(|c| *c == ']') else {
                    return name.first() == Some(&'[') && walk(&pattern[1..], &name[1..]);
                };
                let Some(first) = name.first() else {
                    return false;
                };
                let set = &pattern[1..close];
                let matched = set.chunks(3).any(|chunk| match chunk {
                    [lo, '-', hi] => *first >= *lo && *first <= *hi,
                    _ => set.contains(first),
                });
                matched && walk(&pattern[close + 1..], &name[1..])
            }
            (Some(expected), Some(actual)) if expected == actual => walk(&pattern[1..], &name[1..]),
            _ => false,
        }
    }
    let pattern: Vec<char> = pattern.chars().collect();
    let name: Vec<char> = name.chars().collect();
    walk(&pattern, &name)
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

/// Variables a **fixed-argv `git`** child inherits, on top of [`INHERITED_ENV_ALLOWLIST`] (#1661).
///
/// A `git` spawn is not an agent command, but it is not harmless either: `git commit` runs the
/// repository's hooks, and a hook is a program that came with the repository. The audit's finding was
/// that these spawns inherited the whole parent environment, so `RIGORIX_HMAC_KEY` — the key an audit
/// envelope is signed with — reached a program the repository supplies. Inheritance is now a declaration:
/// the identity git needs to author a commit, the plumbing that says *which* repository, and nothing
/// else.
pub const GIT_INHERITED_ALLOWLIST: &[&str] = &[
    "GIT_AUTHOR_NAME",
    "GIT_AUTHOR_EMAIL",
    "GIT_AUTHOR_DATE",
    "GIT_COMMITTER_NAME",
    "GIT_COMMITTER_EMAIL",
    "GIT_COMMITTER_DATE",
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_CONFIG_GLOBAL",
    "GIT_CONFIG_SYSTEM",
    "GIT_TERMINAL_PROMPT",
];

/// Names that look like git's and are deliberately **not** inherited.
///
/// Each is a program or a configuration channel the child would obey: `GIT_SSH_COMMAND` and
/// `GIT_EXTERNAL_DIFF` name an executable, and `GIT_CONFIG_COUNT`/`GIT_CONFIG_KEY_n` inject configuration
/// that was never written anywhere a reader can find. A deployment that wants one passes it as an explicit
/// override at the call site, where it is visible in the diff — the point is that inheritance cannot be the
/// route.
pub const GIT_REFUSED: &[&str] = &[
    "GIT_SSH_COMMAND",
    "GIT_SSH",
    "GIT_EXTERNAL_DIFF",
    "GIT_PAGER",
    "GIT_EDITOR",
    "GIT_CONFIG_COUNT",
    "GIT_ASKPASS",
];

/// The child environment for a fixed-argv `git` spawn (#1661).
///
/// Pure, like [`child_env`], so the declaration is testable without spawning and without touching the
/// process environment.
pub fn git_child_env(overrides: &[(String, String)]) -> Vec<(String, String)> {
    let mut env = child_env(&[]);
    for key in GIT_INHERITED_ALLOWLIST {
        if GIT_REFUSED.contains(key) {
            continue;
        }
        // sink: tools:shell-policy-env
        if let Ok(value) = std::env::var(key) {
            env.push(((*key).to_string(), value));
        }
    }
    for (key, value) in overrides {
        if !CALLER_MAY_NOT_SET.contains(&key.as_str()) && !GIT_REFUSED.contains(&key.as_str()) {
            env.push((key.clone(), value.clone()));
        }
    }
    env
}

/// Apply [`git_child_env`] to a command: clear, then declare (#1661).
///
/// The same shape as [`apply_child_env`], for the same reason: the property is a property of the helper, not
/// of twelve call sites remembering it.
#[cfg(not(feature = "portable"))]
pub fn apply_git_child_env(cmd: &mut tokio::process::Command) {
    apply_git_child_env_with(cmd, &[]);
}

/// [`apply_git_child_env`] with explicit overrides the caller decided on.
#[cfg(not(feature = "portable"))]
pub fn apply_git_child_env_with(cmd: &mut tokio::process::Command, overrides: &[(String, String)]) {
    cmd.env_clear();
    for (key, value) in git_child_env(overrides) {
        cmd.env(key, value);
    }
}

/// A `git` command whose child environment is **already declared** (#1661).
///
/// The sites that spawn `git` are twelve and most of them are two lines long; an `apply_…` call beside each
/// is a marker somebody can forget, and a forgotten marker is exactly the finding. A constructor takes that
/// possibility away: the command does not exist in a state where its environment is inherited.
///
/// It is a constructor and not a `run()` wrapper on purpose: the callers vary in `current_dir`, argv and
/// error mapping, and collapsing those into one signature would trade a property for a shape.
pub fn git_std_command() -> std::process::Command {
    // sink: tools:git-declared-spawn
    let mut command = std::process::Command::new("git");
    command.env_clear();
    for (key, value) in git_child_env(&[]) {
        command.env(key, value);
    }
    command
}

/// See [`git_std_command`]; the async twin.
#[cfg(not(feature = "portable"))]
pub fn git_tokio_command() -> tokio::process::Command {
    // sink: tools:git-declared-spawn
    let mut command = tokio::process::Command::new("git");
    command.env_clear();
    for (key, value) in git_child_env(&[]) {
        command.env(key, value);
    }
    command
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

    /// #1661: the signing key does not reach a `git` child, and git's own plumbing does.
    #[test]
    fn a_git_child_sees_git_plumbing_and_never_a_secret() {
        // SAFETY: single-threaded test module; the variables are read back by the pure helper and
        // removed again.
        unsafe {
            std::env::set_var("RIGORIX_HMAC_KEY", "the-signing-key");
            std::env::set_var("SSH_AUTH_SOCK", "/tmp/agent");
            std::env::set_var("GIT_AUTHOR_NAME", "operator");
            std::env::set_var("GIT_SSH_COMMAND", "curl example.invalid");
            std::env::set_var("GIT_CONFIG_COUNT", "1");
        }
        let env = git_child_env(&[]);
        unsafe {
            std::env::remove_var("RIGORIX_HMAC_KEY");
            std::env::remove_var("SSH_AUTH_SOCK");
            std::env::remove_var("GIT_AUTHOR_NAME");
            std::env::remove_var("GIT_SSH_COMMAND");
            std::env::remove_var("GIT_CONFIG_COUNT");
        }
        let names: Vec<&str> = env.iter().map(|(name, _)| name.as_str()).collect();
        assert!(names.contains(&"GIT_AUTHOR_NAME"), "{names:?}");
        for refused in [
            "RIGORIX_HMAC_KEY",
            "SSH_AUTH_SOCK",
            "GIT_SSH_COMMAND",
            "GIT_CONFIG_COUNT",
        ] {
            assert!(
                !names.contains(&refused),
                "`{refused}` must not be inherited by a git child: {names:?}"
            );
        }
        // Only the ones this process actually has: an allowlist entry that is unset is legitimately
        // absent from the child, and asserting on it would be a test of the host.
        for allowed in INHERITED_ENV_ALLOWLIST {
            if std::env::var(allowed).is_ok() {
                assert!(
                    names.contains(allowed),
                    "`{allowed}` is still inherited: {names:?}"
                );
            }
        }
    }

    /// A caller may not smuggle one in either — the refusal is the helper's, not the caller's.
    #[test]
    fn a_git_override_cannot_reintroduce_a_refused_name() {
        let env = git_child_env(&[
            ("GIT_SSH_COMMAND".to_string(), "curl".to_string()),
            ("RIGORIX_HMAC_KEY".to_string(), "smuggled".to_string()),
            ("GIT_AUTHOR_NAME".to_string(), "explicit".to_string()),
        ]);
        let names: Vec<&str> = env.iter().map(|(name, _)| name.as_str()).collect();
        assert!(!names.contains(&"GIT_SSH_COMMAND"), "{names:?}");
        assert!(!names.contains(&"RIGORIX_HMAC_KEY"), "{names:?}");
        assert!(
            names.contains(&"GIT_AUTHOR_NAME"),
            "an explicit, refusing-free override is allowed"
        );
    }

    /// The constructor and the pure declaration are the same set — asserted, because a constructor whose
    /// body drifts from the data is a declaration that exists and is not applied.
    #[test]
    fn the_git_constructor_applies_exactly_the_declared_environment() {
        let command = git_std_command();
        let applied: Vec<(String, String)> = command
            .get_envs()
            .map(|(key, value)| {
                (
                    key.to_string_lossy().to_string(),
                    value
                        .map(|v| v.to_string_lossy().to_string())
                        .unwrap_or_default(),
                )
            })
            .collect();
        // Compare as a set: `Command` keeps its own order, and the declaration's order is the allowlist's.
        let mut applied_sorted = applied.clone();
        applied_sorted.sort();
        let mut declared = git_child_env(&[]);
        declared.sort();
        assert_eq!(
            applied_sorted, declared,
            "the constructor must carry exactly the declared environment"
        );
        assert!(
            !applied.iter().any(|(name, _)| name == "RIGORIX_HMAC_KEY"),
            "and never the signing key"
        );
    }

    #[test]
    fn only_the_listed_names_are_permitted() {
        assert!(is_permitted("RIGORIX_TOOL"));
        assert!(!is_permitted("RIGORIX_HMAC_KEY"));
        assert!(!is_permitted("SSH_AUTH_SOCK"));
    }
}

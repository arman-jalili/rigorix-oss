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

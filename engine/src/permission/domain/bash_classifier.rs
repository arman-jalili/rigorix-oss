//! BashClassifier — classifies bash commands by intent for mode-aware gating.
//!
//! @canonical .pi/architecture/modules/permission-enforcer.md#bash-classifier
//! Implements: Contract Freeze — BashClassifier and CommandIntent
//! Issue: issue-contract-freeze
//!
//! Classifies bash commands into intent categories so the permission
//! enforcer can gate them based on the active mode. In ReadOnly mode,
//! only read-only commands are allowed. In WorkspaceWrite mode, all
//! commands are allowed. In DangerousFullAccess mode, no restrictions.
//!
//! # Contract (Frozen)
//! - Classification is purely syntactic (extracts the base command)
//! - No execution or parsing of command arguments for safety
//! - The `classify()` function is a pure function (no side effects)
//! - Command lists are frozen — additions require contract change

use serde::{Deserialize, Serialize};
use std::fmt;

/// Intent classification for a bash command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommandIntent {
    /// Read-only operations that have no side effects.
    /// Examples: ls, cat, grep, find, head, tail, wc, sort, uniq, diff
    ReadOnly,

    /// File/directory write operations within the filesystem.
    /// Examples: cp, mv, mkdir, touch, tee, install
    Write,

    /// Potentially destructive operations that modify state irreversibly.
    /// Examples: rm, shred, truncate, mkfifo, mknod, dd
    Destructive,

    /// Network-related operations that make external connections.
    /// Examples: curl, wget, ssh, scp, rsync, nc, ping
    Network,

    /// Process management operations (start, stop, signal processes).
    /// Examples: kill, pkill, killall, nice, renice, bg, fg
    ProcessManagement,

    /// Package and dependency management operations.
    /// Examples: apt, brew, pip, npm, cargo install, gem, rustup
    PackageManagement,

    /// System administration operations that modify system state.
    /// Examples: sudo, chmod, chown, mount, umount, systemctl, service, docker
    SystemAdmin,

    /// Unknown/unclassified command — treated as potentially unsafe.
    Unknown,
}

impl CommandIntent {
    /// Returns `true` if this intent is read-only (safe to execute in any mode).
    pub fn is_read_only(&self) -> bool {
        matches!(self, CommandIntent::ReadOnly)
    }

    /// Returns `true` if this intent is always unsafe (requires DangerousFullAccess).
    pub fn is_destructive(&self) -> bool {
        matches!(self, CommandIntent::Destructive)
    }

    /// Returns `true` if this intent requires write-like access.
    pub fn requires_write_access(&self) -> bool {
        matches!(
            self,
            CommandIntent::Write
                | CommandIntent::PackageManagement
                | CommandIntent::SystemAdmin
                | CommandIntent::Unknown
        )
    }

    /// Returns `true` if this intent requires full system access.
    pub fn requires_full_access(&self) -> bool {
        matches!(
            self,
            CommandIntent::Destructive
                | CommandIntent::Network
                | CommandIntent::ProcessManagement
        )
    }
}

impl fmt::Display for CommandIntent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandIntent::ReadOnly => write!(f, "read_only"),
            CommandIntent::Write => write!(f, "write"),
            CommandIntent::Destructive => write!(f, "destructive"),
            CommandIntent::Network => write!(f, "network"),
            CommandIntent::ProcessManagement => write!(f, "process_management"),
            CommandIntent::PackageManagement => write!(f, "package_management"),
            CommandIntent::SystemAdmin => write!(f, "system_admin"),
            CommandIntent::Unknown => write!(f, "unknown"),
        }
    }
}

/// Classifies bash commands into intent categories for permission gating.
///
/// Classification extracts the base command (first token before space/pipe)
/// and matches it against known command lists. Complex commands with pipes
/// or chains are classified based on the first command.
pub struct BashClassifier;

// ---------------------------------------------------------------------------
// Frozen command classification tables
// ---------------------------------------------------------------------------

/// Commands that only read state without side effects.
const READ_ONLY_COMMANDS: &[&str] = &[
    "ls", "cat", "grep", "egrep", "fgrep", "find", "head", "tail", "wc",
    "sort", "uniq", "diff", "file", "stat", "du", "df", "ps", "top", "htop",
    "who", "whoami", "env", "echo", "printf", "which", "whereis", "type",
    "printenv", "pwd", "date", "cal", "nproc", "free", "uptime", "lscpu",
    "lsblk", "lspci", "lsusb", "getfacl", "bat", "less", "more", "xxd",
    "hexdump", "od", "strings", "nm", "objdump", "readelf", "ldd",
    "git", // git subcommands classified separately — see below
];

/// Git read-only subcommands (single command: "git log", "git status").
const GIT_READ_ONLY_SUBCOMMANDS: &[&[&str]] = &[
    &["git", "log"],
    &["git", "status"],
    &["git", "diff"],
    &["git", "show"],
    &["git", "branch"],
    &["git", "tag"],
    &["git", "describe"],
    &["git", "blame"],
    &["git", "grep"],
    &["git", "ls-files"],
    &["git", "ls-tree"],
    &["git", "rev-parse"],
    &["git", "rev-list"],
    &["git", "cat-file"],
    &["git", "config", "--list"],
    &["git", "remote", "-v"],
];

/// Git write subcommands (single command: "git commit", "git push").
const GIT_WRITE_SUBCOMMANDS: &[&[&str]] = &[
    &["git", "add"],
    &["git", "commit"],
    &["git", "push"],
    &["git", "pull"],
    &["git", "fetch"],
    &["git", "merge"],
    &["git", "rebase"],
    &["git", "checkout"],
    &["git", "switch"],
    &["git", "restore"],
    &["git", "reset"],
    &["git", "stash"],
    &["git", "cherry-pick"],
    &["git", "revert"],
    &["git", "clean"],
    &["git", "rm"],
    &["git", "mv"],
];

/// Commands that write/modify state (non-destructive).
const WRITE_COMMANDS: &[&str] = &[
    "cp", "mv", "mkdir", "rmdir", "touch", "ln", "tee", "install",
    "test", "[",
];

/// Commands that are destructive or hard to undo.
const DESTRUCTIVE_COMMANDS: &[&str] = &[
    "rm", "shred", "truncate", "mkfifo", "mknod", "dd", "fallocate",
    "fstrim", "wipefs", "mkfs", "fdisk", "parted",
];

/// Commands that make network connections.
const NETWORK_COMMANDS: &[&str] = &[
    "curl", "wget", "ssh", "scp", "rsync", "nc", "ncat", "socat",
    "ping", "traceroute", "tracepath", "mtr", "nslookup", "dig",
    "host", "whois", "netstat", "ss", "ip", "ifconfig", "iwconfig",
    "iw", "tcpdump", "nmap", "telnet", "ftp", "sftp",
];

/// Commands that manage processes.
const PROCESS_COMMANDS: &[&str] = &[
    "kill", "pkill", "killall", "nice", "renice", "bg", "fg", "jobs",
    "wait", "nohup", "disown", "timeout",
];

/// Package and dependency managers.
const PACKAGE_MANAGERS: &[&str] = &[
    "apt", "apt-get", "apt-cache", "dpkg", "brew", "port", "pip",
    "pip3", "pipx", "npm", "yarn", "pnpm", "bun", "cargo", "gem",
    "rustup", "go", "dotnet", "nuget", "conda", "mamba", "flatpak",
    "snap", "pacman", "yay", "paru", "zypper", "dnf", "yum", "rpm",
    "nix", "guix", "choco", "scoop",
];

/// System administration commands.
const SYSTEM_ADMIN_COMMANDS: &[&str] = &[
    "sudo", "doas", "chmod", "chown", "chgrp", "mount", "umount",
    "systemctl", "journalctl", "service", "docker", "podman", "kubectl",
    "helm", "minikube", "systemd-run", "firewall-cmd", "ufw",
    "iptables", "nft", "sysctl", "modprobe", "insmod", "rmmod",
    "swapon", "swapoff", "crontab", "at", "batch", "logrotate",
    "setfacl", "setcap",
];

/// Cargo subcommands that are compilation/build operations.
const CARGO_BUILD_SUBCOMMANDS: &[&str] = &[
    "build", "check", "test", "bench", "clippy", "fmt", "fix",
    "doc", "run", "publish", "package", "update", "clean",
];

impl BashClassifier {
    /// Classify a bash command into an intent category.
    ///
    /// Extracts the base command from the input string by taking the first
    /// whitespace-separated token before any pipe (`|`), semicolon (`;`),
    /// or double ampersand (`&&`). Then matches against known command lists.
    ///
    /// # Edge Cases
    /// - Empty input → `Unknown`
    /// - Just whitespace → `Unknown`
    /// - Complex pipelines → classified by first command only
    /// - Path-prefixed commands (`/usr/bin/ls`) → stripped to base name
    pub fn classify(command: &str) -> CommandIntent {
        let command = command.trim();
        if command.is_empty() {
            return CommandIntent::Unknown;
        }

        // Extract the first command before pipe, semicolon, or &&
        let first_cmd = command
            .split('|')
            .next()
            .unwrap_or(command)
            .split(';')
            .next()
            .unwrap_or(command)
            .split("&&")
            .next()
            .unwrap_or(command)
            .trim();

        // Split into tokens
        let tokens: Vec<&str> = first_cmd.split_whitespace().collect();
        if tokens.is_empty() {
            return CommandIntent::Unknown;
        }

        // Handle redirection operators as first token
        if tokens[0] == ">" || tokens[0] == ">>" {
            return CommandIntent::Destructive;
        }

        // Check for `cargo <build_subcommand>` — classify as PackageManagement
        if tokens.len() >= 2 && tokens[0] == "cargo" && CARGO_BUILD_SUBCOMMANDS.contains(&tokens[1])
        {
            return CommandIntent::PackageManagement;
        }

        // Check for `cargo install` specifically
        if tokens.len() >= 2
            && tokens[0] == "cargo"
            && (tokens[1] == "install" || tokens[1] == "uninstall")
        {
            return CommandIntent::PackageManagement;
        }

        // Check for git subcommands
        if tokens.len() >= 2 && tokens[0] == "git" {
            // Check if git command includes a known write subcommand
            for sub in GIT_WRITE_SUBCOMMANDS {
                if tokens.starts_with(sub) {
                    return CommandIntent::Write;
                }
            }
            // Check if git command includes a known read-only subcommand
            for sub in GIT_READ_ONLY_SUBCOMMANDS {
                if tokens.starts_with(sub) {
                    return CommandIntent::ReadOnly;
                }
            }
            // Unknown git command — treat as write (conservative)
            return CommandIntent::Write;
        }

        // Extract base command name (strip path prefix if present)
        let base = tokens[0];
        let base_name = base.rsplit('/').next().unwrap_or(base);

        // Match against known command lists
        if READ_ONLY_COMMANDS.contains(&base_name) {
            return CommandIntent::ReadOnly;
        }
        if DESTRUCTIVE_COMMANDS.contains(&base_name) {
            return CommandIntent::Destructive;
        }
        if PACKAGE_MANAGERS.contains(&base_name) {
            return CommandIntent::PackageManagement;
        }
        if SYSTEM_ADMIN_COMMANDS.contains(&base_name) {
            return CommandIntent::SystemAdmin;
        }
        if WRITE_COMMANDS.contains(&base_name) {
            return CommandIntent::Write;
        }
        if NETWORK_COMMANDS.contains(&base_name) {
            return CommandIntent::Network;
        }
        if PROCESS_COMMANDS.contains(&base_name) {
            return CommandIntent::ProcessManagement;
        }

        CommandIntent::Unknown
    }

    /// Returns the minimum `PermissionMode` required for a given `CommandIntent`.
    pub fn required_mode_for_intent(intent: CommandIntent) -> crate::permission::domain::PermissionMode {
        use crate::permission::domain::PermissionMode;
        match intent {
            CommandIntent::ReadOnly => PermissionMode::ReadOnly,
            CommandIntent::Write => PermissionMode::WorkspaceWrite,
            CommandIntent::PackageManagement => PermissionMode::WorkspaceWrite,
            CommandIntent::SystemAdmin => PermissionMode::DangerousFullAccess,
            CommandIntent::Destructive => PermissionMode::DangerousFullAccess,
            CommandIntent::Network => PermissionMode::DangerousFullAccess,
            CommandIntent::ProcessManagement => PermissionMode::DangerousFullAccess,
            CommandIntent::Unknown => PermissionMode::WorkspaceWrite, // conservative
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Read-only command tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_classify_ls() {
        assert_eq!(BashClassifier::classify("ls"), CommandIntent::ReadOnly);
        assert_eq!(BashClassifier::classify("ls -la"), CommandIntent::ReadOnly);
        assert_eq!(
            BashClassifier::classify("/usr/bin/ls -la"),
            CommandIntent::ReadOnly
        );
    }

    #[test]
    fn test_classify_cat() {
        assert_eq!(BashClassifier::classify("cat file.txt"), CommandIntent::ReadOnly);
        assert_eq!(
            BashClassifier::classify("cat /etc/passwd"),
            CommandIntent::ReadOnly
        );
    }

    #[test]
    fn test_classify_grep() {
        assert_eq!(
            BashClassifier::classify("grep pattern file.txt"),
            CommandIntent::ReadOnly
        );
        assert_eq!(
            BashClassifier::classify("grep -r pattern /path"),
            CommandIntent::ReadOnly
        );
    }

    #[test]
    fn test_classify_find() {
        assert_eq!(
            BashClassifier::classify("find . -name '*.rs'"),
            CommandIntent::ReadOnly
        );
    }

    #[test]
    fn test_classify_head_tail_wc() {
        assert_eq!(BashClassifier::classify("head -n 10 file"), CommandIntent::ReadOnly);
        assert_eq!(BashClassifier::classify("tail -f log"), CommandIntent::ReadOnly);
        assert_eq!(BashClassifier::classify("wc -l file"), CommandIntent::ReadOnly);
    }

    #[test]
    fn test_classify_git_read_only() {
        assert_eq!(
            BashClassifier::classify("git log --oneline"),
            CommandIntent::ReadOnly
        );
        assert_eq!(
            BashClassifier::classify("git status"),
            CommandIntent::ReadOnly
        );
        assert_eq!(
            BashClassifier::classify("git diff HEAD"),
            CommandIntent::ReadOnly
        );
        assert_eq!(
            BashClassifier::classify("git show HEAD"),
            CommandIntent::ReadOnly
        );
    }

    #[test]
    fn test_classify_echo_printf() {
        assert_eq!(
            BashClassifier::classify("echo hello"),
            CommandIntent::ReadOnly
        );
        assert_eq!(
            BashClassifier::classify("printf '%s\\n' hello"),
            CommandIntent::ReadOnly
        );
    }

    // -----------------------------------------------------------------------
    // Write command tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_classify_write_commands() {
        assert_eq!(BashClassifier::classify("cp a b"), CommandIntent::Write);
        assert_eq!(BashClassifier::classify("mv a b"), CommandIntent::Write);
        assert_eq!(BashClassifier::classify("mkdir dir"), CommandIntent::Write);
        assert_eq!(BashClassifier::classify("touch file"), CommandIntent::Write);
        assert_eq!(BashClassifier::classify("tee file"), CommandIntent::Write);
    }

    #[test]
    fn test_classify_git_write() {
        assert_eq!(
            BashClassifier::classify("git add file.rs"),
            CommandIntent::Write
        );
        assert_eq!(
            BashClassifier::classify("git commit -m 'fix'"),
            CommandIntent::Write
        );
        assert_eq!(
            BashClassifier::classify("git push origin main"),
            CommandIntent::Write
        );
    }

    // -----------------------------------------------------------------------
    // Destructive command tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_classify_destructive() {
        assert_eq!(BashClassifier::classify("rm file"), CommandIntent::Destructive);
        assert_eq!(
            BashClassifier::classify("rm -rf /tmp"),
            CommandIntent::Destructive
        );
        assert_eq!(
            BashClassifier::classify("shred file"),
            CommandIntent::Destructive
        );
        assert_eq!(BashClassifier::classify("dd if=/dev/zero of=file"), CommandIntent::Destructive);
    }

    #[test]
    fn test_classify_redirect_destructive() {
        assert_eq!(
            BashClassifier::classify("> file"),
            CommandIntent::Destructive
        );
        assert_eq!(
            BashClassifier::classify(">> file"),
            CommandIntent::Destructive
        );
    }

    // -----------------------------------------------------------------------
    // Network command tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_classify_network() {
        assert_eq!(
            BashClassifier::classify("curl https://example.com"),
            CommandIntent::Network
        );
        assert_eq!(
            BashClassifier::classify("wget https://example.com/file"),
            CommandIntent::Network
        );
        assert_eq!(
            BashClassifier::classify("ping 8.8.8.8"),
            CommandIntent::Network
        );
    }

    // -----------------------------------------------------------------------
    // Package management tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_classify_package_management() {
        assert_eq!(
            BashClassifier::classify("cargo build"),
            CommandIntent::PackageManagement
        );
        assert_eq!(
            BashClassifier::classify("cargo test"),
            CommandIntent::PackageManagement
        );
        assert_eq!(
            BashClassifier::classify("cargo clippy"),
            CommandIntent::PackageManagement
        );
        assert_eq!(
            BashClassifier::classify("npm install"),
            CommandIntent::PackageManagement
        );
        assert_eq!(
            BashClassifier::classify("pip install flask"),
            CommandIntent::PackageManagement
        );
        assert_eq!(
            BashClassifier::classify("apt update"),
            CommandIntent::PackageManagement
        );
        assert_eq!(
            BashClassifier::classify("brew install curl"),
            CommandIntent::PackageManagement
        );
    }

    // -----------------------------------------------------------------------
    // System admin tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_classify_system_admin() {
        assert_eq!(
            BashClassifier::classify("sudo apt update"),
            CommandIntent::SystemAdmin
        );
        assert_eq!(
            BashClassifier::classify("chmod +x script.sh"),
            CommandIntent::SystemAdmin
        );
        assert_eq!(
            BashClassifier::classify("systemctl start nginx"),
            CommandIntent::SystemAdmin
        );
        assert_eq!(
            BashClassifier::classify("docker ps"),
            CommandIntent::SystemAdmin
        );
    }

    // -----------------------------------------------------------------------
    // Process management tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_classify_process() {
        assert_eq!(
            BashClassifier::classify("kill 1234"),
            CommandIntent::ProcessManagement
        );
        assert_eq!(
            BashClassifier::classify("pkill nginx"),
            CommandIntent::ProcessManagement
        );
        assert_eq!(
            BashClassifier::classify("jobs"),
            CommandIntent::ProcessManagement
        );
    }

    // -----------------------------------------------------------------------
    // Edge case tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_classify_empty_input() {
        assert_eq!(BashClassifier::classify(""), CommandIntent::Unknown);
        assert_eq!(BashClassifier::classify("   "), CommandIntent::Unknown);
    }

    #[test]
    fn test_classify_pipeline() {
        // Pipeline classified by first command
        assert_eq!(
            BashClassifier::classify("ls -la | grep pattern"),
            CommandIntent::ReadOnly
        );
        assert_eq!(
            BashClassifier::classify("cat file | wc -l"),
            CommandIntent::ReadOnly
        );
    }

    #[test]
    fn test_classify_unknown_command() {
        assert_eq!(
            BashClassifier::classify("some_unknown_tool"),
            CommandIntent::Unknown
        );
    }

    #[test]
    fn test_classify_semicolon_chain() {
        assert_eq!(
            BashClassifier::classify("ls; rm file"),
            CommandIntent::ReadOnly
        );
    }

    #[test]
    fn test_classify_and_chain() {
        assert_eq!(
            BashClassifier::classify("ls && echo done"),
            CommandIntent::ReadOnly
        );
    }

    #[test]
    fn test_classify_multiple_pipes() {
        assert_eq!(
            BashClassifier::classify("cat data.json | jq '.users' | head -5"),
            CommandIntent::ReadOnly
        );
    }

    #[test]
    fn test_classify_unrecognized_git_subcommand() {
        // Unknown git subcommand defaults to Write (conservative)
        assert_eq!(
            BashClassifier::classify("git unknown-cmd"),
            CommandIntent::Write
        );
    }

    #[test]
    fn test_classify_path_prefixed() {
        assert_eq!(
            BashClassifier::classify("/bin/ls -la /tmp"),
            CommandIntent::ReadOnly
        );
        assert_eq!(
            BashClassifier::classify("/usr/bin/find . -name '*.rs'"),
            CommandIntent::ReadOnly
        );
    }

    // -----------------------------------------------------------------------
    // required_mode_for_intent tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_required_mode_for_intent() {
        use crate::permission::domain::PermissionMode;
        assert_eq!(
            BashClassifier::required_mode_for_intent(CommandIntent::ReadOnly),
            PermissionMode::ReadOnly
        );
        assert_eq!(
            BashClassifier::required_mode_for_intent(CommandIntent::Write),
            PermissionMode::WorkspaceWrite
        );
        assert_eq!(
            BashClassifier::required_mode_for_intent(CommandIntent::Destructive),
            PermissionMode::DangerousFullAccess
        );
        assert_eq!(
            BashClassifier::required_mode_for_intent(CommandIntent::Network),
            PermissionMode::DangerousFullAccess
        );
        assert_eq!(
            BashClassifier::required_mode_for_intent(CommandIntent::PackageManagement),
            PermissionMode::WorkspaceWrite
        );
        assert_eq!(
            BashClassifier::required_mode_for_intent(CommandIntent::SystemAdmin),
            PermissionMode::DangerousFullAccess
        );
        assert_eq!(
            BashClassifier::required_mode_for_intent(CommandIntent::Unknown),
            PermissionMode::WorkspaceWrite
        );
    }
}

//! The server's command line, such as it is (#1589).
//!
//! @canonical `.pi/architecture/modules/server.md#startup`
//! Issue: #1589
//!
//! Before this module existed, `main.rs` read no arguments at all: the bind address came from
//! `RIGORIX_SERVER_BIND` with a default of `127.0.0.1:3001`, and every argument was accepted and
//! discarded. So `rigorix-server --port 3011` bound 3001 without a word — an operator who typed the
//! obvious flag got a server on the wrong port, and on a host where 3001 is already taken by a
//! wildcard listener the two processes coexisted with no complaint.
//!
//! The decision here is to **refuse** rather than to add a parser: the environment variable is the
//! supported surface, and a flag that does nothing is worse than a flag that is not offered. `--help`
//! is the one accepted argument, because a refusal with no way to read the accepted surface is a wall.

/// The one accepted flag, and the environment variable that actually decides the address.
pub const USAGE: &str = "\
usage: rigorix-server [--help]

  --help    print this message and exit; binds nothing

The bind address is read from RIGORIX_SERVER_BIND (default 127.0.0.1:3001). No command-line flag sets
it: a flag that is accepted and ignored is how a server ends up on a port nobody chose.";

/// What the process should do, decided before anything is initialised or bound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Startup {
    /// Start the server.
    Run,
    /// Print [`USAGE`] and exit successfully without binding.
    Help,
    /// An argument this program does not implement: refuse, and say what it would have done.
    Refused {
        /// The first argument that is not accepted.
        argument: String,
        /// The address it would otherwise have used, so the mistake is visible.
        bind: String,
    },
}

impl Startup {
    /// The exit code this decision carries, or `None` when the server should start.
    #[must_use]
    pub fn exit_code(&self) -> Option<i32> {
        match self {
            Startup::Run => None,
            Startup::Help => Some(0),
            // 2 for usage, the convention every shell tool follows: the invocation was wrong, the
            // server did nothing.
            Startup::Refused { .. } => Some(2),
        }
    }

    /// What to print, for the two decisions that print something.
    #[must_use]
    pub fn message(&self) -> Option<String> {
        match self {
            Startup::Run => None,
            Startup::Help => Some(USAGE.to_string()),
            Startup::Refused { argument, bind } => Some(format!(
                "rigorix-server: unrecognised argument '{argument}'.\n\
                 This server takes no arguments except --help; it would have bound {bind}.\n\
                 Set the address with RIGORIX_SERVER_BIND, e.g. RIGORIX_SERVER_BIND=127.0.0.1:3011.\n\n\
                 {USAGE}"
            )),
        }
    }
}

/// The address the server will bind: the environment variable, or the default when unset or empty.
#[must_use]
pub fn effective_bind(configured: Option<&str>) -> String {
    match configured {
        Some(value) if !value.trim().is_empty() => value.trim().to_string(),
        _ => "127.0.0.1:3001".to_string(),
    }
}

/// Explain where the address came from, for the startup log line.
#[must_use]
pub fn bind_provenance(configured: Option<&str>) -> &'static str {
    match configured {
        Some(value) if !value.trim().is_empty() => "from RIGORIX_SERVER_BIND",
        _ => "default, RIGORIX_SERVER_BIND is unset",
    }
}

/// Decide what to do with this process's arguments.
///
/// `--help` anywhere wins: a user asking for help should not have to guess that a later argument
/// makes their request invalid.
#[must_use]
pub fn parse_args(args: &[String], configured_bind: Option<&str>) -> Startup {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        return Startup::Help;
    }
    if let Some(argument) = args.first() {
        return Startup::Refused {
            argument: argument.clone(),
            bind: effective_bind(configured_bind),
        };
    }
    Startup::Run
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| v.to_string()).collect()
    }

    #[test]
    fn no_arguments_starts_the_server_on_the_configured_address() {
        assert_eq!(parse_args(&[], Some("0.0.0.0:4100")), Startup::Run);
        assert_eq!(effective_bind(Some("0.0.0.0:4100")), "0.0.0.0:4100");
        assert_eq!(
            bind_provenance(Some("0.0.0.0:4100")),
            "from RIGORIX_SERVER_BIND"
        );
    }

    #[test]
    fn the_default_is_named_when_nothing_configured_it() {
        assert_eq!(effective_bind(None), "127.0.0.1:3001");
        assert_eq!(effective_bind(Some("  ")), "127.0.0.1:3001");
        assert_eq!(
            bind_provenance(None),
            "default, RIGORIX_SERVER_BIND is unset"
        );
    }

    #[test]
    fn help_is_accepted_and_binds_nothing() {
        let decision = parse_args(&args(&["--help"]), None);
        assert_eq!(decision, Startup::Help);
        assert_eq!(decision.exit_code(), Some(0));
        assert!(decision.message().unwrap().contains("RIGORIX_SERVER_BIND"));
    }

    #[test]
    fn help_wins_over_an_argument_that_would_be_refused() {
        // Asking for help is never the wrong invocation.
        assert_eq!(
            parse_args(&args(&["--port", "3011", "--help"]), None),
            Startup::Help
        );
    }

    #[test]
    fn the_obvious_flag_is_refused_and_the_refusal_names_the_would_be_address() {
        let decision = parse_args(&args(&["--port", "3011"]), None);
        match &decision {
            Startup::Refused { argument, bind } => {
                assert_eq!(argument, "--port");
                assert_eq!(bind, "127.0.0.1:3001", "the address it would have used");
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
        assert_eq!(decision.exit_code(), Some(2));
        let message = decision.message().expect("a refusal prints something");
        assert!(message.contains("RIGORIX_SERVER_BIND"), "{message}");
        assert!(message.contains("127.0.0.1:3001"), "{message}");
        assert!(message.contains("usage:"), "{message}");
    }

    #[test]
    fn the_refusal_quotes_the_address_the_operator_configured() {
        let decision = parse_args(&args(&["--port"]), Some("0.0.0.0:4100"));
        let message = decision.message().unwrap();
        assert!(message.contains("0.0.0.0:4100"), "{message}");
    }
}

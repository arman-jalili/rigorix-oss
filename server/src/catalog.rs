//! The frozen `rigorix.*` method catalog (#888 OSS-C2).
//!
//! Mirrors `rigorix-sdk/schemas/api/catalog.json` (ADR-0001 D9): every catalog
//! method, its auth level (D4), and its MCP tool mapping. The server serves a
//! **subset** of the one closed `rigorix.*` catalog (ADR-0001 D3) — it MUST NOT
//! extend the namespace, and it deliberately omits the enterprise-side
//! `rigorix.auth.verify`. `server/tests/catalog_parity.rs` enforces the subset
//! relation against the SDK catalog (CI conformance job, `RIGORIX_SDK_SCHEMAS`).
//!
//! `rigorix.policy.bundle` is enterprise-only/admin and has **no** MCP tool;
//! the OSS host refuses it (`not_enabled`) rather than fabricating a local
//! policy export (the engine is not the policy source of truth).

/// Auth level for a catalog method (ADR-0001 D4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthLevel {
    /// No session required.
    Public,
    /// Requires an attested session; identity is taken from the session.
    Session,
    /// Requires an enterprise/admin scope (enterprise-only surface).
    Admin,
}

impl AuthLevel {
    /// The `catalog.json` auth-level token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Session => "session",
            Self::Admin => "admin",
        }
    }
}

/// One frozen catalog method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatalogEntry {
    /// Frozen method name (e.g. `rigorix.plan`).
    pub name: &'static str,
    /// Auth level enforced by the server.
    pub auth: AuthLevel,
    /// The MCP tool this method maps 1:1 to, if any.
    pub mcp_tool: Option<&'static str>,
}

/// The 19 frozen `rigorix.*` methods (catalog.json `methods`).
pub const CATALOG: &[CatalogEntry] = &[
    CatalogEntry {
        name: "rigorix.plan",
        auth: AuthLevel::Public,
        mcp_tool: Some("rigorix_plan"),
    },
    CatalogEntry {
        name: "rigorix.validatePlan",
        auth: AuthLevel::Public,
        mcp_tool: Some("rigorix_validate_plan"),
    },
    CatalogEntry {
        name: "rigorix.run",
        auth: AuthLevel::Session,
        mcp_tool: Some("rigorix_run"),
    },
    CatalogEntry {
        name: "rigorix.execute",
        auth: AuthLevel::Session,
        mcp_tool: Some("rigorix_execute"),
    },
    CatalogEntry {
        name: "rigorix.approve",
        auth: AuthLevel::Session,
        mcp_tool: Some("rigorix_approve_execution"),
    },
    CatalogEntry {
        name: "rigorix.checkEnforcement",
        auth: AuthLevel::Public,
        mcp_tool: Some("rigorix_check_enforcement"),
    },
    CatalogEntry {
        name: "rigorix.audit.read",
        auth: AuthLevel::Session,
        mcp_tool: Some("rigorix_read_audit"),
    },
    CatalogEntry {
        name: "rigorix.audit.list",
        auth: AuthLevel::Session,
        mcp_tool: Some("rigorix_list_audits"),
    },
    CatalogEntry {
        name: "rigorix.audit.summary",
        auth: AuthLevel::Session,
        mcp_tool: Some("rigorix_audit_summary"),
    },
    CatalogEntry {
        name: "rigorix.template.list",
        auth: AuthLevel::Session,
        mcp_tool: Some("rigorix_list_templates"),
    },
    CatalogEntry {
        name: "rigorix.template.get",
        auth: AuthLevel::Session,
        mcp_tool: Some("rigorix_get_template"),
    },
    CatalogEntry {
        name: "rigorix.template.create",
        auth: AuthLevel::Session,
        mcp_tool: Some("rigorix_create_template"),
    },
    CatalogEntry {
        name: "rigorix.template.validate",
        auth: AuthLevel::Public,
        mcp_tool: Some("rigorix_validate_template"),
    },
    CatalogEntry {
        name: "rigorix.auth.login",
        auth: AuthLevel::Public,
        mcp_tool: Some("rigorix_auth_login"),
    },
    CatalogEntry {
        name: "rigorix.auth.status",
        auth: AuthLevel::Public,
        mcp_tool: Some("rigorix_auth_status"),
    },
    CatalogEntry {
        name: "rigorix.auth.logout",
        auth: AuthLevel::Public,
        mcp_tool: Some("rigorix_auth_logout"),
    },
    // Enterprise-only/admin — no MCP counterpart; OSS refuses it.
    CatalogEntry {
        name: "rigorix.policy.bundle",
        auth: AuthLevel::Admin,
        mcp_tool: None,
    },
    CatalogEntry {
        name: "rigorix.usageGuide",
        auth: AuthLevel::Public,
        mcp_tool: Some("rigorix_get_usage_guide"),
    },
    // Handled by the server itself (reports engine/api versions + capabilities).
    CatalogEntry {
        name: "rigorix.system.version",
        auth: AuthLevel::Public,
        mcp_tool: None,
    },
];

/// Look up a catalog method by its frozen name.
pub fn find(name: &str) -> Option<&'static CatalogEntry> {
    CATALOG.iter().find(|entry| entry.name == name)
}

/// The MCP tool a catalog method maps to (ADR-0001 D9), when one exists.
pub fn mcp_tool_for(method: &str) -> Option<&'static str> {
    find(method).and_then(|entry| entry.mcp_tool)
}

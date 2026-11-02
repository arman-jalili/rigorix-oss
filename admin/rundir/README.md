# rundir

Placeholder evidence directory produced by
`admin/verify_oss_standalone.sh`. It is safe to commit as an empty directory
(even when opened by tools that ignore empty folders) and is populated with
the following example when the verifier runs:

- `rigorix.toml.example` — the minimal, offline-only configuration the
  verifier ships into a throwaway clone. It must **not** reference
  `RIGORIX_ENTERPRISE_URL`, `RIGORIX_ENTERPRISE_TOKEN`, or any anchor
  (`RIGORIX_ANCHOR_URL`), because a stand-alone `rigorix-oss` build has no
  enterprise surface by default.

The file is a template only; edit the values to taste. `admin/verify_oss_standalone.sh`
continues to run with every enterprise-facing variable unset, proving the OSS
tool works without the enterprise plane.

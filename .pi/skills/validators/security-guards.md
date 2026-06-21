---
name: security-guards
description: Pre-execution path safety and command deny-list for AI tool calls. Blocks reads of sensitive files and writes to protected directories.
model: inherit
tools: [Read]
---

# Security Guards

Defense-in-depth guards for AI tool execution. These are **pre-execution** checks — they run BEFORE the tool executes, not after.

## Path Safety: Read Blocklist

**Never allow reads** of files matching these patterns:

### Sensitive Basenames
- `.env*` (`.env`, `.env.local`, `.env.production`, etc.)
- `*.pem`, `*.key`, `*.p12`, `*.pfx` (private keys)
- `id_rsa*`, `id_dsa*`, `id_ecdsa*`, `id_ed25519*` (SSH keys)
- `known_hosts`, `authorized_keys`, `htpasswd`
- `.netrc`, `credentials`, `.pgpass`
- `.npmrc`, `.pypirc`
- `secrets.json`, `secrets.yaml`, `secrets.yml`, `secrets.toml`

### Protected Path Segments
- `/.ssh/`, `/.gnupg/`
- `/.aws/`, `/.azure/`, `/.kube/`
- `/.docker/`, `/.config/gh/`, `/.config/git/`
- `/.git/` (git internals)

## Path Safety: Write Blocklist

Writes inherit all read restrictions, **plus**:

### Forbidden System Prefixes
- `/etc/`, `/var/db/`, `/System/`
- `/Library/Keychains/`
- `/private/etc/`, `/private/var/db/`

## Shell Command Deny-list

**Hard-block** these shell commands even after user approval:

| Pattern | Reason |
|---------|--------|
| `rm -rf /` (and variants) | Recursive filesystem deletion |
| `--no-preserve-root` | Override root protection |
| `dd of=/dev/disk*`, `dd of=/dev/sd*`, `dd of=/dev/nvme*` | Raw disk overwrite |
| `mkfs*`, `fdisk`, `parted` | Disk formatting |
| `diskutil eraseDisk` | Mac disk erase |
| `curl\|sh`, `wget\|sh`, `curl\|bash` | Remote code execution |
| `sudo` | Privilege escalation |
| `terraform destroy` | Infrastructure teardown |
| `kubectl delete` | K8s resource deletion |
| `aws s3 rm --recursive` | Bulk cloud deletion |
| `shutdown`, `reboot`, `halt`, `poweroff` | System power operations |

## Read-Before-Edit Invariant

All file mutations (`edit`, `multi_edit`, `write_file`) **require** a prior `read_file` on the same path in the current session. This prevents:
- Blind overwrites of unknown files
- Stale edits on changed content
- Accidental data loss

## Implementation Note

These guards are implemented in:
- `.pi/extensions/bash-guard.ts` — runtime command blocking
- `.pi/scripts/validate-security.sh` — post-hoc validation
- `.pi/skills/agents/security-validator.md` — agent-level security review

Apply guards on **both** read and write paths. Do not bypass them.

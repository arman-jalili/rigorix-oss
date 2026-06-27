# Security Policy

## Reporting a Vulnerability

Rigorix takes security seriously. If you discover a security vulnerability,
please report it privately before disclosing it publicly.

**Do not report security vulnerabilities through public GitHub issues.**

Instead, please report via email to **security@rigorix.dev** (or the
maintainer's email in the commit history). You should receive a response
within 48 hours.

## What to include

- Description of the vulnerability
- Steps to reproduce
- Affected versions
- Any potential mitigations you've identified

## Scope

The following are in scope:
- The `rigorix-engine` library and its dependencies
- The `rigorix-cli` binary
- The `rigorix-actions` GitHub Action
- HMAC signing and verification
- API key handling and authentication

Out of scope:
- Vulnerabilities in LLM provider APIs (Anthropic, OpenAI) — report to those vendors
- Operating system or infrastructure-level vulnerabilities

## Process

1. We acknowledge receipt within 48 hours
2. We investigate and develop a fix
3. We release a patched version
4. We publicly disclose after the fix is released

## Preferred Languages

We prefer English for all security communications.

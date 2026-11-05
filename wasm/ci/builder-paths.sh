#!/usr/bin/env bash
# builder-paths.sh — the one definition of "a path from the machine that ran the build" (#1408).
#
# Sourced by `ci/run-module-smoke.sh` (the packaging path) and `ci/check-bundle-size.sh` (the
# gate that reports the number), so the two cannot disagree about what counts as a builder
# path. Each host checks *its own* names: the property is "this artifact names no filesystem",
# and it has to be assertable on the runner as well as on a maintainer's laptop.
#
# Why more than $HOME: the first version of this assertion was `strings | grep -c "$HOME"`,
# which passed while the module still carried `/rustc/1.96-aarch64-apple-darwin/lib/…` —
# 19 occurrences of the *toolchain* directory, remapped to `/rustc` but with the host-specific
# directory name kept. macOS and Linux then differ by the length of that name, so the size floor
# is a per-host fact again, which is exactly the defect #1405 hid behind a 1 % tolerance.
#
# Usage:
#   . ci/builder-paths.sh
#   builder_path_report <module>    # prints one line per prefix found; empty means clean
#
# Never a bare `grep -q`: a check that cannot look (no `strings`, no rustc) must say so, and
# the callers fail closed on that.

builder_path_prefixes() {
    local sysroot
    sysroot="$(rustc --print sysroot 2>/dev/null || true)"
    printf '%s\n' \
        "$HOME" \
        "${CARGO_HOME:-$HOME/.cargo}" \
        "${RUSTUP_HOME:-$HOME/.rustup}" \
        "${sysroot:-/no-sysroot-on-this-host}" \
        "$(basename "${sysroot:-no-sysroot}")" \
        "$PWD"
}

# builder_path_report <module> — one `<count>\t<prefix>` line per prefix the module names.
builder_path_report() {
    local module="$1" prefix count
    if ! command -v strings > /dev/null 2>&1; then
        echo "❌ \`strings\` is required to inspect the artifact; install binutils" >&2
        return 2
    fi
    if [ ! -f "$module" ]; then
        echo "❌ no artifact at ${module}" >&2
        return 2
    fi
    builder_path_prefixes | while IFS= read -r prefix; do
        [ -n "$prefix" ] || continue
        count="$(strings -a "$module" | grep -c -F -- "$prefix" || true)"
        [ "${count:-0}" -ne 0 ] && printf '%s\t%s\n' "$count" "$prefix"
    done
    return 0
}

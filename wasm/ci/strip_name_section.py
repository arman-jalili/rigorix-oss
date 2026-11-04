#!/usr/bin/env python3
"""Drop the `name` custom section from a wasm module, or report whether it has one.

Issue: #1353 — the size gates measured an artifact no host ships.

`wasm-bindgen` output carries a custom section named `name`: the debug symbol table.
It is 33 485 B of the 243 366 B empty-seam module (13.8 %) and 341 785 B of the
1 512 420 B engine-linked one (22.6 %). No production host needs it, and a trimming
step removes it — but `wasm-opt` (binaryen) is absent from this machine and from CI,
so the strip is done here with nothing but a section walk.

# The format, in full

A module is `\\0asm` + `u32` version, then a sequence of sections:

    id:byte  size:LEB128  body:size bytes

`id == 0` is a *custom* section, whose body starts with a `name:LEB128 length` +
UTF-8 bytes. Everything else is left byte-for-byte alone: `code` (10), `data` (11),
`target_features` and `producers` all stay — `target_features` is meaningful to some
runtimes, and neither of the other two is a symbol table.

Dropping a section means copying every other section's bytes verbatim, so the output
is deterministic: the same input always produces the same output, and the only
difference is the removed bytes.

# Usage

    strip_name_section.py <module.wasm>            # rewrite in place, report bytes saved
    strip_name_section.py --check <module.wasm>    # exit 1 if a `name` section is present
    strip_name_section.py --out <dst> <src>        # write elsewhere
    strip_name_section.py --self-test              # prove the parser can see a name section

Exit codes: 0 = the requested check holds / the strip succeeded; 1 = the check failed or
the file is not a wasm module. Fail-closed: a malformed module is an error, never "no
name section found".
"""

import sys

NAME = b"name"
HEADER = b"\x00asm"


def read_uleb(buf, i):
    """Decode an unsigned LEB128 at `i`; return (value, next index)."""
    result = 0
    shift = 0
    while True:
        if i >= len(buf):
            raise ValueError("truncated LEB128")
        byte = buf[i]
        i += 1
        result |= (byte & 0x7F) << shift
        if not byte & 0x80:
            return result, i
        shift += 7
        if shift > 63:
            raise ValueError("LEB128 too long")


def write_uleb(value):
    out = bytearray()
    while True:
        byte = value & 0x7F
        value >>= 7
        if value:
            out.append(byte | 0x80)
        else:
            out.append(byte)
            return bytes(out)


def sections(buf):
    """Yield (id, start, end, custom_name_or_None) for every section.

    `start`/`end` delimit the section header + body, so `buf[start:end]` is the whole
    section as it appears in the file.
    """
    if len(buf) < 8 or buf[:4] != HEADER:
        raise ValueError("not a wasm module: missing \\0asm magic")
    i = 8  # 4-byte magic + 4-byte version
    while i < len(buf):
        start = i
        section_id = buf[i]
        i += 1
        size, i = read_uleb(buf, i)
        body_start = i
        end = body_start + size
        if end > len(buf):
            raise ValueError(f"section {section_id} claims {size} B beyond the file end")
        name = None
        if section_id == 0:
            name_len, j = read_uleb(buf, body_start)
            name_end = j + name_len
            if name_end > end:
                raise ValueError("custom section name runs past its section body")
            name = buf[j:name_end]
        yield section_id, start, end, name
        i = end


def has_name_section(buf):
    return any(sid == 0 and name == NAME for sid, _, _, name in sections(buf))


def strip(buf):
    """Return (stripped bytes, removed section count, bytes removed)."""
    out = bytearray(buf[:8])
    removed = 0
    removed_bytes = 0
    for section_id, start, end, name in sections(buf):
        if section_id == 0 and name == NAME:
            removed += 1
            removed_bytes += end - start
            continue
        out += buf[start:end]
    return bytes(out), removed, removed_bytes


def _fixture(with_name, *, padding=b""):
    """A minimal module: header + one custom section (optionally named `name`) + one `code` section."""
    code_body = padding
    out = bytearray(b"\x00asm\x01\x00\x00\x00")
    if with_name:
        body = write_uleb(len(NAME)) + NAME + b"fixture-symbols"
        out += b"\x00" + write_uleb(len(body)) + body
    out += b"\x0a" + write_uleb(len(code_body)) + code_body
    return bytes(out)


def self_test():
    """The check must be able to see a `name` section, or a green run means nothing."""
    failures = 0

    def expect(label, condition):
        nonlocal failures
        print(("  ✅ " if condition else "  ✗ ") + label)
        if not condition:
            failures += 1

    with_name = _fixture(with_name=True, padding=b"\x00" * 16)
    without = _fixture(with_name=False, padding=b"\x00" * 16)

    expect("a fixture with a `name` section is detected", has_name_section(with_name))
    expect("a fixture without one is not", not has_name_section(without))

    stripped, removed, removed_bytes = strip(with_name)
    expect("strip removes exactly one section", removed == 1)
    expect("strip leaves no `name` section behind", not has_name_section(stripped))
    expect("strip preserves the `code` section", stripped[8:] == without[8:])
    expect(
        "strip removes only the section's bytes",
        removed_bytes == len(with_name) - len(stripped),
    )
    expect("strip is idempotent", strip(stripped) == (stripped, 0, 0))
    expect("a non-wasm file is refused, not treated as clean", _refuses(b"not a module"))
    expect("a truncated custom name is refused", _refuses(with_name[:-20]))

    print(f"  self-test: {9 - failures}/9 assertions")
    return 0 if failures == 0 else 1


def _refuses(buf):
    try:
        has_name_section(buf)
    except ValueError:
        return True
    return False


def main(argv):
    if len(argv) > 1 and argv[1] == "--self-test":
        return self_test()
    args = argv[1:]
    check = False
    out_path = None
    if args and args[0] == "--check":
        check = True
        args = args[1:]
    elif len(args) == 3 and args[0] == "--out":
        out_path = args[1]
        args = args[2:]
    if len(args) != 1:
        print("usage: strip_name_section.py [--check | --out DST] <module.wasm>", file=sys.stderr)
        return 2

    path = args[0]
    try:
        with open(path, "rb") as handle:
            buf = handle.read()
        present = has_name_section(buf)
    except (OSError, ValueError) as error:
        print(f"❌ {path}: {error}", file=sys.stderr)
        return 1

    if check:
        if present:
            print(f"❌ {path} carries a `name` custom section (debug symbols)", file=sys.stderr)
            print("   the packaged artifact must be stripped before it is measured", file=sys.stderr)
            return 1
        print(f"✅ {path} carries no `name` custom section")
        return 0

    if not present:
        print(f"✅ {path}: no `name` section (nothing to strip)")
        return 0

    stripped, removed, removed_bytes = strip(buf)
    target = out_path or path
    with open(target, "wb") as handle:
        handle.write(stripped)
    print(
        f"✅ stripped {removed} `name` section(s) from {path}: "
        f"-{removed_bytes} B ({len(buf)} → {len(stripped)})"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))

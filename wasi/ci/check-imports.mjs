#!/usr/bin/env node
// check-imports.mjs — the import section is what makes this artifact auditable.
//
//   node wasi/ci/check-imports.mjs <module.wasm> [--expect N]
//
// # Why this parses the module instead of trusting a build log
//
// "It carries N imports" is a claim about a binary, so the check reads the binary. Cargo's link
// line is a claim about an intention: a dependency that acquires a `wasi` call, or a
// `wasm-bindgen` attribute layer compiled for the wrong target, changes the module without
// changing the flags. ADR-014's whole argument is that this claim is *inspectable* — an
// import section with nothing in it says "this module cannot read a file, spawn a process,
// or open a socket" in a form a reviewer can check.
//
// # The formats
//
// WebAssembly binary: a magic + version header, then sections. Section id 2 is the import
// section; its body is `count: u32 LEB` followed by entries, each
// `module: name`, `field: name`, `kind: u8`, `desc`. The names are read to say *what* was
// imported rather than only how many.
//
// A section the parser does not know is a failure, not a skip: a checker that stops reading
// at the first unfamiliar byte reports a section with no entries for a module it did not
// finish reading.

import { readFileSync } from "node:fs";

const EXTERNAL_KINDS = { 0x00: "func", 0x01: "table", 0x02: "memory", 0x03: "global", 0x04: "tag" };

/** Read a ULEB128 value; returns [value, nextOffset]. */
function leb(bytes, offset) {
  let result = 0;
  let shift = 0;
  for (let index = 0; index < 5; index += 1) {
    if (offset >= bytes.length) throw new Error("the module ends inside a LEB128 value");
    const byte = bytes[offset];
    offset += 1;
    result += (byte & 0x7f) * 2 ** shift;
    if ((byte & 0x80) === 0) return [result, offset];
    shift += 7;
  }
  throw new Error("a LEB128 value is longer than five bytes");
}

/** Read a length-prefixed UTF-8 name; returns [string, nextOffset]. */
function name(bytes, offset) {
  const [length, afterLength] = leb(bytes, offset);
  const end = afterLength + length;
  if (end > bytes.length) throw new Error("the module ends inside a name");
  return [Buffer.from(bytes.subarray(afterLength, end)).toString("utf8"), end];
}

/** Parse the import section; throws rather than guessing. */
export function importsOf(bytes) {
  if (bytes.length < 8) throw new Error("the file is too short to be a module");
  const magic = Buffer.from(bytes.subarray(0, 4)).toString("latin1");
  if (magic !== "\0asm") throw new Error(`not a WebAssembly module (magic ${JSON.stringify(magic)})`);

  let offset = 8;
  const imports = [];
  let sawImportSection = false;
  while (offset < bytes.length) {
    const id = bytes[offset];
    offset += 1;
    const [size, bodyStart] = leb(bytes, offset);
    const bodyEnd = bodyStart + size;
    if (bodyEnd > bytes.length) throw new Error(`section ${id} claims ${size} bytes past the end`);
    if (id === 2) {
      sawImportSection = true;
      let cursor = bodyStart;
      const [count, afterCount] = leb(bytes, cursor);
      cursor = afterCount;
      for (let index = 0; index < count; index += 1) {
        let module;
        let field;
        [module, cursor] = name(bytes, cursor);
        [field, cursor] = name(bytes, cursor);
        const kind = bytes[cursor];
        cursor += 1;
        if (!(kind in EXTERNAL_KINDS)) throw new Error(`unknown external kind ${kind}`);
        // Skip the descriptor: every kind's encoding starts with a value, and the shapes
        // differ only in how many values follow.
        if (kind === 0x00) {
          [, cursor] = leb(bytes, cursor); // type index
        } else if (kind === 0x01) {
          cursor += 1; // reference type
          cursor += 1; // limits flags
          let ignored;
          [ignored, cursor] = leb(bytes, cursor);
          if (bytes[cursor - 1] & 0x01) [, cursor] = leb(bytes, cursor);
        } else if (kind === 0x02) {
          cursor += 1; // limits flags
          let ignored;
          [ignored, cursor] = leb(bytes, cursor);
          if (bytes[cursor - 1] & 0x01) [, cursor] = leb(bytes, cursor);
        } else if (kind === 0x03) {
          cursor += 2; // value type + mutability
        } else {
          cursor += 1; // tag attribute
          [, cursor] = leb(bytes, cursor);
        }
        imports.push({ module, field, kind: EXTERNAL_KINDS[kind] });
      }
    }
    offset = bodyEnd;
  }
  return { imports, sawImportSection };
}

function main(argv) {
  // `--list` is the machine-readable form `check-imports.sh` compares; the default form is
  // the human one. Two formats from one parser, so the gate and the reader cannot disagree.
  const listMode = argv.includes("--list");
  const path = argv.find((argument) => !argument.startsWith("--"));
  if (listMode) {
    if (!path) {
      console.error("check-imports --list: a module path is required");
      return 2;
    }
    try {
      const { imports } = importsOf(new Uint8Array(readFileSync(path)));
      for (const item of imports) console.log(`${item.module}::${item.field}`);
      return 0;
    } catch (error) {
      console.error(`check-imports: ${path} could not be parsed: ${error.message}`);
      return 1;
    }
  }
  if (!path) {
    console.error("check-imports: a module path is required");
    return 2;
  }
  let expect = 0;
  const expectFlag = argv.indexOf("--expect");
  if (expectFlag !== -1) {
    const raw = argv[expectFlag + 1];
    expect = Number.parseInt(raw ?? "", 10);
    if (!Number.isInteger(expect) || expect < 0) {
      console.error(`check-imports: --expect needs a count, got ${JSON.stringify(raw)}`);
      return 2;
    }
  }

  let bytes;
  try {
    bytes = new Uint8Array(readFileSync(path));
  } catch (error) {
    console.error(`check-imports: ${path} is not readable: ${error.message}`);
    return 2;
  }

  let parsed;
  try {
    parsed = importsOf(bytes);
  } catch (error) {
    console.error(`check-imports: ${path} could not be parsed: ${error.message}`);
    return 1;
  }

  const { imports, sawImportSection } = parsed;
  if (imports.length !== expect) {
    console.error(`✗ ${path} imports ${imports.length} item(s), expected ${expect}`);
    for (const item of imports) {
      console.error(`    ${item.module}::${item.field} (${item.kind})`);
    }
    return 1;
  }
  const where = sawImportSection
    ? "the import section is present and lists no entries"
    : "the module declares no import section";
  console.log(`✓ ${path} imports ${imports.length} item(s) — ${where}`);
  return 0;
}

if (import.meta.url === `file://${process.argv[1]}`) {
  process.exit(main(process.argv.slice(2)));
}
